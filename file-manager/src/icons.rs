//! Which of Patrone's color icons a file, folder, drive or place gets, and
//! loading them at the size a view needs.

use std::cell::RefCell;
use std::collections::HashMap;

use gtk::{gio, glib, prelude::*};

use crate::places::{self, Special};
use crate::util;

const RESOURCE: &str = "resource:///org/patrone/FileManager/icons";

thread_local! {
    static CACHE: RefCell<HashMap<(String, i32, i32), gtk::IconPaintable>> = RefCell::default();
}

/// An icon from the app's resources, rendered for `size` logical pixels.
pub fn paintable(name: &str, size: i32, scale: i32) -> gtk::IconPaintable {
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry((name.to_owned(), size, scale))
            .or_insert_with(|| {
                let file = gio::File::for_uri(&format!("{RESOURCE}/{name}.svg"));
                gtk::IconPaintable::for_file(&file, size, scale.max(1))
            })
            .clone()
    })
}

/// A `gtk::Image` showing a color icon.
pub fn image(name: &str, size: i32) -> gtk::Image {
    let image = gtk::Image::new();
    set_image(&image, name, size);
    image
}

pub fn set_image(image: &gtk::Image, name: &str, size: i32) {
    let scale = image.scale_factor().max(1);
    image.set_paintable(Some(&paintable(name, size, scale)));
    image.set_pixel_size(size);
}

/// What kind of file this is, which picks its icon and the Filter menu's group.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Folder,
    Text,
    Document,
    Spreadsheet,
    Presentation,
    Pdf,
    Image,
    Audio,
    Video,
    Archive,
    Code,
    Program,
    Font,
    DiscImage,
    Package,
    Web,
    Other,
}

pub fn kind_of(content_type: &str, is_dir: bool) -> Kind {
    if is_dir {
        return Kind::Folder;
    }
    let mime = gio::content_type_get_mime_type(content_type)
        .map(String::from)
        .unwrap_or_else(|| content_type.to_owned());
    let is_a = |parent: &str| gio::content_type_is_a(content_type, parent);
    match mime.as_str() {
        "application/pdf" | "application/x-pdf" => Kind::Pdf,
        "text/html" | "application/xhtml+xml" | "text/xml" | "application/xml" => Kind::Web,
        "application/x-iso9660-image"
        | "application/x-cd-image"
        | "application/x-raw-disk-image" => Kind::DiscImage,
        "application/vnd.debian.binary-package"
        | "application/x-rpm"
        | "application/x-redhat-package-manager"
        | "application/vnd.flatpak"
        | "application/vnd.snap"
        | "application/x-iso9660-appimage"
        | "application/vnd.appimage" => Kind::Package,
        "application/x-executable"
        | "application/x-sharedlib"
        | "application/x-pie-executable"
        | "application/x-shellscript"
        | "application/x-desktop"
        | "application/x-msdownload"
        | "application/x-ms-dos-executable" => Kind::Program,
        m if m.starts_with("image/") => Kind::Image,
        m if m.starts_with("audio/") => Kind::Audio,
        m if m.starts_with("video/") => Kind::Video,
        m if m.starts_with("font/") || m.contains("font") => Kind::Font,
        m if m.contains("spreadsheet") || m.contains("excel") || m == "text/csv" => {
            Kind::Spreadsheet
        }
        m if m.contains("presentation") || m.contains("powerpoint") => Kind::Presentation,
        m if m.contains("wordprocessing")
            || m.contains("msword")
            || m.contains("opendocument.text")
            || m == "application/rtf"
            || m == "text/rtf" =>
        {
            Kind::Document
        }
        m if is_archive(m) => Kind::Archive,
        m if (m.starts_with("text/")
            && !matches!(m, "text/plain" | "text/markdown" | "text/x-readme"))
            || m == "application/json"
            || m == "application/javascript"
            || m == "text/javascript"
            || m == "application/x-perl"
            || m == "text/css"
            || m == "application/toml"
            || m == "application/x-yaml"
            || m == "application/yaml" =>
        {
            Kind::Code
        }
        _ if is_a("text/plain") => Kind::Text,
        _ => Kind::Other,
    }
}

pub fn is_archive(mime: &str) -> bool {
    matches!(
        mime,
        "application/zip"
            | "application/x-zip-compressed"
            | "application/x-tar"
            | "application/x-compressed-tar"
            | "application/x-bzip-compressed-tar"
            | "application/x-xz-compressed-tar"
            | "application/x-zstd-compressed-tar"
            | "application/gzip"
            | "application/x-gzip"
            | "application/x-7z-compressed"
            | "application/vnd.rar"
            | "application/x-rar"
            | "application/x-bzip2"
            | "application/x-xz"
            | "application/zstd"
    )
}

/// The large icon for a kind of file.
pub fn for_kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Folder => "pfm-folder",
        Kind::Text => "pfm-file-text",
        Kind::Document => "pfm-file-document",
        Kind::Spreadsheet => "pfm-file-spreadsheet",
        Kind::Presentation => "pfm-file-presentation",
        Kind::Pdf => "pfm-file-pdf",
        Kind::Image => "pfm-file-image",
        Kind::Audio => "pfm-file-audio",
        Kind::Video => "pfm-file-video",
        Kind::Archive => "pfm-folder-zip",
        Kind::Code => "pfm-file-code",
        Kind::Program => "pfm-file-script",
        Kind::Font => "pfm-file-font",
        Kind::DiscImage => "pfm-file-disc",
        Kind::Package => "pfm-file-package",
        Kind::Web => "pfm-file-web",
        Kind::Other => "pfm-file",
    }
}

/// The icon for an item in a file list.
pub fn for_info(info: &gio::FileInfo) -> &'static str {
    let is_dir = util::is_dir(info);
    if is_dir {
        if let Some(file) = util::file_of(info) {
            return folder_icon(&file);
        }
        return "pfm-folder";
    }
    for_kind(kind_of(&util::content_type(info), false))
}

/// A folder's icon: the special folders have their own.
pub fn folder_icon(file: &gio::File) -> &'static str {
    match places::special_of(file) {
        Some(Special::Home) => "pfm-folder-user",
        Some(Special::Desktop) => "pfm-folder-desktop",
        Some(Special::Documents) => "pfm-folder-documents",
        Some(Special::Downloads) => "pfm-folder-downloads",
        Some(Special::Music) => "pfm-folder-music",
        Some(Special::Pictures) => "pfm-folder-pictures",
        Some(Special::Videos) => "pfm-folder-videos",
        Some(Special::Templates) => "pfm-folder-templates",
        Some(Special::Public) => "pfm-folder-public",
        Some(Special::Root) => "pfm-drive-system",
        Some(Special::Trash) => trash_icon(),
        Some(Special::Network) => "pfm-network",
        None => {
            if let Some(icon) = system_folder_icon(file) {
                icon
            } else if places::mount_root_of(file).is_some() {
                "pfm-drive"
            } else {
                "pfm-folder"
            }
        }
    }
}

/// Linux's well-known folders, by what they hold, as Windows marks its own
/// folders (Windows, Program Files, Users).
const SYSTEM_FOLDERS: &[(&str, &[&str])] = &[
    ("pfm-folder-settings", &["/etc", "~/.config"]),
    (
        "pfm-folder-programs",
        &[
            "/bin",
            "/sbin",
            "/usr/bin",
            "/usr/sbin",
            "/usr/local/bin",
            "/usr/local/sbin",
            "~/.local/bin",
        ],
    ),
    ("pfm-folder-boot", &["/boot", "/boot/efi"]),
    ("pfm-folder-devices", &["/dev", "/sys"]),
    (
        "pfm-folder-libraries",
        &[
            "/lib",
            "/lib32",
            "/lib64",
            "/libx32",
            "/usr/lib",
            "/usr/lib32",
            "/usr/lib64",
            "/usr/libx32",
            "/usr/local/lib",
        ],
    ),
    ("pfm-folder-users", &["/home"]),
    ("pfm-folder-admin", &["/root"]),
    ("pfm-folder-removable", &["/media"]),
    ("pfm-folder-drive", &["/mnt"]),
    (
        "pfm-folder-package",
        &["/opt", "/usr", "/usr/local", "/snap", "~/snap"],
    ),
    ("pfm-folder-live", &["/proc", "/run"]),
    ("pfm-folder-server", &["/srv", "/var/www"]),
    ("pfm-folder-temp", &["/tmp", "/var/tmp"]),
    (
        "pfm-folder-data",
        &["/var", "/var/lib", "/usr/share", "~/.local/share"],
    ),
    ("pfm-folder-logs", &["/var/log"]),
    ("pfm-folder-cache", &["/var/cache", "~/.cache"]),
    ("pfm-folder-recovered", &["/lost+found"]),
    ("pfm-folder-keys", &["~/.ssh"]),
    ("pfm-folder-disc", &["/cdrom"]),
];

fn system_folder_icon(file: &gio::File) -> Option<&'static str> {
    let path = file.path()?;
    let home = glib::home_dir();
    SYSTEM_FOLDERS.iter().find_map(|(icon, paths)| {
        paths
            .iter()
            .any(|known| match known.strip_prefix("~/") {
                Some(rest) => path == home.join(rest),
                None => path.as_os_str() == *known,
            })
            .then_some(*icon)
    })
}

/// The 16-pixel icon for a folder in the navigation pane, tabs and menus.
pub fn small_folder_icon(file: &gio::File) -> &'static str {
    match places::special_of(file) {
        Some(Special::Home) => "pfm-nav-user",
        Some(Special::Desktop) => "pfm-nav-desktop",
        Some(Special::Documents) => "pfm-nav-documents",
        Some(Special::Downloads) => "pfm-nav-downloads",
        Some(Special::Music) => "pfm-nav-music",
        Some(Special::Pictures) => "pfm-nav-pictures",
        Some(Special::Videos) => "pfm-nav-videos",
        Some(Special::Templates) => "pfm-nav-templates",
        Some(Special::Public) => "pfm-nav-public",
        Some(Special::Root) => "pfm-nav-drive-system",
        Some(Special::Trash) => {
            if trash_icon() == "pfm-trash-full" {
                "pfm-nav-trash-full"
            } else {
                "pfm-nav-trash"
            }
        }
        Some(Special::Network) => "pfm-nav-network",
        None => match places::mount_root_of(file) {
            Some(mount) => small_drive_icon(&mount),
            None => "pfm-nav-folder",
        },
    }
}

/// The large icon for a mounted drive.
pub fn drive_icon(mount: &gio::Mount) -> &'static str {
    match drive_kind(mount) {
        DriveKind::Optical => "pfm-drive-optical",
        DriveKind::Removable => "pfm-drive-usb",
        DriveKind::Network => "pfm-drive-network",
        DriveKind::Fixed => "pfm-drive",
    }
}

pub fn small_drive_icon(mount: &gio::Mount) -> &'static str {
    match drive_kind(mount) {
        DriveKind::Optical => "pfm-nav-drive-optical",
        DriveKind::Removable => "pfm-nav-drive-usb",
        DriveKind::Network => "pfm-nav-drive-network",
        DriveKind::Fixed => "pfm-nav-drive",
    }
}

enum DriveKind {
    Fixed,
    Removable,
    Optical,
    Network,
}

fn drive_kind(mount: &gio::Mount) -> DriveKind {
    let root = mount.root();
    if !root.is_native() {
        return DriveKind::Network;
    }
    let icon = mount.icon();
    let names: Vec<String> = icon
        .downcast_ref::<gio::ThemedIcon>()
        .map(|icon| icon.names().iter().map(|name| name.to_string()).collect())
        .unwrap_or_default();
    let has = |part: &str| names.iter().any(|name| name.contains(part));
    // A disc image mounted from a file (Mount, for an .iso) is a disc too,
    // as on Windows; GIO calls it removable media.
    let disc_image = || {
        mount
            .volume()
            .and_then(|volume| volume.identifier("unix-device"))
            .is_some_and(|device| device.starts_with("/dev/loop"))
            && root
                .query_filesystem_info("filesystem::type", gio::Cancellable::NONE)
                .ok()
                .and_then(|info| info.attribute_string("filesystem::type"))
                .is_some_and(|kind| matches!(kind.as_str(), "isofs" | "iso9660" | "udf"))
    };
    if has("optical") || has("cdrom") || has("dvd") || disc_image() {
        DriveKind::Optical
    } else if has("removable") || has("usb") || has("media-flash") || mount.can_eject() {
        DriveKind::Removable
    } else if has("network") {
        DriveKind::Network
    } else {
        DriveKind::Fixed
    }
}

/// The Recycle Bin's icon: full when anything is in it.
pub fn trash_icon() -> &'static str {
    let trash = places::trash();
    match trash.query_info(
        "trash::item-count",
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    ) {
        Ok(info) if info.attribute_uint32("trash::item-count") > 0 => "pfm-trash-full",
        _ => "pfm-trash",
    }
}
