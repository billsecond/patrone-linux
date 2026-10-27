//! Network use per process. Linux keeps no such count, so this adds up the
//! TCP connections each process holds: the kernel's socket diagnostics
//! (what `ss -ti` reads, no administrator rights needed) give every
//! connection's bytes sent and received, and each process's open files in
//! /proc say which connections are its own. UDP traffic (QUIC, used by
//! browsers for some sites) has no such counters and isn't counted.

use std::collections::HashMap;
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

const SOCK_DIAG_BY_FAMILY: u16 = 20;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const INET_DIAG_INFO: u16 = 2;
const TCP_LISTEN: u32 = 10;
/// Where tcp_info keeps tcpi_bytes_acked and tcpi_bytes_received.
const BYTES_ACKED: usize = 120;
const BYTES_RECEIVED: usize = 128;

/// How often to look again at which process holds which connection, and how
/// soon after an unknown connection carried data.
const REMAP_EVERY: Duration = Duration::from_secs(10);
const REMAP_SOON: Duration = Duration::from_secs(2);

#[derive(Default)]
pub struct Tracker {
    /// Each connection's bytes so far, by its socket's inode
    sockets: HashMap<u64, u64>,
    /// Which process holds each connection
    owners: HashMap<u64, i32>,
    mapped: Option<Instant>,
    started: bool,
}

impl Tracker {
    /// Bytes each process sent and received over TCP since the last call.
    /// `pids` are the processes whose open files can be read (the
    /// account's own).
    pub fn sample(&mut self, pids: &[i32]) -> HashMap<i32, u64> {
        let mut used = HashMap::new();
        let Some(counters) = tcp_counters() else {
            return used;
        };
        let mut unknown = false;
        for (&inode, &bytes) in &counters {
            let before = self.sockets.get(&inode).copied();
            // A connection new since the last reading carried all its bytes
            // since then; on the first reading nothing is counted
            let delta = match before {
                Some(before) => bytes.saturating_sub(before),
                None if self.started => bytes,
                None => 0,
            };
            if delta == 0 {
                continue;
            }
            match self.owners.get(&inode) {
                Some(&pid) => *used.entry(pid).or_insert(0) += delta,
                None => unknown = true,
            }
        }
        self.sockets = counters;
        self.started = true;

        let due = match self.mapped {
            None => true,
            Some(when) => {
                when.elapsed() >= REMAP_EVERY || (unknown && when.elapsed() >= REMAP_SOON)
            }
        };
        if due {
            self.owners = socket_owners(pids);
            self.mapped = Some(Instant::now());
        }
        used
    }
}

/// Which of these processes holds each socket, by inode.
fn socket_owners(pids: &[i32]) -> HashMap<u64, i32> {
    let mut owners = HashMap::new();
    for &pid in pids {
        let Ok(entries) = fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            let target = target.to_string_lossy();
            if let Some(inode) = target
                .strip_prefix("socket:[")
                .and_then(|rest| rest.strip_suffix(']'))
                .and_then(|inode| inode.parse::<u64>().ok())
            {
                owners.insert(inode, pid);
            }
        }
    }
    owners
}

/// Every TCP connection's bytes sent and received, by its socket's inode.
fn tcp_counters() -> Option<HashMap<u64, u64>> {
    // SAFETY: a plain socket(2) call; the descriptor is owned right away.
    let fd = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            libc::NETLINK_SOCK_DIAG,
        )
    };
    if fd < 0 {
        return None;
    }
    // SAFETY: fd is a new descriptor that nothing else owns.
    let socket = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut counters = HashMap::new();
    for family in [libc::AF_INET, libc::AF_INET6] {
        if !dump(&socket, family as u8, &mut counters) {
            return None;
        }
    }
    Some(counters)
}

/// Asks for one address family's TCP connections and reads the replies.
fn dump(socket: &OwnedFd, family: u8, counters: &mut HashMap<u64, u64>) -> bool {
    // nlmsghdr (16 bytes) and inet_diag_req_v2 (56 bytes)
    let mut request = [0u8; 72];
    request[0..4].copy_from_slice(&72u32.to_ne_bytes());
    request[4..6].copy_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
    let flags = (libc::NLM_F_REQUEST | libc::NLM_F_DUMP) as u16;
    request[6..8].copy_from_slice(&flags.to_ne_bytes());
    request[8..12].copy_from_slice(&1u32.to_ne_bytes());
    request[16] = family;
    request[17] = libc::IPPROTO_TCP as u8;
    request[18] = 1 << (INET_DIAG_INFO - 1);
    let states = 0xfff & !(1u32 << TCP_LISTEN);
    request[20..24].copy_from_slice(&states.to_ne_bytes());

    // SAFETY: an all-zero sockaddr_nl addresses the kernel.
    let mut kernel: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    kernel.nl_family = libc::AF_NETLINK as libc::sa_family_t;
    // SAFETY: the buffer and address are valid for the lengths given.
    let sent = unsafe {
        libc::sendto(
            socket.as_raw_fd(),
            request.as_ptr().cast(),
            request.len(),
            0,
            (&kernel as *const libc::sockaddr_nl).cast(),
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if sent < 0 {
        return false;
    }

    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        // SAFETY: the buffer is valid for its length.
        let received = unsafe {
            libc::recv(
                socket.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                0,
            )
        };
        if received <= 0 {
            return false;
        }
        let data = &buffer[..received as usize];
        let mut offset = 0;
        while offset + 16 <= data.len() {
            let length = u32::from_ne_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            let kind = u16::from_ne_bytes(data[offset + 4..offset + 6].try_into().unwrap());
            if length < 16 || offset + length > data.len() {
                return false;
            }
            match kind {
                NLMSG_DONE => return true,
                NLMSG_ERROR => return false,
                _ => read_connection(&data[offset + 16..offset + length], counters),
            }
            offset += (length + 3) & !3;
        }
    }
}

/// One inet_diag_msg (72 bytes) and its attributes.
fn read_connection(message: &[u8], counters: &mut HashMap<u64, u64>) {
    if message.len() < 72 {
        return;
    }
    let inode = u32::from_ne_bytes(message[68..72].try_into().unwrap()) as u64;
    let mut offset = 72;
    while offset + 4 <= message.len() {
        let length = u16::from_ne_bytes(message[offset..offset + 2].try_into().unwrap()) as usize;
        let kind = u16::from_ne_bytes(message[offset + 2..offset + 4].try_into().unwrap());
        if length < 4 || offset + length > message.len() {
            return;
        }
        let info = &message[offset + 4..offset + length];
        if kind == INET_DIAG_INFO && info.len() >= BYTES_RECEIVED + 8 && inode != 0 {
            let acked = u64::from_ne_bytes(info[BYTES_ACKED..BYTES_ACKED + 8].try_into().unwrap());
            let received =
                u64::from_ne_bytes(info[BYTES_RECEIVED..BYTES_RECEIVED + 8].try_into().unwrap());
            counters.insert(inode, acked.saturating_add(received));
        }
        offset += (length + 3) & !3;
    }
}
