//! Readings of the whole computer for the Performance page: processors,
//! memory, disks, network adapters and graphics, from /proc and /sys.

use std::collections::{BTreeSet, HashMap};
use std::ffi::CStr;
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::time::Instant;

/// What doesn't change while the computer runs: the processor's make-up.
pub struct CpuInfo {
    pub model: String,
    pub sockets: usize,
    pub cores: usize,
    pub logical: usize,
    pub base_mhz: Option<f64>,
    pub virtualization: bool,
    pub virtual_machine: bool,
    /// L1, L2 and L3 cache, in bytes
    pub caches: [Option<u64>; 3],
}

impl CpuInfo {
    pub fn read() -> Self {
        let text = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let mut model = String::new();
        let mut logical = 0;
        let mut sockets = BTreeSet::new();
        let mut cores = BTreeSet::new();
        let mut physical = String::new();
        let mut flags = String::new();
        let mut mhz = None;
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "processor" => logical += 1,
                "model name" if model.is_empty() => model = value.to_string(),
                "physical id" => {
                    physical = value.to_string();
                    sockets.insert(value.to_string());
                }
                "core id" => {
                    cores.insert(format!("{physical}/{value}"));
                }
                "flags" if flags.is_empty() => flags = value.to_string(),
                "cpu MHz" if mhz.is_none() => mhz = value.parse::<f64>().ok(),
                _ => {}
            }
        }
        let has = |flag: &str| flags.split_ascii_whitespace().any(|f| f == flag);
        let cpufreq = Path::new("/sys/devices/system/cpu/cpu0/cpufreq");
        let base_mhz = read_u64(&cpufreq.join("base_frequency"))
            .or_else(|| read_u64(&cpufreq.join("cpuinfo_max_freq")))
            .map(|khz| khz as f64 / 1000.0)
            .or(mhz);
        CpuInfo {
            model: if model.is_empty() {
                "Processor".into()
            } else {
                model
            },
            sockets: sockets.len().max(1),
            cores: if cores.is_empty() {
                logical.max(1)
            } else {
                cores.len()
            },
            logical: logical.max(1),
            base_mhz,
            virtualization: has("vmx") || has("svm"),
            virtual_machine: has("hypervisor"),
            caches: caches(),
        }
    }
}

/// The caches' sizes by level, each shared cache counted once.
fn caches() -> [Option<u64>; 3] {
    let mut seen = BTreeSet::new();
    let mut sizes = [None; 3];
    let Ok(cpus) = fs::read_dir("/sys/devices/system/cpu") else {
        return sizes;
    };
    for cpu in cpus.flatten() {
        let name = cpu.file_name().to_string_lossy().into_owned();
        if !name.starts_with("cpu")
            || !name[3..].chars().all(|c| c.is_ascii_digit())
            || name.len() == 3
        {
            continue;
        }
        let Ok(indexes) = fs::read_dir(cpu.path().join("cache")) else {
            continue;
        };
        for index in indexes.flatten() {
            let path = index.path();
            let level = read_u64(&path.join("level")).unwrap_or(0) as usize;
            let kind = read_text(&path.join("type")).unwrap_or_default();
            let shared = read_text(&path.join("shared_cpu_list")).unwrap_or_default();
            let size = read_text(&path.join("size")).and_then(|size| parse_size(&size));
            if !(1..=3).contains(&level) || !seen.insert((level, kind, shared)) {
                continue;
            }
            if let Some(size) = size {
                sizes[level - 1] = Some(sizes[level - 1].unwrap_or(0) + size);
            }
        }
    }
    sizes
}

/// "48K", "2048K", "32M".
fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let (number, unit) = text.split_at(
        text.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len()),
    );
    let number: u64 = number.parse().ok()?;
    Some(match unit {
        "K" => number * 1024,
        "M" => number * 1024 * 1024,
        "G" => number * 1024 * 1024 * 1024,
        _ => number,
    })
}

#[derive(Default, Clone)]
pub struct Memory {
    pub total: u64,
    pub available: u64,
    pub free: u64,
    pub cached: u64,
    pub modified: u64,
    pub shared: u64,
    pub kernel: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub committed: u64,
    pub commit_limit: u64,
}

impl Memory {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }
}

#[derive(Clone)]
pub struct Disk {
    pub name: String,
    pub model: String,
    /// Share of the time the disk was busy, 0 to 1
    pub active: f64,
    pub read: f64,
    pub write: f64,
    /// Milliseconds per request
    pub response: f64,
    pub capacity: u64,
    pub kind: &'static str,
    pub system: bool,
}

#[derive(Clone)]
pub struct Net {
    pub name: String,
    pub kind: &'static str,
    pub send: f64,
    pub receive: f64,
    /// Link speed in Mbit/s
    pub speed: Option<u64>,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
}

#[derive(Clone)]
pub struct Gpu {
    pub name: String,
    pub busy: f64,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
}

pub struct Sample {
    /// Share of all processors busy, 0 to 1
    pub cpu: f64,
    pub cores: Vec<f64>,
    pub mhz: Option<f64>,
    pub memory: Memory,
    pub disks: Vec<Disk>,
    pub nets: Vec<Net>,
    pub gpus: Vec<Gpu>,
    pub uptime: f64,
    pub open_files: u64,
}

pub struct Sampler {
    cpu_before: Vec<(u64, u64)>,
    disk_before: HashMap<String, [u64; 7]>,
    net_before: HashMap<String, (u64, u64)>,
    last: Option<Instant>,
    system_disk: Option<String>,
    addresses: HashMap<String, (Vec<String>, Vec<String>)>,
    updates: u32,
}

impl Sampler {
    pub fn new() -> Self {
        Sampler {
            cpu_before: Vec::new(),
            disk_before: HashMap::new(),
            net_before: HashMap::new(),
            last: None,
            system_disk: system_disk(),
            addresses: HashMap::new(),
            updates: 0,
        }
    }

    pub fn sample(&mut self) -> Sample {
        let now = Instant::now();
        let elapsed = self
            .last
            .map(|last| now.duration_since(last).as_secs_f64())
            .unwrap_or(0.0);
        self.last = Some(now);
        if self.updates.is_multiple_of(5) {
            self.addresses = addresses();
        }
        self.updates = self.updates.wrapping_add(1);

        let (cpu, cores) = self.cpu();
        Sample {
            cpu,
            cores,
            mhz: current_mhz(),
            memory: memory(),
            disks: self.disks(elapsed),
            nets: self.nets(elapsed),
            gpus: gpus(),
            uptime: fs::read_to_string("/proc/uptime")
                .ok()
                .and_then(|text| {
                    text.split_whitespace()
                        .next()
                        .and_then(|value| value.parse().ok())
                })
                .unwrap_or(0.0),
            open_files: fs::read_to_string("/proc/sys/fs/file-nr")
                .ok()
                .and_then(|text| {
                    text.split_whitespace()
                        .next()
                        .and_then(|value| value.parse().ok())
                })
                .unwrap_or(0),
        }
    }

    /// /proc/stat's "cpu" lines: busy and total time since boot.
    fn cpu(&mut self) -> (f64, Vec<f64>) {
        let text = fs::read_to_string("/proc/stat").unwrap_or_default();
        let mut now = Vec::new();
        for line in text.lines().take_while(|line| line.starts_with("cpu")) {
            let values: Vec<u64> = line
                .split_ascii_whitespace()
                .skip(1)
                .filter_map(|v| v.parse().ok())
                .collect();
            // user nice system idle iowait irq softirq steal (guest time is
            // already in user)
            let total: u64 = values.iter().take(8).sum();
            let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
            now.push((total, idle));
        }
        let busy = |index: usize| -> f64 {
            match (self.cpu_before.get(index), now.get(index)) {
                (Some(&(total_before, idle_before)), Some(&(total, idle)))
                    if total > total_before =>
                {
                    let total = (total - total_before) as f64;
                    let idle = idle.saturating_sub(idle_before) as f64;
                    ((total - idle) / total).clamp(0.0, 1.0)
                }
                _ => 0.0,
            }
        };
        let overall = busy(0);
        let cores = (1..now.len()).map(busy).collect();
        self.cpu_before = now;
        (overall, cores)
    }

    fn disks(&mut self, elapsed: f64) -> Vec<Disk> {
        let text = fs::read_to_string("/proc/diskstats").unwrap_or_default();
        let mut disks = Vec::new();
        for line in text.lines() {
            let fields: Vec<&str> = line.split_ascii_whitespace().collect();
            if fields.len() < 14 {
                continue;
            }
            let name = fields[2];
            if !shown_disk(name) {
                continue;
            }
            let number = |index: usize| fields[index].parse::<u64>().unwrap_or(0);
            // reads, sectors read, ms reading, writes, sectors written,
            // ms writing, ms busy
            let now = [
                number(3),
                number(5),
                number(6),
                number(7),
                number(9),
                number(10),
                number(12),
            ];
            let block = Path::new("/sys/block").join(name);
            let (active, read, write, response) =
                match self.disk_before.insert(name.to_string(), now) {
                    Some(before) if elapsed > 0.0 => {
                        let delta = |index: usize| now[index].saturating_sub(before[index]) as f64;
                        let requests = delta(0) + delta(3);
                        (
                            (delta(6) / (elapsed * 1000.0)).clamp(0.0, 1.0),
                            delta(1) * 512.0 / elapsed,
                            delta(4) * 512.0 / elapsed,
                            if requests > 0.0 {
                                (delta(2) + delta(5)) / requests
                            } else {
                                0.0
                            },
                        )
                    }
                    _ => (0.0, 0.0, 0.0, 0.0),
                };
            let vendor = read_text(&block.join("device/vendor")).unwrap_or_default();
            let model = read_text(&block.join("device/model")).unwrap_or_default();
            let model = match (
                vendor.is_empty() || model.starts_with(&vendor),
                model.is_empty(),
            ) {
                (_, true) => vendor,
                (true, false) => model,
                (false, false) => format!("{vendor} {model}"),
            };
            let kind = if read_u64(&block.join("removable")) == Some(1) {
                "Removable"
            } else if name.starts_with("nvme") {
                "SSD (NVMe)"
            } else if read_u64(&block.join("queue/rotational")) == Some(1) {
                "HDD"
            } else {
                "SSD"
            };
            disks.push(Disk {
                name: name.to_string(),
                model,
                active,
                read,
                write,
                response,
                capacity: read_u64(&block.join("size")).unwrap_or(0) * 512,
                kind,
                system: self.system_disk.as_deref() == Some(name),
            });
        }
        disks.sort_by(|a, b| a.name.cmp(&b.name));
        disks
    }

    fn nets(&mut self, elapsed: f64) -> Vec<Net> {
        let text = fs::read_to_string("/proc/net/dev").unwrap_or_default();
        let mut nets = Vec::new();
        for line in text.lines().skip(2) {
            let Some((name, values)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim();
            let class = Path::new("/sys/class/net").join(name);
            // Adapters with hardware behind them, not loopback or bridges
            if !class.join("device").exists() {
                continue;
            }
            let values: Vec<u64> = values
                .split_ascii_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect();
            let (received, sent) = (
                values.first().copied().unwrap_or(0),
                values.get(8).copied().unwrap_or(0),
            );
            let (receive, send) = match self.net_before.insert(name.to_string(), (received, sent)) {
                Some((received_before, sent_before)) if elapsed > 0.0 => (
                    received.saturating_sub(received_before) as f64 / elapsed,
                    sent.saturating_sub(sent_before) as f64 / elapsed,
                ),
                _ => (0.0, 0.0),
            };
            let kind = if class.join("wireless").exists() || class.join("phy80211").exists() {
                "Wi-Fi"
            } else if read_u64(&class.join("type")) == Some(1) {
                "Ethernet"
            } else {
                "Network"
            };
            let (ipv4, ipv6) = self.addresses.get(name).cloned().unwrap_or_default();
            nets.push(Net {
                name: name.to_string(),
                kind,
                send,
                receive,
                speed: read_text(&class.join("speed"))
                    .and_then(|speed| speed.parse::<i64>().ok())
                    .filter(|&s| s > 0)
                    .map(|s| s as u64),
                ipv4,
                ipv6,
            });
        }
        nets.sort_by(|a, b| a.name.cmp(&b.name));
        nets
    }
}

/// Whole disks worth showing: not loop devices, RAM disks, device-mapper
/// volumes (their disks are shown) or empty drives.
fn shown_disk(name: &str) -> bool {
    let block = Path::new("/sys/block").join(name);
    block.exists()
        && !["loop", "ram", "zram", "dm-", "fd"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        && read_u64(&block.join("size")).unwrap_or(0) > 0
}

/// The disk holding "/", through any volume or encryption layers.
fn system_disk() -> Option<String> {
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").ok()?;
    let device = mountinfo.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(' ').collect();
        (fields.get(4) == Some(&"/"))
            .then(|| fields.get(2).map(|d| d.to_string()))
            .flatten()
    })?;
    let mut path = fs::canonicalize(format!("/sys/dev/block/{device}")).ok()?;
    for _ in 0..8 {
        // A volume (dm-0, md0): follow it to what it's built on
        if let Some(slave) = fs::read_dir(path.join("slaves"))
            .ok()
            .and_then(|mut slaves| slaves.next())
            .and_then(|s| s.ok())
        {
            path = fs::canonicalize(slave.path()).ok()?;
            continue;
        }
        // A partition: its disk is the folder above
        if path.join("partition").exists() {
            path = path.parent()?.to_path_buf();
        }
        return path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
    }
    None
}

/// The processors' average speed now.
fn current_mhz() -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0;
    if let Ok(cpus) = fs::read_dir("/sys/devices/system/cpu") {
        for cpu in cpus.flatten() {
            if let Some(khz) = read_u64(&cpu.path().join("cpufreq/scaling_cur_freq")) {
                sum += khz as f64 / 1000.0;
                count += 1;
            }
        }
    }
    if count > 0 {
        return Some(sum / count as f64);
    }
    // No cpufreq (a virtual machine): what /proc/cpuinfo says
    let text = fs::read_to_string("/proc/cpuinfo").ok()?;
    for line in text.lines() {
        if let Some(value) = line
            .strip_prefix("cpu MHz")
            .and_then(|rest| rest.split(':').nth(1))
            && let Ok(mhz) = value.trim().parse::<f64>()
        {
            sum += mhz;
            count += 1;
        }
    }
    (count > 0).then(|| sum / count as f64)
}

fn memory() -> Memory {
    let text = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let mut values = HashMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let kib: u64 = value
                .split_ascii_whitespace()
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            values.insert(key.to_string(), kib * 1024);
        }
    }
    let get = |key: &str| values.get(key).copied().unwrap_or(0);
    let modified = get("Dirty") + get("Writeback");
    Memory {
        total: get("MemTotal"),
        available: get("MemAvailable"),
        free: get("MemFree"),
        cached: (get("Cached") + get("Buffers") + get("SReclaimable")).saturating_sub(modified),
        modified,
        shared: get("Shmem"),
        kernel: get("Slab") + get("KernelStack") + get("PageTables"),
        swap_total: get("SwapTotal"),
        swap_free: get("SwapFree"),
        committed: get("Committed_AS"),
        commit_limit: get("CommitLimit"),
    }
}

/// Graphics cards that say how busy they are (amdgpu's gpu_busy_percent).
fn gpus() -> Vec<Gpu> {
    let mut gpus = Vec::new();
    let Ok(cards) = fs::read_dir("/sys/class/drm") else {
        return gpus;
    };
    let mut cards: Vec<_> = cards
        .flatten()
        .map(|card| card.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.strip_prefix("card")
                .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    cards.sort();
    for card in cards {
        let device = Path::new("/sys/class/drm").join(&card).join("device");
        let Some(busy) = read_u64(&device.join("gpu_busy_percent")) else {
            continue;
        };
        let driver = fs::read_link(device.join("driver"))
            .ok()
            .and_then(|link| {
                link.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        let name = read_text(&device.join("product_name"))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| {
                if driver.is_empty() {
                    "Graphics".to_string()
                } else {
                    format!("Graphics ({driver})")
                }
            });
        gpus.push(Gpu {
            name,
            busy: (busy as f64 / 100.0).clamp(0.0, 1.0),
            memory_used: read_u64(&device.join("mem_info_vram_used")),
            memory_total: read_u64(&device.join("mem_info_vram_total")),
        });
    }
    gpus
}

/// Each adapter's IPv4 and IPv6 addresses.
fn addresses() -> HashMap<String, (Vec<String>, Vec<String>)> {
    let mut map: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills a list that freeifaddrs frees below.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return map;
    }
    let mut current = list;
    while !current.is_null() {
        // SAFETY: each node and its strings stay valid until freeifaddrs.
        let entry = unsafe { &*current };
        if !entry.ifa_addr.is_null() && !entry.ifa_name.is_null() {
            let name = unsafe { CStr::from_ptr(entry.ifa_name) }
                .to_string_lossy()
                .into_owned();
            let family = unsafe { (*entry.ifa_addr).sa_family } as i32;
            if family == libc::AF_INET {
                let address = unsafe { &*(entry.ifa_addr as *const libc::sockaddr_in) };
                let ip = Ipv4Addr::from(u32::from_be(address.sin_addr.s_addr));
                map.entry(name).or_default().0.push(ip.to_string());
            } else if family == libc::AF_INET6 {
                let address = unsafe { &*(entry.ifa_addr as *const libc::sockaddr_in6) };
                let ip = Ipv6Addr::from(address.sin6_addr.s6_addr);
                map.entry(name).or_default().1.push(ip.to_string());
            }
        }
        current = entry.ifa_next;
    }
    // SAFETY: list came from getifaddrs.
    unsafe { libc::freeifaddrs(list) };
    map
}

/// When the computer started, in seconds since 1970 (/proc/stat's btime).
pub fn boot_time() -> Option<i64> {
    fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("btime ")
                .and_then(|value| value.trim().parse().ok())
        })
}

pub fn read_text(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
}

pub fn read_u64(path: &Path) -> Option<u64> {
    read_text(path)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(parse_size("48K"), Some(48 * 1024));
        assert_eq!(parse_size("32M\n"), Some(32 * 1024 * 1024));
        assert_eq!(parse_size("x"), None);
    }

    #[test]
    fn readings() {
        let mut sampler = Sampler::new();
        sampler.sample();
        let sample = sampler.sample();
        assert!((0.0..=1.0).contains(&sample.cpu));
        assert!(sample.memory.total > 0);
        assert!(sample.memory.used() <= sample.memory.total);
        let info = CpuInfo::read();
        assert!(info.logical >= 1);
        assert_eq!(sample.cores.len(), info.logical);
    }
}
