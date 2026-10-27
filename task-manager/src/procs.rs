//! Reading processes from /proc: what each one is, and what it used since
//! the last reading (processor, memory, disk and network).

use std::collections::HashMap;
use std::ffi::CStr;
use std::fs::{self, File};
use std::io::Read;
use std::rc::Rc;
use std::time::Instant;

use crate::netdiag;

/// A kernel thread (`PF_KTHREAD` in /proc/PID/stat's flags).
const PF_KTHREAD: u64 = 0x0020_0000;
/// How long a new process's control group and name are read again on every
/// update: the desktop starts an app, then moves it into a scope of its own.
const SETTLING_TICKS: u32 = 10;
/// How often every process's control group is read again.
const RECHECK_EVERY: u32 = 15;

#[derive(Clone)]
pub struct Process {
    pub pid: i32,
    pub ppid: i32,
    /// When it started, in clock ticks since boot: with the PID, tells one
    /// process from a later one that got the same PID
    pub start: u64,
    pub name: Rc<str>,
    pub cmdline: Rc<str>,
    pub cgroup: Rc<str>,
    pub uid: u32,
    pub user: Rc<str>,
    pub state: u8,
    pub nice: i32,
    pub threads: u32,
    pub kernel: bool,
    pub cpu_ticks: u64,
    /// Share of the whole computer's processor time, in percent
    pub cpu: f64,
    /// Private memory: resident, less what's shared with other processes
    pub memory: u64,
    pub resident: u64,
    pub virtual_size: u64,
    /// Bytes read from and written to disk per second; None when it can't
    /// be read (another account's process)
    pub disk: Option<f64>,
    /// TCP bytes per second; None when it can't be known
    pub network: Option<f64>,
}

impl Process {
    pub fn key(&self) -> (i32, u64) {
        (self.pid, self.start)
    }
}

struct Known {
    name: Rc<str>,
    cmdline: Rc<str>,
    cgroup: Rc<str>,
    uid: u32,
    user: Rc<str>,
    age: u32,
    cpu_ticks: u64,
    disk: Option<u64>,
}

pub struct Scanner {
    known: HashMap<(i32, u64), Known>,
    users: HashMap<u32, Rc<str>>,
    pub my_uid: u32,
    ticks_per_second: f64,
    page_size: u64,
    last: Option<Instant>,
    updates: u32,
    network: netdiag::Tracker,
    buffer: String,
}

impl Scanner {
    pub fn new() -> Self {
        // SAFETY: sysconf and getuid have no preconditions.
        let (ticks, page, uid) = unsafe {
            (
                libc::sysconf(libc::_SC_CLK_TCK),
                libc::sysconf(libc::_SC_PAGESIZE),
                libc::getuid(),
            )
        };
        Scanner {
            known: HashMap::new(),
            users: HashMap::new(),
            my_uid: uid,
            ticks_per_second: if ticks > 0 { ticks as f64 } else { 100.0 },
            page_size: if page > 0 { page as u64 } else { 4096 },
            last: None,
            updates: 0,
            network: netdiag::Tracker::default(),
            buffer: String::with_capacity(4096),
        }
    }

    pub fn ticks_per_second(&self) -> f64 {
        self.ticks_per_second
    }

    /// Reads every process. `cpus` is the number of logical processors,
    /// so 100% is the whole computer.
    pub fn scan(&mut self, cpus: usize) -> Vec<Process> {
        let now = Instant::now();
        let elapsed = self
            .last
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        self.last = Some(now);
        self.updates = self.updates.wrapping_add(1);
        let recheck = self.updates.is_multiple_of(RECHECK_EVERY);

        let mut processes = Vec::with_capacity(self.known.len().max(256));
        let Ok(entries) = fs::read_dir("/proc") else {
            return processes;
        };
        let mut seen = Vec::with_capacity(self.known.len().max(256));
        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<i32>().ok())
            else {
                continue;
            };
            if let Some(process) = self.read(pid, elapsed, cpus, recheck) {
                seen.push(process.key());
                processes.push(process);
            }
        }
        let seen: std::collections::HashSet<_> = seen.into_iter().collect();
        self.known.retain(|key, _| seen.contains(key));

        // TCP traffic, for the account's own processes
        let own: Vec<i32> = processes
            .iter()
            .filter(|p| p.uid == self.my_uid && !p.kernel)
            .map(|p| p.pid)
            .collect();
        let used = self.network.sample(&own);
        for process in &mut processes {
            if process.uid == self.my_uid && !process.kernel {
                let bytes = used.get(&process.pid).copied().unwrap_or(0) as f64;
                process.network = Some(if elapsed > 0.0 { bytes / elapsed } else { 0.0 });
            }
        }
        processes
    }

    fn read(&mut self, pid: i32, elapsed: f64, cpus: usize, recheck: bool) -> Option<Process> {
        if !read_into(&format!("/proc/{pid}/stat"), &mut self.buffer) {
            return None;
        }
        let stat = parse_stat(&self.buffer)?;
        let key = (pid, stat.start);

        let fresh = !self.known.contains_key(&key);
        if fresh {
            // The real UID from status: /proc/PID itself belongs to root
            // when the process has made itself non-dumpable
            let uid = fs::read_to_string(format!("/proc/{pid}/status"))
                .ok()
                .and_then(|status| {
                    status.lines().find_map(|line| {
                        line.strip_prefix("Uid:")
                            .and_then(|ids| ids.split_whitespace().next())
                            .and_then(|uid| uid.parse().ok())
                    })
                })
                .unwrap_or(0);
            let user = self.user(uid);
            self.known.insert(
                key,
                Known {
                    name: Rc::from(""),
                    cmdline: Rc::from(""),
                    cgroup: Rc::from(""),
                    uid,
                    user,
                    age: 0,
                    cpu_ticks: stat.cpu_ticks,
                    disk: None,
                },
            );
        }
        let kernel = stat.flags & PF_KTHREAD != 0;
        let known = self.known.get_mut(&key)?;
        known.age = known.age.saturating_add(1);
        if fresh || known.age <= SETTLING_TICKS || recheck {
            known.cmdline = Rc::from(read_cmdline(pid).as_str());
            known.name = Rc::from(display_name(&stat.comm, &known.cmdline, kernel).as_str());
            known.cgroup = Rc::from(read_cgroup(pid).as_str());
        }

        let cpu = if fresh || elapsed <= 0.0 {
            0.0
        } else {
            let used =
                stat.cpu_ticks.saturating_sub(known.cpu_ticks) as f64 / self.ticks_per_second;
            (used / elapsed / cpus.max(1) as f64 * 100.0).clamp(0.0, 100.0)
        };
        known.cpu_ticks = stat.cpu_ticks;

        // Kernel threads have no memory or disk of their own to show
        let (memory, resident, virtual_size) = if kernel {
            (0, 0, 0)
        } else if read_into(&format!("/proc/{pid}/statm"), &mut self.buffer) {
            let mut fields = self
                .buffer
                .split_ascii_whitespace()
                .map(|field| field.parse::<u64>().unwrap_or(0));
            let size = fields.next().unwrap_or(0);
            let resident = fields.next().unwrap_or(0);
            let shared = fields.next().unwrap_or(0);
            (
                resident.saturating_sub(shared) * self.page_size,
                resident * self.page_size,
                size * self.page_size,
            )
        } else {
            (0, 0, 0)
        };

        let disk = if kernel {
            None
        } else if read_into(&format!("/proc/{pid}/io"), &mut self.buffer) {
            let total: u64 = self
                .buffer
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("read_bytes: ")
                        .or_else(|| line.strip_prefix("write_bytes: "))
                })
                .filter_map(|value| value.trim().parse::<u64>().ok())
                .sum();
            let rate = match known.disk {
                Some(before) if elapsed > 0.0 => total.saturating_sub(before) as f64 / elapsed,
                _ => 0.0,
            };
            known.disk = Some(total);
            Some(rate)
        } else {
            None
        };

        Some(Process {
            pid,
            ppid: stat.ppid,
            start: stat.start,
            name: known.name.clone(),
            cmdline: known.cmdline.clone(),
            cgroup: known.cgroup.clone(),
            uid: known.uid,
            user: known.user.clone(),
            state: stat.state,
            nice: stat.nice,
            threads: stat.threads,
            kernel,
            cpu_ticks: stat.cpu_ticks,
            cpu,
            memory,
            resident,
            virtual_size,
            disk,
            network: None,
        })
    }

    fn user(&mut self, uid: u32) -> Rc<str> {
        self.users
            .entry(uid)
            .or_insert_with(|| Rc::from(account(uid).0.as_str()))
            .clone()
    }
}

pub struct Stat {
    pub comm: String,
    pub state: u8,
    pub ppid: i32,
    pub flags: u64,
    pub cpu_ticks: u64,
    pub nice: i32,
    pub threads: u32,
    pub start: u64,
}

/// /proc/PID/stat: "pid (comm) state ppid ...". The name can hold spaces
/// and parentheses, so the fields start after the last ')'.
pub fn parse_stat(text: &str) -> Option<Stat> {
    let open = text.find('(')?;
    let close = text.rfind(')')?;
    let comm = text.get(open + 1..close)?.to_string();
    let fields: Vec<&str> = text.get(close + 2..)?.split_ascii_whitespace().collect();
    // fields[0] is field 3 of stat(5)
    let field = |number: usize| fields.get(number - 3).copied().unwrap_or("0");
    Some(Stat {
        comm,
        state: field(3).bytes().next().unwrap_or(b'?'),
        ppid: field(4).parse().unwrap_or(0),
        flags: field(9).parse().unwrap_or(0),
        cpu_ticks: field(14).parse::<u64>().unwrap_or(0) + field(15).parse::<u64>().unwrap_or(0),
        nice: field(19).parse().unwrap_or(0),
        threads: field(20).parse().unwrap_or(0),
        start: field(22).parse().unwrap_or(0),
    })
}

/// The name shown for a process: its command name, which the kernel cuts
/// at 15 characters, completed from its command line where that's the cut
/// ("patrone-termina" → "patrone-terminal"). Kernel threads in brackets.
fn display_name(comm: &str, cmdline: &str, kernel: bool) -> String {
    if kernel {
        return format!("[{comm}]");
    }
    if comm.len() >= 15 {
        let program = cmdline.split(' ').next().unwrap_or("");
        let base = program.rsplit('/').next().unwrap_or(program);
        if base.starts_with(comm) {
            return base.to_string();
        }
    }
    comm.to_string()
}

fn read_cmdline(pid: i32) -> String {
    let Ok(bytes) = fs::read(format!("/proc/{pid}/cmdline")) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    text.trim_end_matches('\0').replace('\0', " ")
}

/// The control group, from cgroup v2's "0::/path" line.
fn read_cgroup(pid: i32) -> String {
    fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("0::").map(str::to_string))
        })
        .unwrap_or_default()
}

fn read_into(path: &str, buffer: &mut String) -> bool {
    buffer.clear();
    match File::open(path) {
        Ok(mut file) => file.read_to_string(buffer).is_ok(),
        Err(_) => false,
    }
}

/// An account's login name and full name, from the system's user database
/// (getpwuid_r, so network and systemd-homed accounts are found too).
pub fn account(uid: u32) -> (String, String) {
    // SAFETY: an all-zero passwd is a valid out-parameter.
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    let mut buffer = vec![0 as libc::c_char; 4096];
    // SAFETY: every pointer is valid for the lengths given.
    let status = unsafe {
        libc::getpwuid_r(
            uid,
            &mut entry,
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() {
        return (uid.to_string(), String::new());
    }
    // SAFETY: getpwuid_r filled these with strings inside the buffer.
    let (login, gecos) = unsafe {
        (
            CStr::from_ptr(entry.pw_name).to_string_lossy().into_owned(),
            if entry.pw_gecos.is_null() {
                String::new()
            } else {
                CStr::from_ptr(entry.pw_gecos)
                    .to_string_lossy()
                    .into_owned()
            },
        )
    };
    let full = gecos.split(',').next().unwrap_or("").trim().to_string();
    (login, full)
}

/// The descendants of these processes, from a reading's parent links.
pub fn descendants(processes: &[Process], roots: &[(i32, u64)]) -> Vec<(i32, u64)> {
    let mut children: HashMap<i32, Vec<&Process>> = HashMap::new();
    for process in processes {
        children.entry(process.ppid).or_default().push(process);
    }
    let mut found: Vec<(i32, u64)> = roots.to_vec();
    let mut index = 0;
    while index < found.len() {
        let pid = found[index].0;
        if let Some(list) = children.get(&pid) {
            for child in list {
                if !found.contains(&child.key()) {
                    found.push(child.key());
                }
            }
        }
        index += 1;
    }
    found
}

/// Whether this process is still the one that had this PID.
pub fn still_running(pid: i32, start: u64) -> bool {
    let mut buffer = String::new();
    read_into(&format!("/proc/{pid}/stat"), &mut buffer)
        && parse_stat(&buffer).is_some_and(|stat| stat.start == start && stat.state != b'Z')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_with_spaces_in_the_name() {
        let line = "1234 (Web Content (x)) S 1 1234 1234 0 -1 4194560 100 0 0 0 250 50 0 0 20 0 7 0 98765 1000 50 18446744073709551615";
        let stat = parse_stat(line).unwrap();
        assert_eq!(stat.comm, "Web Content (x)");
        assert_eq!(stat.state, b'S');
        assert_eq!(stat.ppid, 1);
        assert_eq!(stat.cpu_ticks, 300);
        assert_eq!(stat.nice, 0);
        assert_eq!(stat.threads, 7);
        assert_eq!(stat.start, 98765);
    }

    #[test]
    fn names() {
        assert_eq!(
            display_name("patrone-termina", "/usr/bin/patrone-terminal --x", false),
            "patrone-terminal"
        );
        assert_eq!(display_name("bash", "-bash", false), "bash");
        assert_eq!(display_name("kworker/0:1", "", true), "[kworker/0:1]");
    }
}
