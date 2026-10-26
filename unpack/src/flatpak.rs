//! Flatpak files: a .flatpak bundle holds the app itself, a .flatpakref
//! names an app in a repository, and a .flatpakrepo adds a repository.
//! Apps install for the account that opens them (`flatpak install --user`),
//! which needs no password. A bundle's runtime, the platform Flatpak apps
//! share, usually comes from Flathub; Unpack sets Flathub up for the account
//! first when the runtime isn't installed yet.

use std::io::Read;
use std::path::Path;
use std::process::Command;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::util;

/// Flathub's own .flatpakrepo, which the package installs
/// (PATRONE_UNPACK_FLATHUB replaces it when running from the source tree).
const FLATHUB_FILE: &str = "/usr/share/patrone-unpack/flathub.flatpakrepo";
const FLATHUB_URL: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// .flatpakref
    Ref,
    /// .flatpak
    Bundle,
    /// .flatpakrepo
    Repo,
}

/// Something an app may use beyond its own sandbox, in plain words.
#[derive(Debug, Clone, PartialEq)]
pub struct Permission {
    pub text: String,
    /// Worth a second look: the whole home folder, the system, devices,
    /// running commands outside the sandbox.
    pub warning: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Info {
    /// The app's ID (org.gimp.GIMP), or the name a repository gets.
    pub id: String,
    pub title: String,
    pub version: String,
    pub developer: String,
    pub branch: String,
    pub summary: String,
    pub description: String,
    /// The repository's address.
    pub url: Option<String>,
    pub homepage: Option<String>,
    pub is_runtime: bool,
    /// For a bundle: the runtime it needs (org.gnome.Platform/x86_64/50).
    pub needs: Option<String>,
    /// For a bundle: where that runtime comes from (a .flatpakrepo address).
    pub runtime_repo: Option<String>,
    pub installed_size: Option<u64>,
    /// For a bundle: what it may use (None when the file doesn't say).
    pub permissions: Option<Vec<Permission>>,
    /// For a bundle: its icon, a PNG.
    pub icon: Option<Vec<u8>>,
}

pub fn read(path: &Path, kind: Kind) -> Result<Info, String> {
    match kind {
        Kind::Ref => read_keyfile(path, "Flatpak Ref", kind),
        Kind::Repo => read_keyfile(path, "Flatpak Repo", kind),
        Kind::Bundle => read_bundle(path),
    }
}

fn read_keyfile(path: &Path, group: &str, kind: Kind) -> Result<Info, String> {
    let file = glib::KeyFile::new();
    file.load_from_file(path, glib::KeyFileFlags::NONE)
        .map_err(|error| format!("This Flatpak file can't be read: {error}"))?;
    let value = |key: &str| {
        file.string(group, key)
            .ok()
            .map(|value| value.to_string())
            .filter(|value| !value.is_empty())
    };
    let url = value("Url");
    if url.is_none() {
        return Err("This Flatpak file doesn't say where the software comes from.".into());
    }
    let id = match kind {
        Kind::Repo => repository_name(path),
        _ => value("Name").ok_or("This Flatpak file doesn't name an app.")?,
    };
    Ok(Info {
        title: value("Title").unwrap_or_else(|| id.clone()),
        id,
        branch: value("Branch").unwrap_or_default(),
        summary: value("Comment").unwrap_or_default(),
        description: value("Description").unwrap_or_default(),
        url,
        homepage: value("Homepage"),
        is_runtime: file.boolean(group, "IsRuntime").unwrap_or(false),
        runtime_repo: value("RuntimeRepo"),
        ..Info::default()
    })
}

/// The name a .flatpakrepo's repository gets: its file name, as Flatpak's
/// own documentation does it ("flathub.flatpakrepo" adds "flathub").
pub fn repository_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let name: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let name = name.trim_matches(['-', '.']).to_string();
    if name.is_empty() {
        "repository".into()
    } else {
        name
    }
}

/// The GVariant type of a bundle: an OSTree static delta superblock, whose
/// first part holds Flatpak's details about the app (and the delta's parts).
const BUNDLE_TYPE: &str = "(a{sv}tayay(a{sv}aya(say)sstayay)a(uayttay)a(yaytt))";

fn read_bundle(path: &Path) -> Result<Info, String> {
    let data = std::fs::read(path).map_err(|error| format!("The bundle can't be read: {error}"))?;
    parse_bundle(data)
}

pub fn parse_bundle(data: Vec<u8>) -> Result<Info, String> {
    let not_a_bundle = || "This file isn't a Flatpak bundle.".to_string();
    let variant_type = glib::VariantTy::new(BUNDLE_TYPE).map_err(|_| not_a_bundle())?;
    let bundle = glib::Variant::from_data_with_type(data, variant_type);
    let details =
        glib::VariantDict::new(Some(&bundle.try_child_value(0).ok_or_else(not_a_bundle)?));
    let string = |key: &str| {
        details
            .lookup_value(key, Some(glib::VariantTy::STRING))
            .and_then(|value| value.str().map(str::to_string))
            .filter(|value| !value.is_empty())
    };
    let bytes = |key: &str| {
        details
            .lookup_value(key, Some(glib::VariantTy::BYTE_STRING))
            .and_then(|value| value.fixed_array::<u8>().ok().map(<[u8]>::to_vec))
    };
    // "app/org.gimp.GIMP/x86_64/stable"
    let full_ref = string("ref").ok_or_else(not_a_bundle)?;
    let parts: Vec<&str> = full_ref.split('/').collect();
    let [kind, id, _arch, branch] = parts[..] else {
        return Err(not_a_bundle());
    };
    let mut info = Info {
        id: id.to_string(),
        title: id.to_string(),
        branch: branch.to_string(),
        is_runtime: kind == "runtime",
        url: string("origin"),
        runtime_repo: string("runtime-repo"),
        // Written big-endian, as OSTree's other numbers are
        installed_size: details
            .lookup_value("installed-size", Some(glib::VariantTy::UINT64))
            .and_then(|value| value.get::<u64>())
            .map(u64::from_be),
        icon: bytes("icon-128").or_else(|| bytes("icon-64")),
        ..Info::default()
    };
    if let Some(metadata) = string("metadata") {
        let file = glib::KeyFile::new();
        if file
            .load_from_data(&metadata, glib::KeyFileFlags::NONE)
            .is_ok()
        {
            let group = if info.is_runtime {
                "Runtime"
            } else {
                "Application"
            };
            info.needs = file
                .string(group, "runtime")
                .ok()
                .map(|runtime| runtime.to_string());
            info.permissions = Some(permissions(&file));
        }
    }
    if let Some(gzipped) = bytes("appdata")
        && let Some(xml) = gunzip(&gzipped)
    {
        if let Some(name) = element(&xml, "name") {
            info.title = name;
        }
        info.summary = element(&xml, "summary").unwrap_or_default();
        info.description = element(&xml, "description").unwrap_or_default();
        info.version = attribute(&xml, "release", "version").unwrap_or_default();
        info.developer = xml
            .find("<developer")
            .and_then(|at| element(&xml[at..], "name"))
            .or_else(|| element(&xml, "developer_name"))
            .unwrap_or_default();
        info.homepage = xml
            .find("<url type=\"homepage\">")
            .and_then(|at| element(&xml[at..].replacen(" type=\"homepage\"", "", 1), "url"));
    }
    Ok(info)
}

fn gunzip(data: &[u8]) -> Option<String> {
    let input = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(data));
    let decompressor = gio::ZlibDecompressor::new(gio::ZlibCompressorFormat::Gzip);
    let stream = gio::ConverterInputStream::new(&input, &decompressor);
    let mut text = String::new();
    stream.into_read().read_to_string(&mut text).ok()?;
    Some(text)
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// The text of the first <NAME> element without a language (AppStream
/// repeats names and summaries for each translation), tags removed.
fn element(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&close)?;
    let inner = &xml[start..end];
    // Paragraphs and list items on lines of their own, then no tags.
    let inner = inner
        .replace("</p>", "\n\n")
        .replace("<li>", "• ")
        .replace("</li>", "\n");
    let mut text = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = unescape(&text);
    // Lines rejoined into paragraphs: AppStream wraps its text anywhere.
    let mut out = String::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push_str("\n\n");
            }
        } else {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push(if line.starts_with('•') { '\n' } else { ' ' });
            }
            out.push_str(line);
        }
    }
    let text = out.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// An attribute of the first <ELEMENT ...> tag: the first release's version.
fn attribute(xml: &str, element: &str, name: &str) -> Option<String> {
    let start = xml.find(&format!("<{element} "))?;
    let tag = &xml[start..start + xml[start..].find('>')?];
    let key = format!(" {name}=\"");
    let at = tag.find(&key)? + key.len();
    let value = &tag[at..at + tag[at..].find('"')?];
    (!value.is_empty()).then(|| unescape(value))
}

/// What an app's metadata lets it use, in plain words, the way an app
/// store lists it. Its own files, the screen (Wayland, or X11 as a
/// fallback), the graphics card and its own settings aren't listed: every
/// app has those.
pub fn permissions(metadata: &glib::KeyFile) -> Vec<Permission> {
    let list = |group: &str, key: &str| -> Vec<String> {
        metadata
            .string(group, key)
            .map(|value| {
                value
                    .split(';')
                    .map(str::trim)
                    .filter(|item| !item.is_empty() && !item.starts_with('!'))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    let mut add = |text: &str, warning: bool| {
        out.push(Permission {
            text: text.to_string(),
            warning,
        })
    };
    let shared = list("Context", "shared");
    let sockets = list("Context", "sockets");
    let devices = list("Context", "devices");
    let features = list("Context", "features");
    if shared.iter().any(|item| item == "network") {
        add("The internet and your network", false);
    }
    for filesystem in list("Context", "filesystems") {
        let (place, mode) = filesystem
            .split_once(':')
            .map_or((filesystem.as_str(), ""), |(place, mode)| (place, mode));
        let read_only = if mode == "ro" { " (read only)" } else { "" };
        let (text, warning) = match place {
            "host" => ("All your files and the system's".to_string(), true),
            "host-os" => ("The system's programs and libraries".to_string(), true),
            "host-etc" => ("The system's settings (/etc)".to_string(), true),
            "home" | "~" => ("Your whole home folder".to_string(), true),
            place => (folder_name(place), false),
        };
        add(&format!("{text}{read_only}"), warning);
    }
    if sockets.iter().any(|item| item == "x11")
        && !sockets.iter().any(|item| item == "fallback-x11")
    {
        add(
            "X11, an older display system that lets apps see other windows",
            true,
        );
    }
    if sockets.iter().any(|item| item == "pulseaudio") {
        add("Sound: playing it, and the microphone", false);
    }
    if sockets.iter().any(|item| item == "cups") {
        add("Printers", false);
    }
    if sockets.iter().any(|item| item == "pcsc") {
        add("Smart cards", false);
    }
    if sockets.iter().any(|item| item == "ssh-auth") {
        add("Your SSH keys", true);
    }
    if sockets.iter().any(|item| item == "gpg-agent") {
        add("Your GPG keys", true);
    }
    if sockets.iter().any(|item| item == "session-bus") {
        add("Every service in your session", true);
    }
    if sockets.iter().any(|item| item == "system-bus") {
        add("Every system service", true);
    }
    if devices.iter().any(|item| item == "all") {
        add(
            "All devices, such as webcams, USB drives and game controllers",
            true,
        );
    } else {
        if devices.iter().any(|item| item == "input") {
            add("Game controllers and other input devices", false);
        }
        if devices.iter().any(|item| item == "usb") {
            add("USB devices", false);
        }
        if devices.iter().any(|item| item == "kvm") {
            add("Virtual machines (KVM)", false);
        }
    }
    if features.iter().any(|item| item == "bluetooth") {
        add("Bluetooth", false);
    }
    if features.iter().any(|item| item == "devel") {
        add("Debugging other programs", true);
    }
    let session_names: Vec<String> = metadata
        .keys("Session Bus Policy")
        .map(|keys| keys.iter().map(|key| key.to_string()).collect())
        .unwrap_or_default();
    if session_names
        .iter()
        .any(|name| name == "org.freedesktop.Flatpak")
    {
        add("Running commands outside its sandbox", true);
    }
    if session_names
        .iter()
        .any(|name| name == "org.freedesktop.secrets")
    {
        add("Your saved passwords (the keyring)", true);
    }
    let system_names = metadata
        .keys("System Bus Policy")
        .map(|keys| keys.len())
        .unwrap_or_default();
    if system_names > 0 {
        add(
            &format!(
                "{system_names} system {}",
                if system_names == 1 {
                    "service"
                } else {
                    "services"
                }
            ),
            false,
        );
    }
    out
}

/// A folder named in an app's file system access, in plain words.
fn folder_name(place: &str) -> String {
    let known = [
        ("xdg-desktop", "Your Desktop folder"),
        ("xdg-documents", "Your Documents folder"),
        ("xdg-download", "Your Downloads folder"),
        ("xdg-music", "Your Music folder"),
        ("xdg-pictures", "Your Pictures folder"),
        ("xdg-videos", "Your Videos folder"),
        ("xdg-public-share", "Your Public folder"),
        ("xdg-templates", "Your Templates folder"),
        ("xdg-config", "Your apps' settings"),
        ("xdg-cache", "Your apps' caches"),
        ("xdg-data", "Your apps' data"),
        ("xdg-run", "Your session's runtime folder"),
    ];
    let (base, rest) = place.split_once('/').map_or((place, ""), |(b, r)| (b, r));
    match known.iter().find(|(name, _)| *name == base) {
        Some((_, text)) if rest.is_empty() => text.to_string(),
        Some((_, text)) => format!("{}: {rest}", text.trim_end_matches(" folder")),
        None => format!("The folder {place}"),
    }
}

/// Whether Flatpak itself is installed.
pub fn available() -> bool {
    util::have("flatpak")
}

/// Whether the app (or runtime) is installed, for this account or everyone.
pub fn installed(id: &str) -> bool {
    Command::new("flatpak")
        .args(["info", id])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Whether a runtime ("org.freedesktop.Platform/x86_64/25.08") is
/// installed, for this account or everyone.
pub fn runtime_installed(runtime: &str) -> bool {
    util::output("flatpak", &["list", "--runtime", "--columns=ref"]).is_some_and(|text| {
        text.lines().any(|line| {
            let line = line.trim();
            let line = line.strip_prefix("runtime/").unwrap_or(line);
            line == runtime
        })
    })
}

/// Whether the account has a repository of that name.
pub fn has_repository(name: &str) -> bool {
    util::output("flatpak", &["remotes", "--user", "--columns=name"])
        .is_some_and(|text| text.lines().any(|line| line.trim() == name))
}

/// Whether the account (or the system) has Flathub set up.
pub fn has_flathub() -> bool {
    util::output("flatpak", &["remotes", "--columns=name,url"]).is_some_and(|text| {
        text.lines().any(|line| {
            line.split_whitespace().next() == Some("flathub") || line.contains("dl.flathub.org")
        })
    })
}

/// Whether installing the bundle needs Flathub added first: its runtime
/// isn't installed, it either names Flathub for its runtime or names no
/// repository at all, and Flathub isn't set up.
pub fn needs_flathub(info: &Info) -> bool {
    let Some(runtime) = &info.needs else {
        return false;
    };
    let from_flathub = info
        .runtime_repo
        .as_deref()
        .is_none_or(|repo| repo.contains("dl.flathub.org"));
    from_flathub && !runtime_installed(runtime) && !has_flathub()
}

/// The commands that install the file for this account (or add the
/// repository), in order.
pub fn install_commands(
    kind: Kind,
    path: &Path,
    info: &Info,
    installed: bool,
    add_flathub: bool,
) -> Vec<Vec<String>> {
    let path = path.to_string_lossy().into_owned();
    let words = |words: &[&str]| {
        words
            .iter()
            .map(|word| word.to_string())
            .collect::<Vec<_>>()
    };
    let mut commands = Vec::new();
    if add_flathub {
        let file = std::env::var("PATRONE_UNPACK_FLATHUB").unwrap_or_else(|_| FLATHUB_FILE.into());
        let source = if Path::new(&file).is_file() {
            file
        } else {
            FLATHUB_URL.into()
        };
        let mut command = words(&[
            "flatpak",
            "remote-add",
            "--user",
            "--if-not-exists",
            "flathub",
        ]);
        command.push(source);
        commands.push(command);
    }
    let mut command = match kind {
        Kind::Ref => words(&["flatpak", "install", "--user", "--assumeyes", "--from"]),
        Kind::Bundle => words(&["flatpak", "install", "--user", "--assumeyes", "--bundle"]),
        Kind::Repo => {
            let mut command = words(&[
                "flatpak",
                "remote-add",
                "--user",
                "--if-not-exists",
                "--from",
            ]);
            command.push(info.id.clone());
            command
        }
    };
    if installed && kind != Kind::Repo {
        command.insert(3, "--reinstall".into());
    }
    command.push(path);
    commands.push(command);
    commands
}

/// The runtime's name and branch, for people: "org.freedesktop.Platform 25.08".
pub fn runtime_label(runtime: &str) -> String {
    let parts: Vec<&str> = runtime.split('/').collect();
    match parts[..] {
        [name, _arch, branch] => format!("{name} {branch}"),
        _ => runtime.to_string(),
    }
}

/// Whether the session already looks in Flatpak's folders for apps; if not
/// (Flatpak was only just installed), new apps reach Start at the next
/// sign-in.
pub fn session_sees_flatpak_apps() -> bool {
    std::env::var("XDG_DATA_DIRS").is_ok_and(|dirs| dirs.contains("flatpak/exports/share"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatpakref() {
        let dir =
            std::env::temp_dir().join(format!("patrone-unpack-flatpak-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("org.example.Notes.flatpakref");
        std::fs::write(
            &path,
            "[Flatpak Ref]\nName=org.example.Notes\nBranch=stable\nTitle=Notes\n\
             IsRuntime=false\nUrl=https://dl.example.org/repo/\n\
             Homepage=https://example.org\nComment=Take notes\n\
             RuntimeRepo=https://dl.example.org/repo/example.flatpakrepo\n",
        )
        .unwrap();
        let info = read(&path, Kind::Ref).unwrap();
        assert_eq!(info.id, "org.example.Notes");
        assert_eq!(info.title, "Notes");
        assert_eq!(info.branch, "stable");
        assert_eq!(info.summary, "Take notes");
        assert_eq!(info.url.as_deref(), Some("https://dl.example.org/repo/"));
        assert!(!info.is_runtime);
        assert_eq!(
            install_commands(Kind::Ref, &path, &info, false, false),
            [vec![
                "flatpak",
                "install",
                "--user",
                "--assumeyes",
                "--from",
                path.to_str().unwrap()
            ]]
        );
        assert_eq!(
            install_commands(Kind::Ref, &path, &info, true, false)[0][..4],
            ["flatpak", "install", "--user", "--reinstall"]
        );

        let repo = dir.join("Flat Hub.flatpakrepo");
        std::fs::write(
            &repo,
            "[Flatpak Repo]\nTitle=Flathub\nUrl=https://dl.flathub.org/repo/\n",
        )
        .unwrap();
        let info = read(&repo, Kind::Repo).unwrap();
        assert_eq!(info.id, "flat-hub");
        assert_eq!(info.title, "Flathub");
        assert_eq!(
            install_commands(Kind::Repo, &repo, &info, false, false)[0][2..6],
            ["--user", "--if-not-exists", "--from", "flat-hub"]
        );

        std::fs::write(&path, "[Flatpak Ref]\nName=org.example.Notes\n").unwrap();
        assert!(read(&path, Kind::Ref).is_err(), "no Url");
        std::fs::write(&path, "not a key file").unwrap();
        assert!(read(&path, Kind::Ref).is_err());
    }

    /// A bundle laid out as `flatpak build-bundle` does, with the parts
    /// Unpack reads: the same keys as a real one (PdfCraft 0.9.0's).
    #[test]
    fn bundle() {
        let appdata = "<?xml version=\"1.0\"?><components><component type=\"desktop-application\">\
            <id>org.example.Notes</id><name xml:lang=\"de\">Notizen</name><name>Notes &amp; Lists</name>\
            <summary>Take notes</summary><description>\n  <p>\n  One line\n  wrapped.\n  </p><ul><li>Two</li><li>Three</li></ul></description>\
            <developer id=\"org.example\"><name>Example LLC</name></developer>\
            <url type=\"bugtracker\">https://example.org/bugs</url><url type=\"homepage\">https://example.org</url>\
            <releases><release type=\"stable\" version=\"0.9.0\" timestamp=\"1792281600\"/><release version=\"0.8.0\"/></releases>\
            </component></components>";
        let compressor = gio::ZlibCompressor::new(gio::ZlibCompressorFormat::Gzip, -1);
        let stream = gio::ConverterInputStream::new(
            &gio::MemoryInputStream::from_bytes(&glib::Bytes::from(appdata.as_bytes())),
            &compressor,
        );
        let mut gzipped = Vec::new();
        stream.into_read().read_to_end(&mut gzipped).unwrap();

        let details = glib::VariantDict::new(None);
        details.insert_value("flatpak", &4u32.to_variant());
        details.insert_value("ref", &"app/org.example.Notes/x86_64/stable".to_variant());
        details.insert_value(
            "runtime-repo",
            &"https://dl.flathub.org/repo/flathub.flatpakrepo".to_variant(),
        );
        details.insert_value("installed-size", &12_345_678u64.to_be().to_variant());
        details.insert_value(
            "metadata",
            &"[Application]\nname=org.example.Notes\nruntime=org.freedesktop.Platform/x86_64/25.08\n\n\
              [Context]\nshared=ipc;\nsockets=x11;wayland;fallback-x11;\ndevices=dri;\n\
              filesystems=xdg-documents;xdg-download;\n"
                .to_variant(),
        );
        details.insert_value("appdata", &glib::Variant::array_from_fixed_array(&gzipped));
        details.insert_value(
            "icon-128",
            &glib::Variant::array_from_fixed_array(b"\x89PNG\r\n\x1a\n"),
        );
        let empty_dict = glib::VariantDict::new(None).end();
        let bytes = |data: &[u8]| glib::Variant::array_from_fixed_array(data);
        let empty = |ty: &str| {
            glib::Variant::array_from_iter_with_type(
                glib::VariantTy::new(ty).unwrap(),
                std::iter::empty::<glib::Variant>(),
            )
        };
        let superblock = glib::Variant::tuple_from_iter([
            details.end(),
            0u64.to_variant(),
            bytes(&[]),
            bytes(&[]),
            glib::Variant::tuple_from_iter([
                empty_dict,
                bytes(&[]),
                empty("(say)"),
                "".to_variant(),
                "".to_variant(),
                0u64.to_variant(),
                bytes(&[]),
                bytes(&[]),
            ]),
            empty("(uayttay)"),
            empty("(yaytt)"),
        ]);
        assert_eq!(superblock.type_().as_str(), BUNDLE_TYPE);
        let info = parse_bundle(superblock.data().to_vec()).unwrap();
        assert_eq!(info.id, "org.example.Notes");
        assert_eq!(info.title, "Notes & Lists");
        assert_eq!(info.version, "0.9.0");
        assert_eq!(info.developer, "Example LLC");
        assert_eq!(info.summary, "Take notes");
        assert_eq!(info.description, "One line wrapped.\n\n• Two\n• Three");
        assert_eq!(info.homepage.as_deref(), Some("https://example.org"));
        assert_eq!(info.branch, "stable");
        assert_eq!(info.installed_size, Some(12_345_678));
        assert_eq!(
            info.needs.as_deref(),
            Some("org.freedesktop.Platform/x86_64/25.08")
        );
        assert_eq!(
            info.runtime_repo.as_deref(),
            Some("https://dl.flathub.org/repo/flathub.flatpakrepo")
        );
        assert_eq!(info.icon.as_deref(), Some(&b"\x89PNG\r\n\x1a\n"[..]));
        let texts: Vec<String> = info
            .permissions
            .unwrap()
            .into_iter()
            .map(|permission| permission.text)
            .collect();
        assert_eq!(texts, ["Your Documents folder", "Your Downloads folder"]);
        assert_eq!(
            runtime_label("org.freedesktop.Platform/x86_64/25.08"),
            "org.freedesktop.Platform 25.08"
        );
        assert!(parse_bundle(b"garbage".to_vec()).is_err());
    }

    #[test]
    fn permission_words() {
        let file = glib::KeyFile::new();
        file.load_from_data(
            "[Application]\nname=x\n\n[Context]\nshared=network;ipc;\n\
             sockets=x11;pulseaudio;ssh-auth;\ndevices=all;\nfeatures=bluetooth;\n\
             filesystems=home;xdg-music:ro;/media;xdg-config/app;!xdg-videos;\n\n\
             [Session Bus Policy]\norg.freedesktop.Flatpak=talk\n\n\
             [System Bus Policy]\norg.freedesktop.UPower=talk\n",
            glib::KeyFileFlags::NONE,
        )
        .unwrap();
        let found: Vec<(String, bool)> = permissions(&file)
            .into_iter()
            .map(|permission| (permission.text, permission.warning))
            .collect();
        let expected = [
            ("The internet and your network", false),
            ("Your whole home folder", true),
            ("Your Music folder (read only)", false),
            ("The folder /media", false),
            ("Your apps' settings: app", false),
            (
                "X11, an older display system that lets apps see other windows",
                true,
            ),
            ("Sound: playing it, and the microphone", false),
            ("Your SSH keys", true),
            (
                "All devices, such as webcams, USB drives and game controllers",
                true,
            ),
            ("Bluetooth", false),
            ("Running commands outside its sandbox", true),
            ("1 system service", false),
        ];
        let expected: Vec<(String, bool)> = expected
            .iter()
            .map(|(text, warning)| (text.to_string(), *warning))
            .collect();
        assert_eq!(found, expected);
    }
}
