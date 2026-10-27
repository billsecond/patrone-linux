//! How Task Manager writes numbers: percentages, sizes, rates and times.

const KB: f64 = 1024.0;
const MB: f64 = 1024.0 * 1024.0;
const GB: f64 = 1024.0 * 1024.0 * 1024.0;

/// A share of the whole computer: "0%", "1.4%", "100%".
pub fn percent(value: f64) -> String {
    if value < 0.05 {
        "0%".into()
    } else if value < 99.95 {
        format!("{value:.1}%")
    } else {
        "100%".into()
    }
}

/// A whole percentage, padded like the Details page's CPU column: "00".
pub fn percent_padded(value: f64) -> String {
    format!("{:02}", value.round().clamp(0.0, 100.0) as u32)
}

/// Memory in the Processes list: "0.1 MB", "12.3 MB", "1,234.5 MB".
pub fn megabytes(bytes: f64) -> String {
    let value = bytes / MB;
    if value >= 1000.0 {
        let whole = value.floor() as u64;
        format!(
            "{}.{} MB",
            grouped(whole),
            ((value - whole as f64) * 10.0).floor() as u64
        )
    } else {
        format!("{value:.1} MB")
    }
}

/// Memory in the Details list: "12,345 K".
pub fn kilobytes(bytes: u64) -> String {
    format!("{} K", grouped(bytes / 1024))
}

/// A number with thousands separators: "12,345".
pub fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Disk use in the Processes list: "0 MB/s", "0.1 MB/s". Empty when it
/// can't be read (another account's process).
pub fn disk_rate(bytes: Option<f64>) -> String {
    match bytes {
        None => String::new(),
        Some(value) if value / MB < 0.05 => "0 MB/s".into(),
        Some(value) => format!("{:.1} MB/s", value / MB),
    }
}

/// Network use in the Processes list: "0 Mbps", "0.1 Mbps".
pub fn network_rate(bytes: Option<f64>) -> String {
    match bytes {
        None => String::new(),
        Some(value) if value * 8.0 / 1e6 < 0.05 => "0 Mbps".into(),
        Some(value) => format!("{:.1} Mbps", value * 8.0 / 1e6),
    }
}

/// A size on the Performance page: "512 KB", "12.3 MB", "7.8 GB".
pub fn bytes(value: f64) -> String {
    if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{value:.0} B")
    }
}

/// A transfer rate for disks: "0 KB/s", "120 KB/s", "12.5 MB/s".
pub fn bytes_rate(value: f64) -> String {
    if value >= GB {
        format!("{:.1} GB/s", value / GB)
    } else if value >= MB {
        format!("{:.1} MB/s", value / MB)
    } else {
        format!("{:.0} KB/s", value / KB)
    }
}

/// A transfer rate for networks: "0 Kbps", "120 Kbps", "1.2 Mbps".
pub fn bits_rate(bytes: f64) -> String {
    let bits = bytes * 8.0;
    if bits >= 1e9 {
        format!("{:.1} Gbps", bits / 1e9)
    } else if bits >= 1e6 {
        format!("{:.1} Mbps", bits / 1e6)
    } else {
        format!("{:.0} Kbps", bits / 1e3)
    }
}

/// How long the computer has been up: "0:02:13:45" (days, hours, minutes,
/// seconds).
pub fn uptime(seconds: f64) -> String {
    let total = seconds as u64;
    format!(
        "{}:{:02}:{:02}:{:02}",
        total / 86400,
        total / 3600 % 24,
        total / 60 % 60,
        total % 60
    )
}

/// Processor time: "0:00:12", "1:02:03".
pub fn cpu_time(seconds: f64) -> String {
    let total = seconds.round() as u64;
    format!("{}:{:02}:{:02}", total / 3600, total / 60 % 60, total % 60)
}

/// A clock speed from megahertz: "2.40 GHz", "800 MHz".
pub fn frequency(mhz: f64) -> String {
    if mhz >= 1000.0 {
        format!("{:.2} GHz", mhz / 1000.0)
    } else {
        format!("{mhz:.0} MHz")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1000), "1,000");
        assert_eq!(grouped(1234567), "1,234,567");
        assert_eq!(kilobytes(12_345 * 1024), "12,345 K");
        assert_eq!(megabytes(1.5 * MB), "1.5 MB");
        assert_eq!(megabytes(1234.5 * MB), "1,234.5 MB");
        assert_eq!(percent(0.0), "0%");
        assert_eq!(percent(12.34), "12.3%");
        assert_eq!(percent(100.0), "100%");
        assert_eq!(percent_padded(3.4), "03");
        assert_eq!(disk_rate(None), "");
        assert_eq!(disk_rate(Some(0.0)), "0 MB/s");
        assert_eq!(network_rate(Some(125_000.0)), "1.0 Mbps");
        assert_eq!(uptime(90061.0), "1:01:01:01");
        assert_eq!(cpu_time(3723.0), "1:02:03");
        assert_eq!(frequency(2400.0), "2.40 GHz");
        assert_eq!(bits_rate(1000.0), "8 Kbps");
    }
}
