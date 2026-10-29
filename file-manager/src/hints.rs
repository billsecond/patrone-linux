//! "System folder hints" (View → Show, and Folder Options → View): what
//! Linux's system folders and best-known files are for, and what plays the
//! same part on Windows, for administrators coming from Windows. The file
//! list tags them, a banner explains the folder on screen, and Properties
//! shows the commands behind its Permissions tab.

use std::path::Path;

use gtk::{gio, glib, prelude::*};

use crate::config;
use crate::glyph::{self, Glyph};

pub struct Hint {
    /// A word or two for the tag beside the name.
    pub tag: &'static str,
    /// What it's for.
    pub about: &'static str,
    /// What plays the same part on Windows, when something does.
    pub windows: Option<&'static str>,
}

/// Whether hints are on.
pub fn enabled() -> bool {
    config::flag("show-system-hints")
}

/// The hint for a file or folder, if it's one Linux users know by name.
pub fn for_file(file: &gio::File) -> Option<&'static Hint> {
    file.path().and_then(|path| for_path(&path))
}

pub fn for_path(path: &Path) -> Option<&'static Hint> {
    let text = path.to_str()?;
    if let Some(index) = SYSTEM.iter().position(|(paths, _)| paths.contains(&text)) {
        return Some(&SYSTEM[index].1);
    }
    let home = glib::home_dir();
    let relative = path.strip_prefix(&home).ok()?.to_str()?;
    PERSONAL
        .iter()
        .find(|(name, _)| *name == relative)
        .map(|(_, hint)| hint)
}

/// The banner above a system folder's files: what the folder is for and
/// what it is on Windows.
pub struct Banner {
    pub widget: gtk::Box,
    title: gtk::Label,
    path: gtk::Label,
    about: gtk::Label,
    windows: gtk::Label,
}

impl Banner {
    pub fn new() -> Banner {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        widget.add_css_class("hint-banner");
        widget.set_visible(false);
        let icon = Glyph::sized("info", 20);
        icon.set_valign(gtk::Align::Start);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::builder().xalign(0.0).build();
        title.add_css_class("hint-title");
        let path = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        path.add_css_class("hint-path");
        heading.append(&title);
        heading.append(&path);
        let wrapped = |class: &str| {
            let label = gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .width_chars(30)
                .max_width_chars(110)
                .selectable(true)
                .build();
            label.add_css_class(class);
            label
        };
        let about = wrapped("hint-about");
        let windows = wrapped("hint-windows");
        text.append(&heading);
        text.append(&about);
        text.append(&windows);
        let close = glyph::button("close", "Turn off system folder hints");
        close.add_css_class("flat");
        close.set_valign(gtk::Align::Start);
        close.connect_clicked(|_| {
            let _ = config::settings().set_boolean("show-system-hints", false);
        });
        widget.append(&icon);
        widget.append(&text);
        widget.append(&close);
        Banner {
            widget,
            title,
            path,
            about,
            windows,
        }
    }

    /// Explains `folder` if hints are on and it's a system folder.
    pub fn show(&self, folder: Option<&gio::File>) {
        let hint = folder
            .filter(|_| enabled())
            .and_then(|folder| Some((folder.path()?, for_file(folder)?)));
        let Some((path, hint)) = hint else {
            self.widget.set_visible(false);
            return;
        };
        self.title.set_label(hint.tag);
        self.path.set_label(&path.to_string_lossy());
        self.about.set_label(hint.about);
        self.windows.set_label(&format!(
            "On Windows: {}",
            hint.windows.unwrap_or("nothing quite like it")
        ));
        self.windows.set_visible(hint.windows.is_some());
        self.widget.set_visible(true);
    }
}

const fn hint(tag: &'static str, about: &'static str, windows: &'static str) -> Hint {
    Hint {
        tag,
        about,
        windows: if windows.is_empty() {
            None
        } else {
            Some(windows)
        },
    }
}

/// System folders and files, by path. Ubuntu links /bin, /sbin and /lib to
/// their /usr folders, so both names share a hint.
const SYSTEM: &[(&[&str], Hint)] = &[
    (
        &["/"],
        hint(
            "System drive",
            "The top of everything on this computer. Other drives and partitions appear in folders inside it rather than as drive letters.",
            r"C:\ (the system drive)",
        ),
    ),
    (
        &["/bin", "/usr/bin"],
        hint(
            "Programs",
            "The programs and commands everyone can run, such as ls, cp and firefox. Installed and updated by the package manager, so don't change files here by hand.",
            r"C:\Windows\System32 and C:\Program Files (the programs themselves)",
        ),
    ),
    (
        &["/sbin", "/usr/sbin"],
        hint(
            "System programs",
            "Commands for running the system, such as fdisk, useradd and reboot. Most of them need administrator rights (sudo).",
            r"C:\Windows\System32 (administrative tools such as diskpart and net)",
        ),
    ),
    (
        &["/boot"],
        hint(
            "Startup files",
            "The Linux kernel and the files the computer starts from. Damaging them can stop the computer from starting.",
            r"The System Reserved partition and C:\Windows\Boot",
        ),
    ),
    (
        &["/boot/efi"],
        hint(
            "EFI partition",
            "The firmware's startup partition, holding the boot loader.",
            r"The EFI System Partition (EFI\Microsoft\Boot)",
        ),
    ),
    (
        &["/dev"],
        hint(
            "Devices",
            "Every disk, USB device, terminal and sound card, each shown as a file. The system creates them as hardware appears; nothing here is stored on disk.",
            r"Device Manager, and device paths such as \\.\PhysicalDrive0",
        ),
    ),
    (
        &["/etc"],
        hint(
            "Settings",
            "The computer's own settings and those of its programs, kept as text files. Changing them needs administrator rights.",
            r"The Registry's HKEY_LOCAL_MACHINE, and C:\ProgramData",
        ),
    ),
    (
        &["/home"],
        hint(
            "Users",
            "A folder for each user, with their documents, downloads, desktop and personal settings.",
            r"C:\Users",
        ),
    ),
    (
        &["/root"],
        hint(
            "Administrator's folder",
            "The home folder of root, the built-in administrator account.",
            r"C:\Users\Administrator",
        ),
    ),
    (
        &["/lib", "/usr/lib", "/lib64", "/usr/lib64"],
        hint(
            "Libraries",
            "Shared libraries that programs load, the kernel's drivers (modules), and files that belong to installed services.",
            r"C:\Windows\System32 (the DLLs) and C:\Windows\System32\drivers",
        ),
    ),
    (
        &["/lib32", "/usr/lib32"],
        hint(
            "32-bit libraries",
            "Libraries for 32-bit programs.",
            r"C:\Windows\SysWOW64",
        ),
    ),
    (
        &["/media"],
        hint(
            "Removable drives",
            "USB drives, memory cards and discs appear here, in a folder for each user, when they're connected.",
            "The drive letters (D:, E:...) that removable drives get",
        ),
    ),
    (
        &["/mnt"],
        hint(
            "Mount point",
            "An empty place where an administrator can attach a drive by hand.",
            "Mounting a drive in an empty NTFS folder instead of a drive letter",
        ),
    ),
    (
        &["/opt"],
        hint(
            "Add-on programs",
            "Programs that didn't come from the package manager, each in a folder of its own.",
            r"C:\Program Files",
        ),
    ),
    (
        &["/proc"],
        hint(
            "Running processes",
            "A live view of the running programs and the kernel's state. Nothing here is stored on disk.",
            "Task Manager and Resource Monitor",
        ),
    ),
    (
        &["/run"],
        hint(
            "Runtime data",
            "Information for running services, such as process IDs, sockets and locks. Emptied at every restart.",
            "",
        ),
    ),
    (
        &["/cdrom"],
        hint(
            "Disc drive",
            "An old place for a CD or DVD; discs now appear under /media when inserted.",
            "The disc drive's letter (D:)",
        ),
    ),
    (
        &["/srv"],
        hint(
            "Server data",
            "Files this computer serves to others, such as websites.",
            r"C:\inetpub",
        ),
    ),
    (
        &["/sys"],
        hint(
            "Hardware information",
            "A live view of the hardware and its drivers, from the kernel. Nothing here is stored on disk.",
            "Device Manager and System Information (msinfo32)",
        ),
    ),
    (
        &["/tmp"],
        hint(
            "Temporary files",
            "Temporary files that programs create. Emptied at every restart.",
            r"C:\Windows\Temp and %TEMP%",
        ),
    ),
    (
        &["/usr"],
        hint(
            "Installed software",
            "Programs, libraries, documentation and shared data, installed by the package manager.",
            r"C:\Program Files, and much of C:\Windows",
        ),
    ),
    (
        &["/usr/share"],
        hint(
            "Shared data",
            "Data programs share: icons, themes, fonts, sounds, documentation and app menu entries.",
            r"C:\Windows\Resources, and the data folders in C:\Program Files",
        ),
    ),
    (
        &["/usr/share/applications"],
        hint(
            "Start menu entries",
            "A .desktop file for each app in the app menu, for everyone.",
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs",
        ),
    ),
    (
        &["/usr/share/fonts"],
        hint(
            "Fonts",
            "Fonts installed for everyone.",
            r"C:\Windows\Fonts",
        ),
    ),
    (
        &["/usr/share/backgrounds"],
        hint(
            "Wallpapers",
            "Desktop backgrounds installed for everyone.",
            r"C:\Windows\Web\Wallpaper",
        ),
    ),
    (
        &["/usr/share/doc"],
        hint(
            "Documentation",
            "Each installed package's documentation, licenses and change logs.",
            "",
        ),
    ),
    (
        &["/usr/local"],
        hint(
            "Locally installed software",
            "Programs an administrator built or installed by hand, kept apart from the package manager's.",
            r"C:\Program Files (for software installed by hand)",
        ),
    ),
    (
        &["/usr/src"],
        hint(
            "Source code",
            "Source code and the kernel's headers, for building drivers and programs.",
            "",
        ),
    ),
    (
        &["/var"],
        hint(
            "Changing data",
            "Files that grow and change as the system runs: logs, caches, databases, mail and print queues.",
            r"C:\ProgramData, and the logs and caches in C:\Windows",
        ),
    ),
    (
        &["/var/log"],
        hint(
            "Logs",
            "Log files from the system and its services. The system journal itself is read with the Logs app or journalctl.",
            r"Event Viewer, and C:\Windows\Logs",
        ),
    ),
    (
        &["/var/cache"],
        hint(
            "Caches",
            "Data programs can download or rebuild again, such as the packages apt downloaded.",
            r"C:\Windows\SoftwareDistribution\Download",
        ),
    ),
    (
        &["/var/lib"],
        hint(
            "Program state",
            "Data that services keep between restarts: package lists, databases, containers.",
            r"C:\ProgramData",
        ),
    ),
    (
        &["/var/tmp"],
        hint(
            "Temporary files (kept)",
            "Temporary files that survive a restart.",
            r"C:\Windows\Temp",
        ),
    ),
    (
        &["/var/www"],
        hint(
            "Websites",
            "Web pages served by a web server on this computer.",
            r"C:\inetpub\wwwroot",
        ),
    ),
    (
        &["/snap"],
        hint(
            "Snap apps",
            "Apps installed as snaps, each a read-only package of its own.",
            r"C:\Program Files\WindowsApps (Microsoft Store apps)",
        ),
    ),
    (
        &["/lost+found"],
        hint(
            "Recovered files",
            "Pieces of files the disk check (fsck) recovered after a crash. Usually empty.",
            "The FOUND.000 folders chkdsk creates",
        ),
    ),
    (
        &["/etc/apt"],
        hint(
            "Software sources",
            "Where apt gets software and updates from, and how.",
            "Windows Update's settings",
        ),
    ),
    (
        &[
            "/etc/systemd/system",
            "/usr/lib/systemd/system",
            "/lib/systemd/system",
        ],
        hint(
            "Services",
            "Unit files that define each service and when it starts. Managed with systemctl.",
            "Services (services.msc)",
        ),
    ),
    (
        &["/etc/xdg/autostart"],
        hint(
            "Startup applications (everyone)",
            "Apps that start when anyone signs in.",
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\StartUp",
        ),
    ),
    (
        &[
            "/etc/cron.d",
            "/etc/cron.daily",
            "/etc/cron.weekly",
            "/etc/cron.monthly",
            "/etc/cron.hourly",
        ],
        hint(
            "Scheduled tasks",
            "Jobs that run on a schedule.",
            "Task Scheduler",
        ),
    ),
    (
        &["/etc/skel"],
        hint(
            "New user template",
            "Files copied into each new user's home folder.",
            r"C:\Users\Default",
        ),
    ),
    (
        &["/etc/sudoers.d"],
        hint(
            "Administrator rights",
            "Extra rules for who may use sudo. Edit them only with visudo, which checks them first.",
            "The Administrators group",
        ),
    ),
    (
        &["/etc/NetworkManager/system-connections"],
        hint(
            "Network connections",
            "Saved Wi-Fi and wired connections, with their passwords. Only administrators can read them.",
            "Network Connections (ncpa.cpl) and saved Wi-Fi profiles",
        ),
    ),
    (
        &["/etc/ssh"],
        hint(
            "Remote access (SSH)",
            "Settings and host keys for the SSH server and client.",
            r"C:\ProgramData\ssh (OpenSSH Server)",
        ),
    ),
    (
        &["/etc/hosts"],
        hint(
            "Host names",
            "Names this computer looks up itself, before asking DNS.",
            r"C:\Windows\System32\drivers\etc\hosts (the same file)",
        ),
    ),
    (
        &["/etc/fstab"],
        hint(
            "Drives to mount",
            "Which drives and partitions are attached at startup, and where. A mistake here can stop the computer from starting.",
            "The drive letters set in Disk Management",
        ),
    ),
    (
        &["/etc/passwd"],
        hint(
            "User accounts",
            "Every account on the computer. The passwords themselves are in /etc/shadow.",
            "Local Users and Groups (lusrmgr.msc)",
        ),
    ),
    (
        &["/etc/shadow"],
        hint(
            "Passwords",
            "The accounts' password hashes. Only administrators can read it.",
            r"The SAM database (C:\Windows\System32\config\SAM)",
        ),
    ),
    (
        &["/etc/group"],
        hint(
            "Groups",
            "The groups on the computer and who's in them. The sudo group can act as administrator.",
            "Local Users and Groups (lusrmgr.msc)",
        ),
    ),
    (
        &["/etc/sudoers"],
        hint(
            "Administrator rights",
            "Who may use sudo. Edit it only with visudo: a mistake can lock everyone out of administrator rights.",
            "The Administrators group",
        ),
    ),
    (
        &["/etc/hostname"],
        hint(
            "Computer name",
            "This computer's name on the network.",
            "The computer name (Settings → System → About)",
        ),
    ),
    (
        &["/etc/os-release"],
        hint("Version", "Which Linux this is, and its version.", "winver"),
    ),
    (
        &["/etc/environment"],
        hint(
            "Environment variables",
            "Environment variables for everyone.",
            "System Properties → Environment Variables (system variables)",
        ),
    ),
    (
        &["/etc/crontab"],
        hint(
            "Scheduled tasks",
            "The system's table of jobs that run on a schedule.",
            "Task Scheduler",
        ),
    ),
    (
        &["/etc/resolv.conf"],
        hint(
            "DNS servers",
            "Which DNS servers this computer asks. Usually managed for you.",
            "A network adapter's DNS server settings",
        ),
    ),
    (
        &["/etc/default/grub"],
        hint(
            "Boot menu settings",
            "Settings for the boot menu (GRUB). Run sudo update-grub after changing them.",
            "bcdedit, or the Boot tab of msconfig",
        ),
    ),
];

/// Folders and files in the home folder, by their path inside it.
const PERSONAL: &[(&str, Hint)] = &[
    (
        ".config",
        hint(
            "App settings",
            "Your own settings for each app, kept as files.",
            r"%APPDATA% (AppData\Roaming) and HKEY_CURRENT_USER",
        ),
    ),
    (
        ".config/autostart",
        hint(
            "Startup applications",
            "Apps that start when you sign in.",
            r"shell:startup (Start Menu\Programs\Startup)",
        ),
    ),
    (
        ".local/share",
        hint(
            "App data",
            "Your apps' own data, such as mail, game saves and the apps you installed for yourself.",
            r"%LOCALAPPDATA% (AppData\Local)",
        ),
    ),
    (
        ".local/share/Trash",
        hint(
            "Recycle Bin",
            "The files in your Recycle Bin, and where each came from.",
            r"C:\$Recycle.Bin",
        ),
    ),
    (
        ".local/share/applications",
        hint(
            "Your Start menu entries",
            "App menu entries just for you.",
            r"%APPDATA%\Microsoft\Windows\Start Menu\Programs",
        ),
    ),
    (
        ".local/share/fonts",
        hint(
            "Your fonts",
            "Fonts installed just for you.",
            r"%LOCALAPPDATA%\Microsoft\Windows\Fonts",
        ),
    ),
    (
        ".local/bin",
        hint(
            "Your programs",
            "Programs only you run. Anything here can be run by name in a terminal.",
            "",
        ),
    ),
    (
        ".cache",
        hint(
            "Caches",
            "Data your apps can rebuild, such as thumbnails. Safe to empty.",
            r"%LOCALAPPDATA%\Temp, and apps' cache folders",
        ),
    ),
    (
        ".ssh",
        hint(
            "SSH keys",
            "Your SSH keys and the servers you've connected to. Never share the private keys (the files without .pub).",
            r"%USERPROFILE%\.ssh",
        ),
    ),
    (
        "snap",
        hint(
            "Snap app data",
            "Each snap app's data and settings.",
            r"%LOCALAPPDATA%\Packages",
        ),
    ),
    (
        ".bashrc",
        hint(
            "Shell settings",
            "Commands run whenever you open a terminal: aliases, your prompt, PATH.",
            "Your PowerShell profile ($PROFILE)",
        ),
    ),
    (
        ".profile",
        hint(
            "Sign-in settings",
            "Run when you sign in; sets environment variables such as PATH.",
            "Your user environment variables",
        ),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_system_and_personal_hints() {
        assert_eq!(for_path(Path::new("/etc")).map(|h| h.tag), Some("Settings"));
        assert_eq!(
            for_path(Path::new("/usr/bin")).map(|h| h.tag),
            Some("Programs")
        );
        assert!(for_path(Path::new("/etc/nothing-here")).is_none());
        let config = glib::home_dir().join(".config");
        assert_eq!(for_path(&config).map(|h| h.tag), Some("App settings"));
    }
}
