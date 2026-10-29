//! The special places: the home folder, the user folders (Documents,
//! Downloads...), the system drive, mounted drives, the Recycle Bin (GIO's
//! trash) and Network.

use gtk::{gio, glib, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Special {
    Home,
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
    Templates,
    Public,
    /// The system drive, `/`.
    Root,
    Trash,
    Network,
}

pub const USER_FOLDERS: &[(Special, glib::UserDirectory)] = &[
    (Special::Desktop, glib::UserDirectory::Desktop),
    (Special::Downloads, glib::UserDirectory::Downloads),
    (Special::Documents, glib::UserDirectory::Documents),
    (Special::Pictures, glib::UserDirectory::Pictures),
    (Special::Music, glib::UserDirectory::Music),
    (Special::Videos, glib::UserDirectory::Videos),
    (Special::Templates, glib::UserDirectory::Templates),
    (Special::Public, glib::UserDirectory::PublicShare),
];

pub fn home() -> gio::File {
    gio::File::for_path(glib::home_dir())
}

pub fn root() -> gio::File {
    gio::File::for_path("/")
}

pub fn trash() -> gio::File {
    gio::File::for_uri("trash:///")
}

pub fn network() -> gio::File {
    gio::File::for_uri("network:///")
}

/// A user folder, unless xdg-user-dirs points it at the home folder (unset).
pub fn user_folder(special: Special) -> Option<gio::File> {
    let (_, directory) = USER_FOLDERS.iter().find(|(kind, _)| *kind == special)?;
    let path = glib::user_special_dir(*directory)?;
    (path != glib::home_dir()).then(|| gio::File::for_path(path))
}

pub fn special_of(file: &gio::File) -> Option<Special> {
    match file.uri_scheme().as_deref() {
        Some("trash") => {
            return (file.uri().as_str().trim_end_matches('/') == "trash:")
                .then_some(Special::Trash);
        }
        Some("network") => {
            return (file.uri().as_str().trim_end_matches('/') == "network:")
                .then_some(Special::Network);
        }
        _ => {}
    }
    let path = file.path()?;
    if path == std::path::Path::new("/") {
        return Some(Special::Root);
    }
    if path == glib::home_dir() {
        return Some(Special::Home);
    }
    USER_FOLDERS.iter().find_map(|(special, directory)| {
        (glib::user_special_dir(*directory).as_deref() == Some(path.as_path())).then_some(*special)
    })
}

pub fn is_trash(file: &gio::File) -> bool {
    file.has_uri_scheme("trash")
}

/// Whether `file` is the Recycle Bin itself, not something inside it.
pub fn is_trash_root(file: &gio::File) -> bool {
    special_of(file) == Some(Special::Trash)
}

/// The system drive's name, as Computer shows it.
pub fn root_name() -> String {
    "System drive (/)".to_owned()
}

/// The name shown for a folder in tabs, the address bar and the
/// navigation pane.
pub fn display_name(file: &gio::File) -> String {
    match special_of(file) {
        Some(Special::Trash) => return "Recycle Bin".into(),
        Some(Special::Network) => return "Network".into(),
        Some(Special::Root) => return root_name(),
        _ => {}
    }
    if crate::drives::is_own_drive_folder(file) {
        return "Drive mounts".into();
    }
    if let Some(mount) = mount_root_of(file) {
        return mount.name().into();
    }
    if !file.is_native() && file.parent().is_none() {
        // The root of a server: smb://host/ shows as "host".
        if let Some(host) = glib::Uri::parse(&file.uri(), glib::UriFlags::NONE)
            .ok()
            .and_then(|uri| uri.host())
        {
            return host.into();
        }
    }
    match file.basename() {
        Some(name) if name.as_os_str() != "/" => {
            if file.is_native() {
                name.to_string_lossy().into_owned()
            } else {
                // Remote names arrive URI-escaped.
                glib::Uri::unescape_string(&name.to_string_lossy(), None::<&str>)
                    .map(String::from)
                    .unwrap_or_else(|| name.to_string_lossy().into_owned())
            }
        }
        _ => file.parse_name().into(),
    }
}

/// User-visible mounted drives, other than the system drive.
pub fn mounts() -> Vec<gio::Mount> {
    crate::drives::monitor()
        .mounts()
        .into_iter()
        .filter(|mount| !mount.is_shadowed())
        .filter(|mount| mount.root().path().as_deref() != Some(std::path::Path::new("/")))
        .collect()
}

/// The mount whose root `file` is, if it's one.
pub fn mount_root_of(file: &gio::File) -> Option<gio::Mount> {
    mounts().into_iter().find(|mount| mount.root().equal(file))
}

/// The mount that holds `file`, if it isn't on the system drive.
pub fn mount_containing(file: &gio::File) -> Option<gio::Mount> {
    mounts()
        .into_iter()
        .filter(|mount| {
            let root = mount.root();
            root.equal(file) || file.has_prefix(&root)
        })
        .max_by_key(|mount| mount.root().uri().len())
}
