//! What Hyper-V support this VM has, and what it's missing: Patrone's
//! side (the integration services' daemons, and the enhanced session's
//! service), and Windows' side (the VM's settings in Hyper-V), which only
//! shows from here by what the host offers and does.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Hyper-V's integration services. The host offers each to the VM as a
/// VMBus device of its own class while it's turned on in the VM's settings
/// (Integration Services, where Hyper-V Manager gives it this name).
/// Guest services (Copy-VMFile) is off for a new VM.
const SERVICES: [(&str, &str); 6] = [
    (
        "0e0b6031-5213-4934-818b-38d90ced39db",
        "Operating system shutdown",
    ),
    (
        "9527e630-d0ae-497b-adce-e80ab0175caf",
        "Time synchronization",
    ),
    ("a9a0f4e7-5a45-4d96-b827-8a841e8c03e6", "Data Exchange"),
    ("57164f39-9115-4e78-ab55-382f3bd5422d", "Heartbeat"),
    (
        "35fa2e29-ea23-4236-96ae-3a6ebacba440",
        "Backup (volume shadow copy)",
    ),
    ("34d14be3-dee4-41c8-9ae7-6b174977c192", "Guest services"),
];

/// patrone-remote-desktop's service for VMConnect's enhanced session
/// (0.5.0 and later)
pub const SESSION_UNIT: &str = "patrone-remote-desktop-vmconnect.service";

/// Written when Hyper-V first opens an enhanced session with this VM
/// (62-patrone-hyperv-setup.rules): Hyper-V does that over a Hyper-V
/// socket only when the VM is set to use that transport, which Linux needs
pub const SEEN: &str = "/var/lib/patrone-hyperv-setup/enhanced-session";

/// Where Hyper-V's daemon keeps what the host tells the VM about itself,
/// its name among it
const HOST_POOL: &str = "/var/lib/hyperv/.kvp_pool_3";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Session {
    /// The service is running
    On,
    /// Installed but not running
    Off,
    /// patrone-remote-desktop is from before the enhanced session
    Old,
    /// patrone-remote-desktop isn't installed
    Missing,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub hyper_v: bool,
    /// The installer's live session
    pub live: bool,
    /// patrone-guest-tools is installed
    pub guest_tools: bool,
    pub session: Session,
    /// Hyper-V has opened an enhanced session with this VM, so it's set to
    /// the transport Linux needs
    pub transport: bool,
    /// Integration services turned off in the VM's settings
    pub services_off: Vec<&'static str>,
    /// The VM's name in Hyper-V
    pub vm_name: Option<String>,
}

impl Status {
    /// The checks, with the parts named in `pretend` (guest-tools,
    /// enhanced-session, transport, services) taken as missing, for testing
    /// on a VM that has them.
    pub fn check(pretend: &[String]) -> Status {
        let pretending = |part: &str| pretend.iter().any(|name| name == part);
        let mut status = Status {
            hyper_v: hyper_v(),
            live: live(),
            guest_tools: installed("patrone-guest-tools"),
            session: session(),
            transport: Path::new(SEEN).exists(),
            services_off: services_off(),
            vm_name: vm_name(),
        };
        if pretending("guest-tools") {
            status.guest_tools = false;
        }
        if pretending("enhanced-session") {
            status.session = Session::Off;
        }
        if pretending("transport") {
            status.transport = false;
        }
        if pretending("services") && status.services_off.is_empty() {
            status.services_off.push(SERVICES[5].1);
        }
        status
    }

    /// What the helper is to set up: its arguments
    pub fn to_set_up(&self) -> Vec<&'static str> {
        let mut parts = Vec::new();
        if !self.guest_tools {
            parts.push("guest-tools");
        }
        if self.session != Session::On {
            parts.push("enhanced-session");
        }
        parts
    }

    /// Whether what's to set up needs a restart to finish: the integration
    /// services' daemons start as their devices appear, and the login
    /// screen's session comes from Remote Desktop as GDM starts
    pub fn restart_after(&self) -> bool {
        !self.guest_tools || matches!(self.session, Session::Old | Session::Missing)
    }

    /// Whether Windows' side needs the VM shut down first: the transport
    /// can't change while it runs
    pub fn windows_needs_shutdown(&self) -> bool {
        !self.transport
    }

    /// The PowerShell commands for what's missing on Windows' side, if
    /// anything is, a line each
    pub fn windows_command(&self) -> Option<String> {
        let name = self
            .vm_name
            .as_deref()
            .map_or_else(|| "'NAME'".to_string(), powershell_quote);
        let mut commands = Vec::new();
        if !self.transport {
            commands.push(format!(
                "Set-VM -VMName {name} -EnhancedSessionTransportType HvSocket"
            ));
        }
        if !self.services_off.is_empty() {
            // Every one of the VM's services: the same six, without their
            // names, which Windows translates
            commands.push(format!(
                "Get-VMIntegrationService -VMName {name} | Enable-VMIntegrationService"
            ));
        }
        (!commands.is_empty()).then(|| commands.join("\n"))
    }

    /// Nothing missing on either side
    pub fn set_up(&self) -> bool {
        self.to_set_up().is_empty() && self.windows_command().is_none()
    }
}

/// systemd-detect-virt, as the login unit's ConditionVirtualization
fn hyper_v() -> bool {
    Command::new("systemd-detect-virt")
        .arg("--vm")
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "microsoft")
}

/// The installer's live session: the disc mounted at /cdrom, where casper
/// puts it (its casper-bottom/05mountpoints). Patrone's boot menu doesn't
/// say boot=casper: casper's initramfs boots that way without it. An
/// installed system's /cdrom is an empty folder.
fn live() -> bool {
    let cdrom = fs::read_to_string("/proc/self/mountinfo").is_ok_and(|mounts| {
        mounts
            .lines()
            .any(|mount| mount.split(' ').nth(4) == Some("/cdrom"))
    });
    cdrom
        || fs::read_to_string("/proc/cmdline")
            .is_ok_and(|line| line.split_whitespace().any(|word| word == "boot=casper"))
}

fn installed(package: &str) -> bool {
    Command::new("dpkg-query")
        .args(["-W", "-f=${db:Status-Status}", package])
        .output()
        .is_ok_and(|output| output.stdout == b"installed")
}

fn session() -> Session {
    if !installed("patrone-remote-desktop") {
        return Session::Missing;
    }
    if !Path::new("/usr/lib/systemd/system")
        .join(SESSION_UNIT)
        .exists()
    {
        return Session::Old;
    }
    let active = Command::new("systemctl")
        .args(["is-active", "--quiet", SESSION_UNIT])
        .status()
        .is_ok_and(|status| status.success());
    if active { Session::On } else { Session::Off }
}

/// The integration services' names whose devices the host doesn't offer
fn services_off() -> Vec<&'static str> {
    let classes: Vec<String> = fs::read_dir("/sys/bus/vmbus/devices")
        .map(|devices| {
            devices
                .flatten()
                .filter_map(|device| fs::read_to_string(device.path().join("class_id")).ok())
                .map(|class| class.trim().trim_matches(['{', '}']).to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    // No VMBus at all says nothing about the VM's settings
    if classes.is_empty() {
        return Vec::new();
    }
    SERVICES
        .iter()
        .filter(|(class, _)| !classes.iter().any(|offered| offered == class))
        .map(|(_, name)| *name)
        .collect()
}

/// The VM's name, from what the host tells it (Data Exchange): each record
/// in the pool is a key of 512 bytes and a value of 2048, padded with NULs
fn vm_name() -> Option<String> {
    let pool = fs::read(HOST_POOL).ok()?;
    let text = |bytes: &[u8]| {
        let end = bytes
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    };
    let (records, _) = pool.as_chunks::<{ 512 + 2048 }>();
    records
        .iter()
        .find(|record| text(&record[..512]) == "VirtualMachineName")
        .map(|record| text(&record[512..]))
        .filter(|name| !name.is_empty())
}

/// A string as a PowerShell literal: in single quotes, where only a single
/// quote (PowerShell counts the curly ones too) needs doubling
fn powershell_quote(text: &str) -> String {
    let mut quoted = String::from("'");
    for c in text.chars() {
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
            quoted.push(c);
        }
        quoted.push(c);
    }
    quoted.push('\'');
    quoted
}

/// The account's settings: $XDG_CONFIG_HOME, or ~/.config
pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}

/// "Don't show this again": its file, which the login unit checks for too
/// (ConditionPathExists in patrone-hyperv-setup.service)
pub fn dont_show_file() -> PathBuf {
    config_dir().join("patrone-hyperv-setup/dont-show-at-login")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> Status {
        Status {
            hyper_v: true,
            live: false,
            guest_tools: true,
            session: Session::On,
            transport: true,
            services_off: Vec::new(),
            vm_name: Some("Patrone".into()),
        }
    }

    #[test]
    fn set_up_needs_nothing() {
        let status = status();
        assert!(status.set_up());
        assert!(status.to_set_up().is_empty());
        assert_eq!(status.windows_command(), None);
    }

    #[test]
    fn missing_parts() {
        let mut status = status();
        status.guest_tools = false;
        status.session = Session::Old;
        assert_eq!(status.to_set_up(), ["guest-tools", "enhanced-session"]);
        assert!(status.restart_after());
        status.guest_tools = true;
        status.session = Session::Off;
        assert!(!status.restart_after());
    }

    #[test]
    fn windows_command() {
        let mut status = status();
        status.transport = false;
        status.vm_name = Some("Bob's VM".into());
        assert_eq!(
            status.windows_command().unwrap(),
            "Set-VM -VMName 'Bob''s VM' -EnhancedSessionTransportType HvSocket"
        );
        status.services_off.push("Guest services");
        status.vm_name = None;
        assert_eq!(
            status.windows_command().unwrap(),
            "Set-VM -VMName 'NAME' -EnhancedSessionTransportType HvSocket\n\
             Get-VMIntegrationService -VMName 'NAME' | Enable-VMIntegrationService"
        );
        status.transport = true;
        assert!(!status.windows_needs_shutdown());
    }

    #[test]
    fn quotes() {
        assert_eq!(powershell_quote("A\u{2019}s"), "'A\u{2019}\u{2019}s'");
    }
}
