//! Drives and their mounts (the user, 2026-10-28: "add under computer
//! Mounts where the drives are ... and add the ability to mount other
//! drives/resources"). On Linux every drive's files appear in a folder of
//! the one tree under `/`, its mount (Linux's drive letter): the system
//! drive is `/` itself, others are mounted at `/run/media/USER/NAME`,
//! network shares wherever GIO puts them. This mounts and unmounts drives, maps network
//! drives (Windows' "Map network drive", with "Reconnect at sign-in"), and
//! mounts disk images (`.iso`, `.img`) as Windows does.

use std::time::Duration;

use gtk::{gio, glib, prelude::*};

use crate::config;

thread_local! {
    static MONITOR: gio::VolumeMonitor = gio::VolumeMonitor::get();
}

/// GIO's volume monitor, kept for as long as the app runs: GIO frees it
/// with its last reference, and the signals connected to it with it, so
/// This PC and the navigation pane missed drives being mounted and plugged
/// in.
pub fn monitor() -> gio::VolumeMonitor {
    MONITOR.with(Clone::clone)
}

/// A volume's size, read from the kernel before it's mounted (GIO knows a
/// drive's size only once its files are reachable).
pub fn device_size(volume: &gio::Volume) -> Option<u64> {
    let device = volume.identifier("unix-device")?;
    let name = std::path::Path::new(device.as_str())
        .file_name()?
        .to_str()?
        .to_owned();
    let sectors: u64 = std::fs::read_to_string(format!("/sys/class/block/{name}/size"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    (sectors > 0).then_some(sectors * 512)
}

/// Where a drive's files are: its mount folder, or a network address. A
/// network drive has a folder too, GVfs's (`/run/user/UID/gvfs/...`),
/// which every app can open, a terminal included.
pub fn mount_point(root: &gio::File) -> String {
    match root.path() {
        Some(path) => path.to_string_lossy().into_owned(),
        None => root.uri().to_string(),
    }
}

// --- Mount folders -----------------------------------------------------------

/// A folder drives are mounted in, as This PC and the navigation pane show
/// it (the user, 2026-10-28: "also show /media too, making it easy to
/// access these things").
pub struct MountFolder {
    pub folder: gio::File,
    pub name: &'static str,
    pub about: &'static str,
    pub icon: &'static str,
}

/// The folder the drives this account mounts appear in: udisks makes
/// /run/media/USER (Ubuntu 26.04's udisks; /media/USER where it's built
/// the older way) with the first one. Until then, /media.
pub fn drive_mounts_folder() -> gio::File {
    let user = glib::user_name();
    let found = [
        std::path::Path::new("/run/media").join(&user),
        std::path::Path::new("/media").join(&user),
    ]
    .into_iter()
    .find(|path| path.is_dir())
    .unwrap_or_else(|| "/media".into());
    gio::File::for_path(found)
}

/// The mount folders to show: the account's own always, so it's learned
/// where drives appear, and /mnt when an administrator mounted something
/// there.
pub fn mount_folders() -> Vec<MountFolder> {
    let mut folders = vec![MountFolder {
        folder: drive_mounts_folder(),
        name: "Drive mounts",
        about: "Where USB drives and other drives appear",
        icon: "pfm-folder-removable",
    }];
    let mnt = std::path::Path::new("/mnt");
    if std::fs::read_dir(mnt).is_ok_and(|mut entries| entries.next().is_some()) {
        folders.push(MountFolder {
            folder: gio::File::for_path(mnt),
            name: "Other mounts",
            about: "Drives an administrator mounted",
            icon: "pfm-folder-drive",
        });
    }
    folders
}

/// Whether this is the account's own drive folder (/run/media/USER), which
/// goes by "Drive mounts" rather than the account's name.
pub fn is_own_drive_folder(file: &gio::File) -> bool {
    let Some(path) = file.path().filter(|_| file.is_native()) else {
        return false;
    };
    let user = glib::user_name();
    path.file_name() == Some(user.as_os_str())
        && matches!(
            path.parent().and_then(|parent| parent.to_str()),
            Some("/run/media" | "/media")
        )
}

/// Mounts a drive (asking for a password if the system wants one) and
/// gives its root folder.
pub fn mount_volume(
    volume: &gio::Volume,
    parent: Option<&gtk::Window>,
    done: impl FnOnce(Result<gio::File, glib::Error>) + 'static,
) {
    let operation =
        crate::signin::operation(parent, crate::signin::Target::Drive(volume.name().into()));
    let mounted = volume.clone();
    volume.mount(
        gio::MountMountFlags::NONE,
        Some(&operation),
        gio::Cancellable::NONE,
        move |result| {
            done(result.and_then(|()| root_of(&mounted)));
        },
    );
}

fn root_of(volume: &gio::Volume) -> Result<gio::File, glib::Error> {
    volume
        .get_mount()
        .map(|mount| mount.root())
        .ok_or_else(|| glib::Error::new(gio::IOErrorEnum::NotMounted, "The drive wasn't mounted."))
}

/// Unmounts a drive, or ejects it when it can be (a USB drive, a disc, a
/// disk image), so it's safe to remove.
pub fn unmount(
    mount: &gio::Mount,
    parent: Option<&gtk::Window>,
    done: impl FnOnce(Result<(), glib::Error>) + 'static,
) {
    let operation = gtk::MountOperation::new(parent);
    if mount.can_eject() {
        mount.eject_with_operation(
            gio::MountUnmountFlags::NONE,
            Some(&operation),
            gio::Cancellable::NONE,
            done,
        );
    } else {
        mount.unmount_with_operation(
            gio::MountUnmountFlags::NONE,
            Some(&operation),
            gio::Cancellable::NONE,
            done,
        );
    }
}

// --- Network drives ----------------------------------------------------------

/// What was typed in Map network drive, as an address GIO opens: Windows'
/// `\\server\share` (and `server\share`, `server/share`) is a Windows
/// share, `smb://server/share`; addresses with their own kind
/// (`sftp://`, `ftp://`, `dav://`, `nfs://`...) stay as they are.
pub fn network_address(typed: &str) -> String {
    let typed = typed.trim();
    if typed.contains("://") {
        return typed.to_owned();
    }
    let path = typed.trim_start_matches(['\\', '/']).replace('\\', "/");
    // "\\nas\My Files": an address has no spaces
    format!(
        "smb://{}",
        glib::Uri::escape_string(&path, Some("/:@"), true)
    )
}

/// The network drives mapped with "Reconnect at sign-in".
pub fn mapped_drives() -> Vec<String> {
    config::settings()
        .strv("mapped-drives")
        .iter()
        .map(|uri| uri.to_string())
        .collect()
}

fn set_mapped_drives(uris: &[String]) {
    let uris: Vec<&str> = uris.iter().map(String::as_str).collect();
    if let Err(error) = config::settings().set_strv("mapped-drives", uris.as_slice()) {
        eprintln!("patrone-file-manager: could not save the mapped drives: {error}");
    }
}

/// Remembers a network drive, to connect again at each sign-in.
pub fn remember_mapped(uri: &str) {
    let mut uris = mapped_drives();
    if !uris.iter().any(|known| same_address(known, uri)) {
        uris.push(uri.to_owned());
        set_mapped_drives(&uris);
    }
}

/// Forgets a mapped network drive (Disconnect).
pub fn forget_mapped(uri: &str) {
    let mut uris = mapped_drives();
    let before = uris.len();
    uris.retain(|known| !same_address(known, uri));
    if uris.len() != before {
        set_mapped_drives(&uris);
    }
}

fn same_address(a: &str, b: &str) -> bool {
    a.trim_end_matches('/')
        .eq_ignore_ascii_case(b.trim_end_matches('/'))
}

/// A short name for a network address: its share and server, as Windows
/// names a mapped drive ("share (\\server)").
pub fn network_name(uri: &str) -> String {
    let Ok(parsed) = glib::Uri::parse(uri, glib::UriFlags::NONE) else {
        return uri.to_owned();
    };
    let host = parsed
        .host()
        .map(|host| host.to_string())
        .unwrap_or_default();
    let path = parsed.path();
    let share = path.split('/').rfind(|part| !part.is_empty()).map(|part| {
        glib::Uri::unescape_string(part, None::<&str>).map_or(part.to_owned(), |s| s.to_string())
    });
    match (share, parsed.scheme().as_str()) {
        (Some(share), _) if host.is_empty() => share,
        (Some(share), "smb") => format!("{share} (\\\\{host})"),
        (Some(share), _) => format!("{share} ({host})"),
        (None, _) => host,
    }
}

/// Why a network folder didn't connect, with Windows' advice: GVfs's own
/// words can be as bare as "Invalid argument" for a computer that isn't
/// there.
pub fn describe_connect_error(error: &glib::Error) -> String {
    format!(
        "{}\n\nCheck the spelling of the name, and that the computer is on and sharing.",
        crate::dialogs::describe(error)
    )
}

/// What a sign-in for this folder says it's for: a network folder's share
/// and computer, or the folder.
pub fn target_of(file: &gio::File) -> crate::signin::Target {
    if file.is_native() {
        crate::signin::Target::Drive(file.parse_name().into())
    } else {
        crate::signin::Target::Network(network_name(&file.uri()))
    }
}

/// Connects to a network folder, asking for a sign-in if the server wants
/// one (it can be remembered in the keyring for later sign-ins).
pub async fn connect(uri: &str, parent: Option<&gtk::Window>) -> Result<gio::File, glib::Error> {
    let file = gio::File::for_uri(uri);
    let operation = crate::signin::operation(parent, target_of(&file));
    match file
        .mount_enclosing_volume_future(gio::MountMountFlags::NONE, Some(&operation))
        .await
    {
        Ok(()) => Ok(file),
        Err(error) if error.matches(gio::IOErrorEnum::AlreadyMounted) => Ok(file),
        Err(error) => Err(error),
    }
}

/// The mount a network folder is in, when it's connected: from the
/// volume monitor's list (GVfs answers `find_enclosing_mount` from a cache
/// that outlives the mount).
pub fn mount_of(uri: &str) -> Option<gio::Mount> {
    let file = gio::File::for_uri(uri);
    monitor().mounts().into_iter().find(|mount| {
        let root = mount.root();
        root.equal(&file) || file.has_prefix(&root)
    })
}

/// Whether a network folder is connected now.
pub fn is_connected(uri: &str) -> bool {
    mount_of(uri).is_some()
}

/// At sign-in (`--restore`): connects the mapped network drives again, as
/// Windows does with "Reconnect at sign-in". A share whose password wasn't
/// remembered asks for it.
pub fn reconnect_mapped(app: &gtk::Application) {
    let uris: Vec<String> = mapped_drives()
        .into_iter()
        .filter(|uri| !is_connected(uri))
        .collect();
    if uris.is_empty() {
        return;
    }
    let hold = app.hold();
    glib::spawn_future_local(async move {
        for uri in uris {
            if let Err(error) = connect(&uri, None).await {
                eprintln!("patrone-file-manager: could not reconnect {uri}: {error}");
            }
        }
        drop(hold);
    });
}

/// What an item on the Network page (or in a network computer) stands for.
#[derive(Debug, PartialEq)]
pub enum NetworkItem {
    /// A computer: smb://nas2/
    Computer(String),
    /// A shared folder: smb://nas2/photos
    Share(String),
}

/// The computer or shared folder an item is: GVfs lists them as shortcuts
/// (or mountables) to their addresses.
pub fn network_item(info: &gio::FileInfo) -> Option<NetworkItem> {
    if !matches!(
        info.file_type(),
        gio::FileType::Shortcut | gio::FileType::Mountable
    ) || !info.has_attribute("standard::target-uri")
    {
        return None;
    }
    classify_network(&info.attribute_string("standard::target-uri")?)
}

fn classify_network(uri: &str) -> Option<NetworkItem> {
    let parsed = glib::Uri::parse(uri, glib::UriFlags::NONE).ok()?;
    let network = matches!(
        parsed.scheme().as_str(),
        "smb" | "sftp" | "ssh" | "ftp" | "ftps" | "dav" | "davs" | "nfs" | "afp"
    );
    if !network || parsed.host().is_none_or(|host| host.is_empty()) {
        return None;
    }
    Some(if parsed.path().trim_matches('/').is_empty() {
        NetworkItem::Computer(uri.to_owned())
    } else {
        NetworkItem::Share(uri.to_owned())
    })
}

/// An address as Map network drive shows it: a Windows share as
/// \\server\share, as on Windows.
pub fn windows_address(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("smb://") else {
        return uri.to_owned();
    };
    let rest = glib::Uri::unescape_string(rest, None::<&str>)
        .map_or_else(|| rest.to_owned(), |text| text.to_string());
    // GVfs names a computer with SMB's own port (nas2.local:445), which
    // nobody types
    let (host, path) = rest.split_once('/').unwrap_or((&rest, ""));
    let host = host.strip_suffix(":445").unwrap_or(host);
    let path = path.trim_end_matches('/').replace('/', "\\");
    if path.is_empty() {
        format!(r"\\{host}")
    } else {
        format!(r"\\{host}\{path}")
    }
}

// --- Disk images ----------------------------------------------------------------

/// Whether a file is a disk image Windows would offer to mount.
pub fn is_disk_image(info: &gio::FileInfo) -> bool {
    let name = info.name().to_string_lossy().to_lowercase();
    let mime = crate::util::content_type(info);
    matches!(
        mime.as_str(),
        "application/x-cd-image" | "application/x-raw-disk-image" | "application/vnd.efi.iso"
    ) || name.ends_with(".iso")
        || name.ends_with(".img")
}

/// Mounts a disk image, read-only as Windows does: the system sets it up as
/// a drive (`udisksctl loop-setup`), then its file system is mounted like
/// any drive's. Gives the folder to open.
pub async fn mount_image(
    file: &gio::File,
    parent: Option<gtk::Window>,
) -> Result<gio::File, String> {
    let path = file
        .path()
        .ok_or_else(|| "Only disk images on this computer's drives can be mounted.".to_owned())?;
    let launcher = gio::SubprocessLauncher::new(
        gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE,
    );
    launcher.setenv("LC_ALL", "C", true);
    let argv = [
        std::ffi::OsStr::new("udisksctl"),
        std::ffi::OsStr::new("loop-setup"),
        std::ffi::OsStr::new("--read-only"),
        std::ffi::OsStr::new("--file"),
        path.as_os_str(),
    ];
    let process = launcher.spawn(&argv).map_err(|error| error.to_string())?;
    let (stdout, stderr) = process
        .communicate_utf8_future(None)
        .await
        .map_err(|error| error.to_string())?;
    if !process.is_successful() {
        let reason = stderr
            .map(|text| text.trim().to_owned())
            .unwrap_or_default();
        return Err(if reason.is_empty() {
            "The system couldn't set the image up as a drive.".to_owned()
        } else {
            reason
        });
    }
    let device = stdout
        .as_deref()
        .and_then(loop_device)
        .ok_or_else(|| "The system didn't say which drive the image became.".to_owned())?;
    // GIO sees the new drive a moment later: the image's own file system,
    // or its first partition's.
    let monitor = monitor();
    for _ in 0..50 {
        let volumes = monitor.volumes();
        let found = volumes
            .iter()
            .find(|volume| volume.identifier("unix-device").as_deref() == Some(device.as_str()))
            .or_else(|| {
                volumes.iter().find(|volume| {
                    volume
                        .identifier("unix-device")
                        .is_some_and(|other| other.starts_with(&format!("{device}p")))
                })
            })
            .cloned();
        if let Some(volume) = found {
            let mounted = match root_of(&volume) {
                Ok(root) => Ok(root),
                Err(_) => {
                    let (sender, receiver) = crate::dialogs::oneshot();
                    mount_volume(&volume, parent.as_ref(), move |result| {
                        sender.send(result);
                    });
                    match receiver.await {
                        Ok(root) => Ok(root),
                        // The desktop may have mounted it first.
                        Err(error) => {
                            root_of(&volume).map_err(|_| crate::dialogs::describe(&error))
                        }
                    }
                }
            };
            // udisks has the image's drive detach itself once it's
            // unmounted, so Eject puts it away.
            if mounted.is_err() {
                detach(&device).await;
            }
            return mounted;
        }
        glib::timeout_future(Duration::from_millis(200)).await;
    }
    detach(&device).await;
    Err(format!(
        "The image became {device}, but it has no file system to open."
    ))
}

/// Detaches an image's drive that couldn't be mounted.
async fn detach(device: &str) {
    let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::STDERR_SILENCE);
    if let Ok(process) = launcher.spawn(&[
        std::ffi::OsStr::new("udisksctl"),
        std::ffi::OsStr::new("loop-delete"),
        std::ffi::OsStr::new("--block-device"),
        std::ffi::OsStr::new(device),
    ]) {
        let _ = process.wait_future().await;
    }
}

/// Whether a mount is a disk image mounted from a file.
pub fn is_image_mount(mount: &gio::Mount) -> bool {
    mount
        .volume()
        .and_then(|volume| volume.identifier("class"))
        .is_some_and(|class| class == "loop")
}

/// The drive in udisksctl's "Mapped file X as /dev/loop12."
fn loop_device(output: &str) -> Option<String> {
    let at = output.rfind(" as /dev/")?;
    let device = output[at + 4..].trim().trim_end_matches('.');
    device.starts_with("/dev/loop").then(|| device.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_addresses_become_windows_shares() {
        assert_eq!(network_address(r"\\nas\photos"), "smb://nas/photos");
        assert_eq!(
            network_address(r"\\nas\photos\2026"),
            "smb://nas/photos/2026"
        );
        assert_eq!(network_address("nas/photos"), "smb://nas/photos");
        assert_eq!(network_address("  //nas/photos "), "smb://nas/photos");
        assert_eq!(
            network_address("sftp://me@host/home"),
            "sftp://me@host/home"
        );
    }

    #[test]
    fn mapped_drives_are_named_like_windows() {
        assert_eq!(network_name("smb://nas/photos"), r"photos (\\nas)");
        assert_eq!(network_name("sftp://me@host/srv/data/"), "data (host)");
        assert_eq!(network_name("ftp://ftp.example.com/"), "ftp.example.com");
        assert_eq!(network_name("localtest:///home/me/share"), "share");
    }

    #[test]
    fn the_loop_device_is_read_from_udisksctl() {
        assert_eq!(
            loop_device("Mapped file /home/me/a.iso as /dev/loop23.\n").as_deref(),
            Some("/dev/loop23")
        );
        assert_eq!(loop_device("Error setting up loop device"), None);
    }

    #[test]
    fn addresses_compare_without_a_trailing_slash() {
        assert!(same_address("smb://NAS/photos/", "smb://nas/photos"));
        assert!(!same_address("smb://nas/photos", "smb://nas/music"));
    }

    #[test]
    fn network_items_are_computers_or_shares() {
        assert_eq!(
            classify_network("smb://nas2/"),
            Some(NetworkItem::Computer("smb://nas2/".into()))
        );
        assert_eq!(
            classify_network("smb://nas2.local/users"),
            Some(NetworkItem::Share("smb://nas2.local/users".into()))
        );
        assert_eq!(classify_network("network:///nas2"), None);
        assert_eq!(classify_network("file:///home"), None);
    }

    #[test]
    fn shares_listed_by_gvfs_are_network_items() {
        // As GVfs lists a computer's shares: mountables with a target
        let info = gio::FileInfo::new();
        info.set_file_type(gio::FileType::Mountable);
        info.set_attribute_string("standard::target-uri", "smb://nas2.local:445/users");
        assert_eq!(
            network_item(&info),
            Some(NetworkItem::Share("smb://nas2.local:445/users".into()))
        );
        // An ordinary folder isn't one
        let folder = gio::FileInfo::new();
        folder.set_file_type(gio::FileType::Directory);
        assert_eq!(network_item(&folder), None);
    }

    #[test]
    fn shares_are_shown_as_windows_addresses() {
        assert_eq!(windows_address("smb://nas2/photos/"), r"\\nas2\photos");
        assert_eq!(windows_address("smb://nas2/"), r"\\nas2");
        assert_eq!(windows_address("smb://NAS2.local:445/"), r"\\NAS2.local");
        assert_eq!(
            windows_address("smb://NAS2.local:445/users"),
            r"\\NAS2.local\users"
        );
        assert_eq!(windows_address("smb://nas2/My%20Files"), r"\\nas2\My Files");
        assert_eq!(windows_address("sftp://me@host/srv"), "sftp://me@host/srv");
        // And back again, as Map network drive reads it
        assert_eq!(
            network_address(&windows_address("smb://nas2/photos")),
            "smb://nas2/photos"
        );
        assert_eq!(network_address(r"\\nas2\My Files"), "smb://nas2/My%20Files");
    }
}
