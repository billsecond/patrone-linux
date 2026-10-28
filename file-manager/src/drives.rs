//! Drives and their mounts (the user, 2026-10-28: "add under computer
//! Mounts where the drives are ... and add the ability to mount other
//! drives/resources"). On Linux every drive's files appear in a folder of
//! the one tree under `/`, its mount (Linux's drive letter): the system
//! drive is `/` itself, others are mounted at `/media/NAME`, network shares
//! wherever GIO puts them. This mounts and unmounts drives, maps network
//! drives (Windows' "Map network drive", with "Reconnect at sign-in"), and
//! mounts disk images (`.iso`, `.img`) as Windows does.

use std::time::Duration;

use gtk::{gio, glib, prelude::*};

use crate::config;

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

/// Where a drive's files are: its mount folder, or a network address.
pub fn mount_point(root: &gio::File) -> String {
    if root.is_native() {
        root.path()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        root.uri().to_string()
    }
}

/// Mounts a drive (asking for a password if the system wants one) and
/// gives its root folder.
pub fn mount_volume(
    volume: &gio::Volume,
    parent: Option<&gtk::Window>,
    done: impl FnOnce(Result<gio::File, glib::Error>) + 'static,
) {
    let operation = gtk::MountOperation::new(parent);
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
    format!("smb://{path}")
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

/// The mapped drive a mount is, if it's one.
pub fn mapped_uri_of(mount: &gio::Mount) -> Option<String> {
    let root = mount.root();
    mapped_drives().into_iter().find(|uri| {
        let file = gio::File::for_uri(uri);
        root.equal(&file) || file.has_prefix(&root)
    })
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
        (Some(share), "smb") => format!("{share} (\\\\{host})"),
        (Some(share), _) => format!("{share} ({host})"),
        (None, _) => host,
    }
}

/// Connects to a network folder, asking for a sign-in if the server wants
/// one (GTK's dialog can remember the password for later sign-ins).
pub async fn connect(uri: &str, parent: Option<&gtk::Window>) -> Result<gio::File, glib::Error> {
    let file = gio::File::for_uri(uri);
    let operation = gtk::MountOperation::new(parent);
    match file
        .mount_enclosing_volume_future(gio::MountMountFlags::NONE, Some(&operation))
        .await
    {
        Ok(()) => Ok(file),
        Err(error) if error.matches(gio::IOErrorEnum::AlreadyMounted) => Ok(file),
        Err(error) => Err(error),
    }
}

/// Whether a network folder is connected now.
pub fn is_connected(uri: &str) -> bool {
    gio::File::for_uri(uri)
        .find_enclosing_mount(gio::Cancellable::NONE)
        .is_ok()
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
    let monitor = gio::VolumeMonitor::get();
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
            if let Ok(root) = root_of(&volume) {
                return Ok(root);
            }
            let (sender, receiver) = crate::dialogs::oneshot();
            mount_volume(&volume, parent.as_ref(), move |result| {
                sender.send(result);
            });
            return match receiver.await {
                Ok(root) => Ok(root),
                // The desktop may have mounted it first.
                Err(error) => root_of(&volume).map_err(|_| crate::dialogs::describe(&error)),
            };
        }
        glib::timeout_future(Duration::from_millis(200)).await;
    }
    Err(format!(
        "The image became {device}, but it has no file system to open."
    ))
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
}
