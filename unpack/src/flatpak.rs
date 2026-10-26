//! Flatpak files: a .flatpakref names an app in a repository, a .flatpak
//! bundle holds the app itself, and a .flatpakrepo adds a repository.
//! Flatpak installs them for everyone; its own polkit rules decide whether
//! that needs an administrator's password.

use std::io::Read;
use std::path::Path;
use std::process::Command;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::util;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// .flatpakref
    Ref,
    /// .flatpak
    Bundle,
    /// .flatpakrepo
    Repo,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Info {
    /// The app's ID (org.gimp.GIMP), or the name a repository gets.
    pub id: String,
    pub title: String,
    pub branch: String,
    pub summary: String,
    pub description: String,
    /// The repository's address.
    pub url: Option<String>,
    pub homepage: Option<String>,
    pub runtime: bool,
    /// For a bundle: the runtime it needs (org.gnome.Platform/x86_64/50).
    pub needs: Option<String>,
    pub installed_size: Option<u64>,
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
        runtime: file.boolean(group, "IsRuntime").unwrap_or(false),
        needs: None,
        installed_size: None,
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
/// first part holds Flatpak's details about the app.
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
        runtime: kind == "runtime",
        url: string("origin"),
        installed_size: details
            .lookup_value("installed-size", Some(glib::VariantTy::UINT64))
            .and_then(|value| value.get::<u64>())
            .map(u64::from_be),
        ..Info::default()
    };
    if let Some(metadata) = string("metadata") {
        let file = glib::KeyFile::new();
        if file
            .load_from_data(&metadata, glib::KeyFileFlags::NONE)
            .is_ok()
        {
            info.needs = file
                .string("Application", "runtime")
                .ok()
                .map(|runtime| runtime.to_string());
        }
    }
    if let Some(appdata) = details.lookup_value("appdata", Some(glib::VariantTy::BYTE_STRING))
        && let Ok(gzipped) = appdata.fixed_array::<u8>()
        && let Some(xml) = gunzip(gzipped)
    {
        if let Some(name) = element(&xml, "name") {
            info.title = name;
        }
        info.summary = element(&xml, "summary").unwrap_or_default();
        info.description = element(&xml, "description").unwrap_or_default();
    }
    Ok(info)
}

fn gunzip(data: &[u8]) -> Option<String> {
    let input = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(data));
    let decompressor = gio::ZlibDecompressor::new(gio::ZlibCompressorFormat::Gzip);
    let stream = gio::ConverterInputStream::new(&input, &decompressor);
    let mut text = String::new();
    stream
        .upcast::<gio::InputStream>()
        .into_read()
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
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
    let text = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let text = lines.join("\n");
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Whether Flatpak itself is installed.
pub fn available() -> bool {
    util::have("flatpak")
}

/// Whether the app is installed for everyone already.
pub fn installed(id: &str) -> bool {
    Command::new("flatpak")
        .args(["info", "--system", id])
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Whether a repository of that name is set up for everyone.
pub fn has_repository(name: &str) -> bool {
    util::output("flatpak", &["remotes", "--system", "--columns=name"])
        .is_some_and(|text| text.lines().any(|line| line.trim() == name))
}

/// The command that installs the file (or adds the repository).
pub fn install_command(kind: Kind, path: &Path, info: &Info) -> Vec<String> {
    let path = path.to_string_lossy().into_owned();
    let mut command: Vec<String> = ["flatpak"].map(String::from).to_vec();
    match kind {
        Kind::Ref => {
            command.extend(["install", "--system", "--assumeyes", "--from"].map(String::from))
        }
        Kind::Bundle => {
            command.extend(["install", "--system", "--assumeyes", "--bundle"].map(String::from))
        }
        Kind::Repo => {
            command
                .extend(["remote-add", "--system", "--if-not-exists", "--from"].map(String::from));
            command.push(info.id.clone());
        }
    }
    command.push(path);
    command
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
        assert!(!info.runtime);
        assert_eq!(
            install_command(Kind::Ref, &path, &info),
            [
                "flatpak",
                "install",
                "--system",
                "--assumeyes",
                "--from",
                path.to_str().unwrap()
            ]
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
            install_command(Kind::Repo, &repo, &info)[4..6],
            ["--from", "flat-hub"]
        );

        std::fs::write(&path, "[Flatpak Ref]\nName=org.example.Notes\n").unwrap();
        assert!(read(&path, Kind::Ref).is_err(), "no Url");
        std::fs::write(&path, "not a key file").unwrap();
        assert!(read(&path, Kind::Ref).is_err());
    }

    /// A bundle as `flatpak build-bundle` lays it out (only the parts
    /// Unpack reads are filled in).
    #[test]
    fn bundle() {
        let appdata = "<?xml version=\"1.0\"?><components><component type=\"desktop-application\">\
            <id>org.example.Notes</id><name xml:lang=\"de\">Notizen</name><name>Notes &amp; Lists</name>\
            <summary>Take notes</summary><description><p>One.</p><ul><li>Two</li></ul></description>\
            </component></components>";
        let compressor = gio::ZlibCompressor::new(gio::ZlibCompressorFormat::Gzip, -1);
        let stream = gio::ConverterInputStream::new(
            &gio::MemoryInputStream::from_bytes(&glib::Bytes::from(appdata.as_bytes())),
            &compressor,
        );
        let mut gzipped = Vec::new();
        stream
            .upcast::<gio::InputStream>()
            .into_read()
            .read_to_end(&mut gzipped)
            .unwrap();

        let details = glib::VariantDict::new(None);
        details.insert_value("ref", &"app/org.example.Notes/x86_64/stable".to_variant());
        details.insert_value("origin", &"https://dl.example.org/repo/".to_variant());
        details.insert_value("installed-size", &12_345_678u64.to_be().to_variant());
        details.insert_value(
            "metadata",
            &"[Application]\nname=org.example.Notes\nruntime=org.gnome.Platform/x86_64/50\n"
                .to_variant(),
        );
        details.insert_value("appdata", &glib::Variant::array_from_fixed_array(&gzipped));
        let empty_dict = glib::VariantDict::new(None).end();
        let bytes = |data: &[u8]| glib::Variant::array_from_fixed_array(data);
        let superblock = glib::Variant::tuple_from_iter([
            details.end(),
            0u64.to_variant(),
            bytes(&[]),
            bytes(&[]),
            glib::Variant::tuple_from_iter([
                empty_dict,
                bytes(&[]),
                glib::Variant::array_from_iter_with_type(
                    glib::VariantTy::new("(say)").unwrap(),
                    std::iter::empty::<glib::Variant>(),
                ),
                "".to_variant(),
                "".to_variant(),
                0u64.to_variant(),
                bytes(&[]),
                bytes(&[]),
            ]),
            glib::Variant::array_from_iter_with_type(
                glib::VariantTy::new("(uayttay)").unwrap(),
                std::iter::empty::<glib::Variant>(),
            ),
            glib::Variant::array_from_iter_with_type(
                glib::VariantTy::new("(yaytt)").unwrap(),
                std::iter::empty::<glib::Variant>(),
            ),
        ]);
        assert_eq!(superblock.type_().as_str(), BUNDLE_TYPE);
        let info = parse_bundle(superblock.data().to_vec()).unwrap();
        assert_eq!(info.id, "org.example.Notes");
        assert_eq!(info.title, "Notes & Lists");
        assert_eq!(info.summary, "Take notes");
        assert_eq!(info.description, "One.\n\n• Two");
        assert_eq!(info.branch, "stable");
        assert_eq!(info.installed_size, Some(12_345_678));
        assert_eq!(info.needs.as_deref(), Some("org.gnome.Platform/x86_64/50"));
        assert!(parse_bundle(b"garbage".to_vec()).is_err());
    }
}
