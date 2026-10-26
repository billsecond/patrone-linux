//! Archives through libarchive, the library GNOME's own Files uses: zip,
//! tar (plain, gzip, xz, bzip2, zstd, lzma, lzip), 7z, rar (versions 4 and
//! 5), cpio, cab, ISO images, and single compressed files (`notes.txt.gz`).
//!
//! Extracting is done here rather than by libarchive's disk writer, so that
//! paths can be rewritten into the destination without changing the current
//! folder, and so that nothing lands outside the destination: absolute paths
//! and `..` are refused, and no file is written through a folder that is
//! really a link (an archive could make the link first, then a file "inside"
//! it).

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{OpenOptionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[repr(C)]
struct RawArchive {
    _private: [u8; 0],
}

#[repr(C)]
struct RawEntry {
    _private: [u8; 0],
}

const ARCHIVE_EOF: c_int = 1;
const ARCHIVE_OK: c_int = 0;
const ARCHIVE_RETRY: c_int = -10;
const ARCHIVE_WARN: c_int = -20;

const AE_IFMT: u32 = 0o170000;
const AE_IFREG: u32 = 0o100000;
const AE_IFLNK: u32 = 0o120000;
const AE_IFDIR: u32 = 0o040000;

unsafe extern "C" {
    fn archive_read_new() -> *mut RawArchive;
    fn archive_read_support_filter_all(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_7zip(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_ar(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_cab(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_cpio(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_empty(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_iso9660(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_lha(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_rar(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_rar5(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_tar(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_xar(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_zip(a: *mut RawArchive) -> c_int;
    fn archive_read_support_format_raw(a: *mut RawArchive) -> c_int;
    fn archive_read_add_passphrase(a: *mut RawArchive, passphrase: *const c_char) -> c_int;
    fn archive_read_open_filename(
        a: *mut RawArchive,
        filename: *const c_char,
        block_size: usize,
    ) -> c_int;
    fn archive_read_next_header(a: *mut RawArchive, entry: *mut *mut RawEntry) -> c_int;
    fn archive_read_data_block(
        a: *mut RawArchive,
        buffer: *mut *const c_void,
        size: *mut usize,
        offset: *mut i64,
    ) -> c_int;
    fn archive_read_data_skip(a: *mut RawArchive) -> c_int;
    fn archive_read_free(a: *mut RawArchive) -> c_int;
    fn archive_error_string(a: *mut RawArchive) -> *const c_char;
    fn archive_format_name(a: *mut RawArchive) -> *const c_char;
    fn archive_filter_count(a: *mut RawArchive) -> c_int;
    fn archive_filter_name(a: *mut RawArchive, n: c_int) -> *const c_char;
    fn archive_entry_pathname(e: *mut RawEntry) -> *const c_char;
    fn archive_entry_pathname_utf8(e: *mut RawEntry) -> *const c_char;
    fn archive_entry_symlink(e: *mut RawEntry) -> *const c_char;
    fn archive_entry_hardlink(e: *mut RawEntry) -> *const c_char;
    fn archive_entry_filetype(e: *mut RawEntry) -> u32;
    fn archive_entry_perm(e: *mut RawEntry) -> u32;
    fn archive_entry_size(e: *mut RawEntry) -> i64;
    fn archive_entry_size_is_set(e: *mut RawEntry) -> c_int;
    fn archive_entry_mtime(e: *mut RawEntry) -> i64;
    fn archive_entry_mtime_is_set(e: *mut RawEntry) -> c_int;
    fn archive_entry_is_encrypted(e: *mut RawEntry) -> c_int;
}

/// Why reading or extracting stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The archive is protected by a password and none was given.
    NeedsPassword,
    /// The password given isn't the archive's.
    WrongPassword,
    Cancelled,
    Failed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NeedsPassword => write!(f, "This archive is protected by a password."),
            Error::WrongPassword => write!(f, "That password isn't right."),
            Error::Cancelled => write!(f, "Cancelled."),
            Error::Failed(message) => write!(f, "{message}"),
        }
    }
}

/// libarchive's message, turned into an Error.
fn classify(message: String) -> Error {
    let lower = message.to_lowercase();
    if lower.contains("incorrect passphrase") {
        Error::WrongPassword
    } else if lower.contains("passphrase required") || lower.contains("passphrase is required") {
        Error::NeedsPassword
    } else if lower.contains("encrypt")
        && (lower.contains("not supported") || lower.contains("unsupported"))
    {
        Error::Failed(format!(
            "This archive is encrypted in a way Unpack can't open ({message})."
        ))
    } else {
        Error::Failed(message)
    }
}

fn text(pointer: *const c_char) -> Option<String> {
    if pointer.is_null() {
        None
    } else {
        // SAFETY: libarchive returns NUL-terminated strings it owns, valid
        // until the next call on the same object; this copies at once.
        Some(
            unsafe { CStr::from_ptr(pointer) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    File,
    Folder,
    /// A symbolic link, and where it points.
    Link(String),
    /// A second name for a file earlier in the archive (tar's hard links).
    HardLink(String),
    /// Devices, pipes and the like, which Unpack doesn't extract.
    Other,
}

/// One item in an archive, as stored.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: String,
    pub kind: Kind,
    pub size: Option<u64>,
    pub modified: Option<i64>,
    pub permissions: u32,
    pub encrypted: bool,
}

/// An open archive being read from start to end.
struct Reader(*mut RawArchive);

impl Drop for Reader {
    fn drop(&mut self) {
        // SAFETY: the pointer came from archive_read_new and is freed once.
        unsafe { archive_read_free(self.0) };
    }
}

impl Reader {
    /// Opens an archive; with `raw`, a single compressed file instead.
    fn open(path: &Path, passphrase: Option<&str>, raw: bool) -> Result<Reader, Error> {
        // SAFETY: plain libarchive calls on an object this function owns.
        let archive = unsafe { archive_read_new() };
        if archive.is_null() {
            return Err(Error::Failed("libarchive couldn't start.".into()));
        }
        let reader = Reader(archive);
        unsafe {
            archive_read_support_filter_all(archive);
            if raw {
                archive_read_support_format_raw(archive);
            } else {
                // Every archive format but mtree (a BSD listing format), which
                // takes any line of text for an archive, so a compressed text
                // file came out as an empty one.
                archive_read_support_format_7zip(archive);
                archive_read_support_format_ar(archive);
                archive_read_support_format_cab(archive);
                archive_read_support_format_cpio(archive);
                archive_read_support_format_empty(archive);
                archive_read_support_format_iso9660(archive);
                archive_read_support_format_lha(archive);
                archive_read_support_format_rar(archive);
                archive_read_support_format_rar5(archive);
                archive_read_support_format_tar(archive);
                archive_read_support_format_xar(archive);
                archive_read_support_format_zip(archive);
            }
        }
        if let Some(passphrase) = passphrase {
            let passphrase = CString::new(passphrase)
                .map_err(|_| Error::Failed("The password can't contain a NUL.".into()))?;
            // libarchive copies the passphrase.
            unsafe { archive_read_add_passphrase(archive, passphrase.as_ptr()) };
        }
        let name = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| Error::Failed("The file's name can't be read.".into()))?;
        if unsafe { archive_read_open_filename(archive, name.as_ptr(), 64 * 1024) } != ARCHIVE_OK {
            return Err(reader.error());
        }
        Ok(reader)
    }

    fn error(&self) -> Error {
        let message = text(unsafe { archive_error_string(self.0) })
            .unwrap_or_else(|| "The archive couldn't be read.".into());
        classify(message)
    }

    /// The next entry's header, or None at the end.
    fn next(&mut self) -> Result<Option<Entry>, Error> {
        let mut entry: *mut RawEntry = std::ptr::null_mut();
        let mut status = ARCHIVE_RETRY;
        for _ in 0..3 {
            status = unsafe { archive_read_next_header(self.0, &mut entry) };
            if status != ARCHIVE_RETRY {
                break;
            }
        }
        match status {
            ARCHIVE_EOF => return Ok(None),
            ARCHIVE_OK | ARCHIVE_WARN => {}
            _ => return Err(self.error()),
        }
        // SAFETY: libarchive gave a valid entry, which stays valid until the
        // next header is read; everything is copied out here.
        unsafe {
            let path = text(archive_entry_pathname_utf8(entry))
                .or_else(|| text(archive_entry_pathname(entry)))
                .unwrap_or_default();
            let kind = if let Some(target) = text(archive_entry_hardlink(entry)) {
                Kind::HardLink(target)
            } else {
                match archive_entry_filetype(entry) & AE_IFMT {
                    AE_IFREG => Kind::File,
                    AE_IFDIR => Kind::Folder,
                    AE_IFLNK => Kind::Link(text(archive_entry_symlink(entry)).unwrap_or_default()),
                    // Raw (single compressed file) entries have no type.
                    0 => Kind::File,
                    _ => Kind::Other,
                }
            };
            let size = (archive_entry_size_is_set(entry) != 0)
                .then(|| archive_entry_size(entry).max(0) as u64);
            let modified =
                (archive_entry_mtime_is_set(entry) != 0).then(|| archive_entry_mtime(entry));
            Ok(Some(Entry {
                path,
                kind,
                size,
                modified,
                permissions: archive_entry_perm(entry),
                encrypted: archive_entry_is_encrypted(entry) != 0,
            }))
        }
    }

    /// The current entry's data, a block at a time, with its offset (blocks
    /// jump ahead over holes in sparse files). None at the end.
    fn block(&mut self) -> Result<Option<(&[u8], u64)>, Error> {
        let mut buffer: *const c_void = std::ptr::null();
        let mut size = 0usize;
        let mut offset = 0i64;
        let status =
            unsafe { archive_read_data_block(self.0, &mut buffer, &mut size, &mut offset) };
        match status {
            ARCHIVE_EOF => Ok(None),
            ARCHIVE_OK | ARCHIVE_WARN => {
                let data = if size == 0 || buffer.is_null() {
                    &[][..]
                } else {
                    // SAFETY: libarchive's buffer holds `size` bytes until
                    // the next read call, which needs `&mut self` again.
                    unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), size) }
                };
                Ok(Some((data, offset.max(0) as u64)))
            }
            _ => Err(self.error()),
        }
    }

    fn skip(&mut self) {
        unsafe { archive_read_data_skip(self.0) };
    }

    /// "ZIP 2.0 (deflation)", "GNU tar format", "RAR5"...
    fn format(&self) -> String {
        text(unsafe { archive_format_name(self.0) }).unwrap_or_default()
    }

    /// The compression around the archive ("gzip", "xz"...), if any.
    fn filters(&self) -> Vec<String> {
        let count = unsafe { archive_filter_count(self.0) };
        (0..count)
            .filter_map(|n| text(unsafe { archive_filter_name(self.0, n) }))
            .filter(|name| name != "none")
            .collect()
    }
}

/// What's in an archive.
#[derive(Debug, Clone)]
pub struct Listing {
    pub entries: Vec<Entry>,
    /// "ZIP", "tar (gzip)", "7-Zip", "RAR"...
    pub format: String,
    /// A single compressed file rather than an archive: it holds one file,
    /// named after the archive without its compression suffix.
    pub raw: bool,
}

/// A short name for the archive's format.
fn format_name(format: &str, filters: &[String]) -> String {
    let lower = format.to_lowercase();
    // GNU tar, POSIX ustar, POSIX pax interchange format
    let base = if lower.starts_with("zip") {
        "ZIP"
    } else if lower.contains("tar") || lower.contains("pax") {
        "tar"
    } else if lower.starts_with("7-zip") {
        "7-Zip"
    } else if lower.starts_with("rar") {
        "RAR"
    } else if lower.starts_with("raw") {
        "Compressed file"
    } else {
        format
    };
    match filters.first() {
        Some(filter) => format!("{base} ({filter})"),
        None => base.to_string(),
    }
}

/// Reads every entry's header. Archives are tried first, then a single
/// compressed file.
pub fn list(path: &Path, passphrase: Option<&str>) -> Result<Listing, Error> {
    match list_with(path, passphrase, false) {
        Err(Error::Failed(message)) if message.contains("Unrecognized archive format") => {
            // Not an archive libarchive knows: maybe one compressed file.
            list_with(path, passphrase, true)
                .map_err(|_| Error::Failed("This isn't an archive Unpack knows.".into()))
        }
        result => result,
    }
}

fn list_with(path: &Path, passphrase: Option<&str>, raw: bool) -> Result<Listing, Error> {
    let mut reader = Reader::open(path, passphrase, raw)?;
    let mut entries = Vec::new();
    while let Some(mut entry) = reader.next()? {
        if raw {
            entry.path = raw_name(path);
        }
        entries.push(entry);
        reader.skip();
    }
    let filters = reader.filters();
    // In raw mode anything opens; it only counts if it was compressed.
    if raw && filters.is_empty() {
        return Err(Error::Failed("This isn't an archive Unpack knows.".into()));
    }
    Ok(Listing {
        entries,
        format: format_name(&reader.format(), &filters),
        raw,
    })
}

/// Suffixes that name an archive or compression, longest first where one
/// ends another.
const SUFFIXES: &[&str] = &[
    ".tar.gz",
    ".tar.xz",
    ".tar.bz2",
    ".tar.zst",
    ".tar.lzma",
    ".tar.lz",
    ".tar.z",
    ".tgz",
    ".txz",
    ".tbz2",
    ".tbz",
    ".tzst",
    ".tlz",
    ".zip",
    ".7z",
    ".rar",
    ".tar",
    ".gz",
    ".xz",
    ".bz2",
    ".zst",
    ".lzma",
    ".lz",
    ".z",
    ".cpio",
    ".cab",
    ".iso",
    ".xar",
    ".jar",
];

/// The archive's name without its suffix: the name its folder gets.
pub fn stem(file_name: &str) -> String {
    let lower = file_name.to_lowercase();
    for suffix in SUFFIXES {
        if lower.ends_with(suffix) && lower.len() > suffix.len() {
            return file_name[..file_name.len() - suffix.len()].to_string();
        }
    }
    match file_name.rfind('.') {
        Some(dot) if dot > 0 => file_name[..dot].to_string(),
        _ => file_name.to_string(),
    }
}

/// The name of the one file in a single compressed file.
pub fn raw_name(archive: &Path) -> String {
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = stem(&name);
    if stem == name {
        format!("{name}.out")
    } else {
        stem
    }
}

/// An entry's path split into names, or None if it would leave the
/// destination (absolute, or going up with `..`).
pub fn safe_parts(path: &str) -> Option<Vec<&str>> {
    if path.starts_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then_some(parts)
}

/// One file or folder in the archive's folder tree.
#[derive(Debug, Clone)]
pub struct Node {
    pub name: String,
    /// The path inside the archive, with `/` between names.
    pub path: String,
    pub folder: bool,
    pub size: Option<u64>,
    pub modified: Option<i64>,
    pub encrypted: bool,
    pub link: bool,
    pub parent: usize,
    pub children: Vec<usize>,
}

/// The archive as folders and files. Folders that only appear in other
/// entries' paths are made up, as Windows shows them.
#[derive(Debug, Clone)]
pub struct Tree {
    /// `nodes[0]` is the archive itself.
    pub nodes: Vec<Node>,
    /// Entries left out because their paths would leave the destination.
    pub unsafe_entries: usize,
}

impl Tree {
    pub fn build(entries: &[Entry]) -> Tree {
        let mut nodes = vec![Node {
            name: String::new(),
            path: String::new(),
            folder: true,
            size: None,
            modified: None,
            encrypted: false,
            link: false,
            parent: 0,
            children: Vec::new(),
        }];
        let mut by_path: HashMap<String, usize> = HashMap::new();
        let mut unsafe_entries = 0;
        for entry in entries {
            if matches!(entry.kind, Kind::Other) {
                continue;
            }
            let Some(parts) = safe_parts(&entry.path) else {
                unsafe_entries += 1;
                continue;
            };
            let mut parent = 0;
            for depth in 0..parts.len() {
                let path = parts[..=depth].join("/");
                let last = depth == parts.len() - 1;
                let folder = !last || entry.kind == Kind::Folder;
                let index = match by_path.get(&path) {
                    Some(&index) => index,
                    None => {
                        nodes.push(Node {
                            name: parts[depth].to_string(),
                            path: path.clone(),
                            folder,
                            size: None,
                            modified: None,
                            encrypted: false,
                            link: false,
                            parent,
                            children: Vec::new(),
                        });
                        let index = nodes.len() - 1;
                        nodes[parent].children.push(index);
                        by_path.insert(path, index);
                        index
                    }
                };
                if last {
                    let node = &mut nodes[index];
                    node.modified = entry.modified;
                    node.encrypted = entry.encrypted;
                    if !node.folder {
                        node.size = entry.size;
                        node.link = matches!(entry.kind, Kind::Link(_));
                    }
                }
                parent = index;
            }
        }
        // Folders first, then by name, as Explorer sorts.
        let keys: Vec<(bool, String)> = nodes
            .iter()
            .map(|node| (!node.folder, node.name.to_lowercase()))
            .collect();
        for node in &mut nodes {
            node.children.sort_by(|&a, &b| keys[a].cmp(&keys[b]));
        }
        Tree {
            nodes,
            unsafe_entries,
        }
    }

    pub fn roots(&self) -> &[usize] {
        &self.nodes[0].children
    }

    /// Files and folders under (not counting) a node.
    pub fn counts(&self, index: usize) -> (usize, usize) {
        let mut files = 0;
        let mut folders = 0;
        for &child in &self.nodes[index].children {
            if self.nodes[child].folder {
                folders += 1;
                let (f, d) = self.counts(child);
                files += f;
                folders += d;
            } else {
                files += 1;
            }
        }
        (files, folders)
    }

    /// The size of every file, when extracted.
    pub fn total_size(&self) -> u64 {
        self.nodes
            .iter()
            .filter(|node| !node.folder)
            .filter_map(|node| node.size)
            .sum()
    }

    pub fn encrypted(&self) -> bool {
        self.nodes.iter().any(|node| node.encrypted)
    }

    /// The one folder the whole archive is in, if it's like that.
    pub fn single_folder(&self) -> Option<&str> {
        match self.roots() {
            [only] if self.nodes[*only].folder => Some(&self.nodes[*only].name),
            _ => None,
        }
    }
}

/// Replace or skip files that are already there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Existing {
    Replace,
    Skip,
}

/// Where and how to extract.
#[derive(Debug, Clone)]
pub struct Plan {
    pub archive: PathBuf,
    /// The folder everything goes into (made if it isn't there).
    pub destination: PathBuf,
    /// A first folder to leave out of every path: its contents go straight
    /// into the destination.
    pub strip: Option<String>,
    pub existing: Existing,
    /// Only this path (a file, or a folder and what's in it).
    pub only: Option<String>,
    pub passphrase: Option<String>,
    /// For a single compressed file: the name to give the file.
    pub raw_name: Option<String>,
}

impl Plan {
    pub fn new(archive: &Path, destination: &Path) -> Plan {
        Plan {
            archive: archive.to_path_buf(),
            destination: destination.to_path_buf(),
            strip: None,
            existing: Existing::Replace,
            only: None,
            passphrase: None,
            raw_name: None,
        }
    }

    /// An entry's path relative to the destination, or None to leave it
    /// out (outside `only`, or the stripped folder itself).
    fn relative<'a>(&self, parts: &[&'a str]) -> Option<Vec<&'a str>> {
        let mut parts = parts.to_vec();
        if let Some(only) = &self.only {
            let joined = parts.join("/");
            if joined != *only && !joined.starts_with(&format!("{only}/")) {
                return None;
            }
        }
        if let Some(strip) = &self.strip
            && parts.first() == Some(&strip.as_str())
        {
            parts.remove(0);
        }
        (!parts.is_empty()).then_some(parts)
    }
}

/// "Extract here": the folder and name a whole archive goes to, next to
/// the archive. An archive holding one folder or one file extracts as that
/// folder or file; anything else goes into a new folder named after the
/// archive. Names already taken get " (2)", " (3)", as in Windows.
pub fn plan_here(archive: &Path, tree: &Tree, raw: bool) -> Plan {
    let parent = archive.parent().unwrap_or(Path::new(".")).to_path_buf();
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut plan = Plan::new(archive, &parent);
    if raw {
        plan.raw_name = Some(unique_name(&parent, &raw_name(archive), false));
        return plan;
    }
    match tree.roots() {
        [only] if tree.nodes[*only].folder => {
            let folder = &tree.nodes[*only].name;
            plan.destination = parent.join(unique_name(&parent, folder, true));
            plan.strip = Some(folder.clone());
        }
        [only]
            if parent
                .join(&tree.nodes[*only].name)
                .symlink_metadata()
                .is_err() => {}
        _ => plan.destination = parent.join(unique_name(&parent, &stem(&name), true)),
    }
    plan
}

/// "Extract all": the folder Windows suggests, next to the archive and
/// named after it.
pub fn suggested_folder(archive: &Path) -> PathBuf {
    let parent = archive.parent().unwrap_or(Path::new("."));
    let name = archive
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    parent.join(stem(&name))
}

/// The folder to leave out when extracting into `destination`: an archive
/// holding just `name/`, extracted into a folder also called `name`, puts
/// its contents straight in (not `name/name/`).
pub fn strip_for(tree: &Tree, destination: &Path) -> Option<String> {
    let folder = tree.single_folder()?;
    let name = destination.file_name()?.to_string_lossy();
    (name == folder).then(|| folder.to_string())
}

/// How many files would replace ones already in the destination.
pub fn conflicts(tree: &Tree, plan: &Plan) -> usize {
    if let Some(name) = &plan.raw_name {
        return usize::from(plan.destination.join(name).exists());
    }
    tree.nodes
        .iter()
        .skip(1)
        .filter(|node| !node.folder)
        .filter(|node| {
            let parts: Vec<&str> = node.path.split('/').collect();
            plan.relative(&parts).is_some_and(|relative| {
                let target = plan.destination.join(relative.join("/"));
                target.symlink_metadata().is_ok()
            })
        })
        .count()
}

/// A name not yet taken in a folder: "name", else "name (2)", "name (3)"...
/// (before the extension, for files).
pub fn unique_name(folder: &Path, name: &str, is_folder: bool) -> String {
    if folder.join(name).symlink_metadata().is_err() {
        return name.to_string();
    }
    let (base, extension) = match name.rfind('.') {
        Some(dot) if !is_folder && dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (2..)
        .map(|n| format!("{base} ({n}){extension}"))
        .find(|candidate| folder.join(candidate).symlink_metadata().is_err())
        .expect("some number is free")
}

/// Extraction's progress, shared with the window while it runs.
#[derive(Default)]
pub struct Progress {
    pub bytes: AtomicU64,
    pub cancel: AtomicBool,
    pub current: Mutex<String>,
}

/// What an extraction did.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Files, folders and links made.
    pub extracted: usize,
    /// Left as they were, because they were already there.
    pub skipped: usize,
    /// Items that couldn't be extracted, and why.
    pub problems: Vec<String>,
}

/// Extracts what the plan says.
pub fn extract(plan: &Plan, progress: &Progress) -> Result<Outcome, Error> {
    fs::create_dir_all(&plan.destination).map_err(|error| {
        Error::Failed(format!(
            "Couldn't make the folder {}: {error}",
            plan.destination.display()
        ))
    })?;
    let raw = plan.raw_name.is_some();
    let mut reader = Reader::open(&plan.archive, plan.passphrase.as_deref(), raw)?;
    let mut outcome = Outcome::default();
    while let Some(entry) = reader.next()? {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let relative: Vec<String> = if let Some(name) = &plan.raw_name {
            vec![name.clone()]
        } else {
            let Some(parts) = safe_parts(&entry.path) else {
                outcome
                    .problems
                    .push(format!("{}: leaves the destination folder", entry.path));
                continue;
            };
            match plan.relative(&parts) {
                Some(relative) => relative.iter().map(|part| part.to_string()).collect(),
                None => continue,
            }
        };
        *progress.current.lock().unwrap() = relative.join("/");
        let target = plan.destination.join(relative.join("/"));
        let result = folders_on_the_way(&plan.destination, &relative[..relative.len() - 1])
            .and_then(|()| match &entry.kind {
                Kind::Folder => {
                    folders_on_the_way(&plan.destination, &relative).map(|()| Made::Yes)
                }
                Kind::File => write_file(&mut reader, &target, &entry, plan.existing, progress),
                Kind::Link(points_to) => make_link(&target, points_to, plan.existing),
                Kind::HardLink(original) => {
                    let original = safe_parts(original)
                        .and_then(|parts| plan.relative(&parts))
                        .map(|parts| plan.destination.join(parts.join("/")));
                    match original {
                        Some(original) => make_hard_link(&original, &target, plan.existing),
                        None => Err(Step::Problem("links to a file outside the archive".into())),
                    }
                }
                Kind::Other => Err(Step::Problem("isn't a file, folder or link".into())),
            });
        match result {
            Ok(Made::Yes) => outcome.extracted += 1,
            Ok(Made::Skipped) => outcome.skipped += 1,
            Err(Step::Problem(why)) => outcome
                .problems
                .push(format!("{}: {why}", relative.join("/"))),
            Err(Step::Stop(error)) => return Err(error),
        }
    }
    if progress.cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    Ok(outcome)
}

enum Made {
    Yes,
    Skipped,
}

/// An item that failed (extraction goes on), or a reason to stop.
enum Step {
    Problem(String),
    Stop(Error),
}

/// Makes each folder of `parts` under `base`, refusing anything in the way
/// that isn't a real folder (a file, or a link the archive may have made).
fn folders_on_the_way(base: &Path, parts: &[String]) -> Result<(), Step> {
    let mut path = base.to_path_buf();
    for part in parts {
        path.push(part);
        match path.symlink_metadata() {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(Step::Problem(format!(
                    "{} is in the way, and isn't a folder",
                    path.display()
                )));
            }
            Err(_) => fs::create_dir(&path).map_err(|error| {
                Step::Problem(format!(
                    "couldn't make the folder {}: {error}",
                    path.display()
                ))
            })?,
        }
    }
    Ok(())
}

/// Clears the way for a new item, or says to skip it.
fn clear(target: &Path, existing: Existing) -> Result<bool, Step> {
    match target.symlink_metadata() {
        Err(_) => Ok(true),
        Ok(_) if existing == Existing::Skip => Ok(false),
        Ok(metadata) if metadata.is_dir() => Err(Step::Problem(
            "a folder of that name is already there".into(),
        )),
        Ok(_) => fs::remove_file(target)
            .map(|()| true)
            .map_err(|error| Step::Problem(format!("couldn't replace it: {error}"))),
    }
}

fn write_file(
    reader: &mut Reader,
    target: &Path,
    entry: &Entry,
    existing: Existing,
    progress: &Progress,
) -> Result<Made, Step> {
    if !clear(target, existing)? {
        reader.skip();
        return Ok(Made::Skipped);
    }
    // The archive's permissions, less the account's umask (open applies it);
    // never set-user-ID and the like.
    let mode = match entry.permissions & 0o777 {
        0 => 0o644,
        mode => mode,
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(target)
        .map_err(|error| Step::Problem(format!("couldn't create it: {error}")))?;
    let failed = |error: std::io::Error| {
        let _ = fs::remove_file(target);
        Step::Problem(format!("couldn't write it: {error}"))
    };
    let mut position = 0u64;
    loop {
        if progress.cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_file(target);
            return Err(Step::Stop(Error::Cancelled));
        }
        let block = match reader.block() {
            Ok(Some(block)) => block,
            Ok(None) => break,
            Err(error) => {
                let _ = fs::remove_file(target);
                return Err(match error {
                    Error::NeedsPassword | Error::WrongPassword => Step::Stop(error),
                    Error::Failed(message) if message.contains("ncrypt") => {
                        Step::Stop(Error::Failed(message))
                    }
                    error => Step::Problem(error.to_string()),
                });
            }
        };
        let (data, offset) = block;
        if offset != position {
            file.seek(SeekFrom::Start(offset)).map_err(failed)?;
        }
        file.write_all(data).map_err(failed)?;
        position = offset + data.len() as u64;
        progress
            .bytes
            .fetch_add(data.len() as u64, Ordering::Relaxed);
    }
    // A sparse file can end in a hole.
    if let Some(size) = entry.size
        && position < size
    {
        file.set_len(size).map_err(failed)?;
    }
    if let Some(seconds) = entry.modified {
        let time = if seconds >= 0 {
            UNIX_EPOCH + Duration::from_secs(seconds as u64)
        } else {
            UNIX_EPOCH
        };
        let _ = file.set_modified(time.min(SystemTime::now() + Duration::from_secs(86_400 * 365)));
    }
    Ok(Made::Yes)
}

fn make_link(target: &Path, points_to: &str, existing: Existing) -> Result<Made, Step> {
    if !clear(target, existing)? {
        return Ok(Made::Skipped);
    }
    symlink(points_to, target)
        .map(|()| Made::Yes)
        .map_err(|error| Step::Problem(format!("couldn't make the link: {error}")))
}

fn make_hard_link(original: &Path, target: &Path, existing: Existing) -> Result<Made, Step> {
    if !clear(target, existing)? {
        return Ok(Made::Skipped);
    }
    // Copied rather than linked when the original is itself a link.
    let result = match original.symlink_metadata() {
        Ok(metadata) if metadata.is_file() => fs::hard_link(original, target),
        Ok(_) => return Err(Step::Problem("links to something that isn't a file".into())),
        Err(error) => Err(error),
    };
    result.map(|()| Made::Yes).map_err(|error| {
        Step::Problem(format!(
            "couldn't link it to {}: {error}",
            original.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // libarchive's writer, to make test archives without other tools.
    unsafe extern "C" {
        fn archive_write_new() -> *mut RawArchive;
        fn archive_write_set_format_zip(a: *mut RawArchive) -> c_int;
        fn archive_write_set_format_7zip(a: *mut RawArchive) -> c_int;
        fn archive_write_set_format_pax_restricted(a: *mut RawArchive) -> c_int;
        fn archive_write_set_format_raw(a: *mut RawArchive) -> c_int;
        fn archive_write_add_filter_by_name(a: *mut RawArchive, name: *const c_char) -> c_int;
        fn archive_write_set_options(a: *mut RawArchive, options: *const c_char) -> c_int;
        fn archive_write_set_passphrase(a: *mut RawArchive, passphrase: *const c_char) -> c_int;
        fn archive_write_open_filename(a: *mut RawArchive, filename: *const c_char) -> c_int;
        fn archive_write_header(a: *mut RawArchive, e: *mut RawEntry) -> c_int;
        fn archive_write_data(a: *mut RawArchive, data: *const c_void, size: usize) -> isize;
        fn archive_write_close(a: *mut RawArchive) -> c_int;
        fn archive_write_free(a: *mut RawArchive) -> c_int;
        fn archive_entry_new() -> *mut RawEntry;
        fn archive_entry_free(e: *mut RawEntry);
        fn archive_entry_copy_pathname(e: *mut RawEntry, name: *const c_char);
        fn archive_entry_copy_symlink(e: *mut RawEntry, name: *const c_char);
        fn archive_entry_copy_hardlink(e: *mut RawEntry, name: *const c_char);
        fn archive_entry_set_filetype(e: *mut RawEntry, kind: u32);
        fn archive_entry_set_perm(e: *mut RawEntry, perm: u32);
        fn archive_entry_set_size(e: *mut RawEntry, size: i64);
        fn archive_entry_set_mtime(e: *mut RawEntry, seconds: i64, nanoseconds: std::ffi::c_long);
    }

    enum Item<'a> {
        Folder(&'a str),
        File(&'a str, &'a [u8], u32),
        Link(&'a str, &'a str),
        HardLink(&'a str, &'a str),
    }

    fn c(text: &str) -> CString {
        CString::new(text).unwrap()
    }

    /// Writes an archive: format "zip", "7zip", "tar" or "raw", an optional
    /// compression ("gzip", "xz"...), an optional zip password.
    fn write(
        path: &Path,
        format: &str,
        filter: Option<&str>,
        password: Option<&str>,
        items: &[Item],
    ) {
        unsafe {
            let a = archive_write_new();
            match format {
                "zip" => archive_write_set_format_zip(a),
                "7zip" => archive_write_set_format_7zip(a),
                "tar" => archive_write_set_format_pax_restricted(a),
                "raw" => archive_write_set_format_raw(a),
                _ => unreachable!(),
            };
            if let Some(filter) = filter {
                assert_eq!(
                    archive_write_add_filter_by_name(a, c(filter).as_ptr()),
                    ARCHIVE_OK,
                    "{filter}"
                );
            }
            if let Some(password) = password {
                assert_eq!(
                    archive_write_set_options(a, c("zip:encryption=zipcrypt").as_ptr()),
                    ARCHIVE_OK
                );
                archive_write_set_passphrase(a, c(password).as_ptr());
            }
            let name = c(path.to_str().unwrap());
            assert_eq!(archive_write_open_filename(a, name.as_ptr()), ARCHIVE_OK);
            for item in items {
                let e = archive_entry_new();
                archive_entry_set_mtime(e, 1_790_000_000, 0);
                let data: &[u8] = match item {
                    Item::Folder(path) => {
                        archive_entry_copy_pathname(e, c(path).as_ptr());
                        archive_entry_set_filetype(e, AE_IFDIR);
                        archive_entry_set_perm(e, 0o755);
                        &[]
                    }
                    Item::File(path, data, perm) => {
                        archive_entry_copy_pathname(e, c(path).as_ptr());
                        archive_entry_set_filetype(e, AE_IFREG);
                        archive_entry_set_perm(e, *perm);
                        archive_entry_set_size(e, data.len() as i64);
                        data
                    }
                    Item::Link(path, target) => {
                        archive_entry_copy_pathname(e, c(path).as_ptr());
                        archive_entry_set_filetype(e, AE_IFLNK);
                        archive_entry_set_perm(e, 0o777);
                        archive_entry_copy_symlink(e, c(target).as_ptr());
                        &[]
                    }
                    Item::HardLink(path, original) => {
                        archive_entry_copy_pathname(e, c(path).as_ptr());
                        archive_entry_set_filetype(e, AE_IFREG);
                        archive_entry_set_perm(e, 0o644);
                        archive_entry_copy_hardlink(e, c(original).as_ptr());
                        &[]
                    }
                };
                assert!(archive_write_header(a, e) >= ARCHIVE_WARN);
                if !data.is_empty() {
                    assert_eq!(
                        archive_write_data(a, data.as_ptr().cast(), data.len()),
                        data.len() as isize
                    );
                }
                archive_entry_free(e);
            }
            assert_eq!(archive_write_close(a), ARCHIVE_OK);
            archive_write_free(a);
        }
    }

    /// A fresh folder in the temporary folder, for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("patrone-unpack-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SAMPLE: &[Item] = &[
        Item::Folder("project/"),
        Item::File("project/hello.txt", b"Hello from Patrone\n", 0o644),
        Item::Folder("project/tools/"),
        Item::File("project/tools/run.sh", b"#!/bin/sh\necho hi\n", 0o755),
        Item::Link("project/greeting", "hello.txt"),
        Item::File("project/empty.txt", b"", 0o644),
    ];

    fn check_sample(root: &Path) {
        assert_eq!(
            fs::read(root.join("hello.txt")).unwrap(),
            b"Hello from Patrone\n"
        );
        assert_eq!(fs::read(root.join("empty.txt")).unwrap(), b"");
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(root.join("tools/run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o100, 0o100, "run.sh keeps its run permission");
        assert_eq!(
            fs::read_link(root.join("greeting")).unwrap(),
            Path::new("hello.txt")
        );
    }

    #[test]
    fn every_format_lists_and_extracts() {
        let dir = scratch("formats");
        let cases = [
            ("sample.zip", "zip", None, "ZIP"),
            ("sample.7z", "7zip", None, "7-Zip"),
            ("sample.tar", "tar", None, "tar"),
            ("sample.tar.gz", "tar", Some("gzip"), "tar (gzip)"),
            ("sample.tar.xz", "tar", Some("xz"), "tar (xz)"),
            ("sample.tar.bz2", "tar", Some("bzip2"), "tar (bzip2)"),
            ("sample.tar.zst", "tar", Some("zstd"), "tar (zstd)"),
        ];
        for (name, format, filter, expected) in cases {
            let archive = dir.join(name);
            write(&archive, format, filter, None, SAMPLE);
            let listing = list(&archive, None).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(listing.format, expected, "{name}");
            assert!(!listing.raw);
            let tree = Tree::build(&listing.entries);
            assert_eq!(tree.single_folder(), Some("project"), "{name}");
            assert_eq!(
                tree.counts(0),
                (4, 2),
                "{name}: files (with the link) and folders"
            );
            assert_eq!(tree.total_size(), 19 + 18 + tree_link_size(&tree), "{name}");

            let plan = plan_here(&archive, &tree, false);
            assert_eq!(plan.destination, dir.join("project"));
            let outcome = extract(&plan, &Progress::default()).unwrap();
            assert!(
                outcome.problems.is_empty(),
                "{name}: {:?}",
                outcome.problems
            );
            check_sample(&dir.join("project"));
            fs::remove_dir_all(dir.join("project")).unwrap();
        }
    }

    /// Some formats store a link's target as its data size.
    fn tree_link_size(tree: &Tree) -> u64 {
        tree.nodes
            .iter()
            .filter(|n| n.link)
            .filter_map(|n| n.size)
            .sum()
    }

    #[test]
    fn extract_here_names() {
        let dir = scratch("here");
        // One folder: extracts as that folder, " (2)" when it's taken.
        let one = dir.join("one.zip");
        write(&one, "zip", None, None, SAMPLE);
        let tree = Tree::build(&list(&one, None).unwrap().entries);
        fs::create_dir(dir.join("project")).unwrap();
        let plan = plan_here(&one, &tree, false);
        assert_eq!(plan.destination, dir.join("project (2)"));
        assert_eq!(plan.strip.as_deref(), Some("project"));
        extract(&plan, &Progress::default()).unwrap();
        check_sample(&dir.join("project (2)"));

        // Loose files: into a folder named after the archive.
        let loose = dir.join("Loose Files.tar.gz");
        write(
            &loose,
            "tar",
            Some("gzip"),
            None,
            &[
                Item::File("a.txt", b"a", 0o644),
                Item::File("b.txt", b"b", 0o644),
            ],
        );
        let tree = Tree::build(&list(&loose, None).unwrap().entries);
        let plan = plan_here(&loose, &tree, false);
        assert_eq!(plan.destination, dir.join("Loose Files"));
        assert_eq!(plan.strip, None);
        extract(&plan, &Progress::default()).unwrap();
        assert_eq!(fs::read(dir.join("Loose Files/b.txt")).unwrap(), b"b");

        // One file: next to the archive, unless that name is taken.
        let single = dir.join("report.zip");
        write(
            &single,
            "zip",
            None,
            None,
            &[Item::File("report.txt", b"r", 0o644)],
        );
        let tree = Tree::build(&list(&single, None).unwrap().entries);
        assert_eq!(plan_here(&single, &tree, false).destination, dir);
        fs::write(dir.join("report.txt"), b"mine").unwrap();
        assert_eq!(
            plan_here(&single, &tree, false).destination,
            dir.join("report")
        );
    }

    #[test]
    fn extract_all_into_a_folder_of_the_same_name() {
        let dir = scratch("all");
        let archive = dir.join("project.tar.xz");
        write(&archive, "tar", Some("xz"), None, SAMPLE);
        let tree = Tree::build(&list(&archive, None).unwrap().entries);
        let folder = suggested_folder(&archive);
        assert_eq!(folder, dir.join("project"));
        let mut plan = Plan::new(&archive, &folder);
        plan.strip = strip_for(&tree, &folder);
        assert_eq!(plan.strip.as_deref(), Some("project"));
        assert_eq!(conflicts(&tree, &plan), 0);
        extract(&plan, &Progress::default()).unwrap();
        check_sample(&folder);

        // Again: everything is already there. Skip leaves the files alone.
        fs::write(folder.join("hello.txt"), b"changed").unwrap();
        assert_eq!(conflicts(&tree, &plan), 4);
        plan.existing = Existing::Skip;
        let outcome = extract(&plan, &Progress::default()).unwrap();
        assert_eq!(outcome.skipped, 4);
        assert_eq!(fs::read(folder.join("hello.txt")).unwrap(), b"changed");
        plan.existing = Existing::Replace;
        extract(&plan, &Progress::default()).unwrap();
        check_sample(&folder);

        // Into another folder, the archive's folder is kept.
        let other = dir.join("elsewhere");
        assert_eq!(strip_for(&tree, &other), None);
        extract(&Plan::new(&archive, &other), &Progress::default()).unwrap();
        check_sample(&other.join("project"));
    }

    #[test]
    fn nothing_lands_outside_the_destination() {
        let dir = scratch("unsafe");
        let archive = dir.join("evil.tar");
        let outside = dir.join("outside");
        fs::create_dir(&outside).unwrap();
        write(
            &archive,
            "tar",
            None,
            None,
            &[
                Item::File("../escaped.txt", b"x", 0o644),
                Item::File("/tmp/patrone-unpack-absolute.txt", b"x", 0o644),
                Item::File("ok/../../escaped2.txt", b"x", 0o644),
                // A link to a folder outside, then a file "inside" it
                Item::Link("way-out", outside.to_str().unwrap()),
                Item::File("way-out/planted.txt", b"x", 0o644),
                Item::HardLink("hard", "/etc/passwd"),
                Item::File("fine.txt", b"fine", 0o644),
            ],
        );
        let listing = list(&archive, None).unwrap();
        let tree = Tree::build(&listing.entries);
        assert_eq!(tree.unsafe_entries, 3);
        let destination = dir.join("into");
        let outcome = extract(&Plan::new(&archive, &destination), &Progress::default()).unwrap();
        assert_eq!(outcome.problems.len(), 5, "{:?}", outcome.problems);
        assert!(!dir.join("escaped.txt").exists());
        assert!(!dir.join("escaped2.txt").exists());
        assert!(!Path::new("/tmp/patrone-unpack-absolute.txt").exists());
        assert!(!outside.join("planted.txt").exists());
        assert!(!destination.join("hard").exists());
        assert_eq!(fs::read(destination.join("fine.txt")).unwrap(), b"fine");
    }

    #[test]
    fn hard_links_inside_the_archive() {
        let dir = scratch("hard");
        let archive = dir.join("links.tar");
        write(
            &archive,
            "tar",
            None,
            None,
            &[
                Item::File("data/original.txt", b"same", 0o644),
                Item::HardLink("data/copy.txt", "data/original.txt"),
            ],
        );
        let destination = dir.join("out");
        let mut plan = Plan::new(&archive, &destination);
        plan.strip = Some("data".into());
        let outcome = extract(&plan, &Progress::default()).unwrap();
        assert!(outcome.problems.is_empty(), "{:?}", outcome.problems);
        assert_eq!(fs::read(destination.join("copy.txt")).unwrap(), b"same");
    }

    #[test]
    fn single_compressed_file() {
        let dir = scratch("raw");
        let archive = dir.join("notes.txt.gz");
        // One word on a line: libarchive's mtree reader took that for an
        // archive listing.
        write(
            &archive,
            "raw",
            Some("gzip"),
            None,
            &[Item::File("data", b"hello\n", 0o644)],
        );
        let listing = list(&archive, None).unwrap();
        assert!(listing.raw);
        assert_eq!(listing.format, "Compressed file (gzip)");
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].path, "notes.txt");
        let tree = Tree::build(&listing.entries);
        let plan = plan_here(&archive, &tree, true);
        assert_eq!(plan.raw_name.as_deref(), Some("notes.txt"));
        extract(&plan, &Progress::default()).unwrap();
        assert_eq!(fs::read(dir.join("notes.txt")).unwrap(), b"hello\n");
        // Taken now: the next one is "notes (2).txt".
        assert_eq!(
            plan_here(&archive, &tree, true).raw_name.as_deref(),
            Some("notes (2).txt")
        );

        // Plain files aren't archives.
        let plain = dir.join("plain.txt");
        fs::write(&plain, b"just text").unwrap();
        assert!(matches!(list(&plain, None), Err(Error::Failed(_))));
    }

    #[test]
    fn password_protected_zip() {
        let dir = scratch("password");
        let archive = dir.join("secret.zip");
        write(
            &archive,
            "zip",
            None,
            Some("opensesame"),
            &[Item::File("secret.txt", b"the plans", 0o644)],
        );
        let listing = list(&archive, None).unwrap();
        assert!(listing.entries.iter().all(|entry| entry.encrypted));
        let tree = Tree::build(&listing.entries);
        assert!(tree.encrypted());
        let destination = dir.join("out");
        let mut plan = Plan::new(&archive, &destination);
        assert_eq!(
            extract(&plan, &Progress::default()).unwrap_err(),
            Error::NeedsPassword
        );
        assert!(
            !destination.join("secret.txt").exists(),
            "no half-written file"
        );
        // Zip's old encryption checks a password against one byte, so one
        // wrong password in 256 gets past that check and fails on the data.
        plan.passphrase = Some("wrong".into());
        match extract(&plan, &Progress::default()) {
            Err(Error::WrongPassword) => {}
            Ok(outcome) => assert_eq!(outcome.problems.len(), 1),
            Err(error) => panic!("{error}"),
        }
        assert!(
            !destination.join("secret.txt").exists(),
            "no half-written file"
        );
        plan.passphrase = Some("opensesame".into());
        extract(&plan, &Progress::default()).unwrap();
        assert_eq!(
            fs::read(destination.join("secret.txt")).unwrap(),
            b"the plans"
        );
    }

    #[test]
    fn opening_one_file() {
        let dir = scratch("only");
        let archive = dir.join("sample.7z");
        write(&archive, "7zip", None, None, SAMPLE);
        let destination = dir.join("open");
        let mut plan = Plan::new(&archive, &destination);
        plan.only = Some("project/tools/run.sh".into());
        plan.strip = Some("project".into());
        let outcome = extract(&plan, &Progress::default()).unwrap();
        assert_eq!(outcome.extracted, 1);
        assert!(destination.join("tools/run.sh").exists());
        assert!(!destination.join("hello.txt").exists());
    }

    #[test]
    fn cancelling_stops() {
        let dir = scratch("cancel");
        let archive = dir.join("sample.zip");
        write(&archive, "zip", None, None, SAMPLE);
        let progress = Progress::default();
        progress.cancel.store(true, Ordering::Relaxed);
        let result = extract(&Plan::new(&archive, &dir.join("out")), &progress);
        assert_eq!(result.unwrap_err(), Error::Cancelled);
    }

    #[test]
    fn names() {
        assert_eq!(stem("linux-7.0.tar.xz"), "linux-7.0");
        assert_eq!(stem("Photos.ZIP"), "Photos");
        assert_eq!(stem("backup.tgz"), "backup");
        assert_eq!(stem("game.7z"), "game");
        assert_eq!(stem("noextension"), "noextension");
        assert_eq!(stem(".zip"), ".zip");
        assert_eq!(stem("odd.thing"), "odd");
        assert_eq!(raw_name(Path::new("/x/notes.txt.gz")), "notes.txt");
        assert_eq!(safe_parts("./a//b/"), Some(vec!["a", "b"]));
        assert_eq!(safe_parts("a/../b"), None);
        assert_eq!(safe_parts("/etc"), None);
        assert_eq!(safe_parts("./"), None);
        let dir = scratch("unique");
        fs::write(dir.join("photo.jpg"), b"").unwrap();
        fs::write(dir.join("photo (2).jpg"), b"").unwrap();
        assert_eq!(unique_name(&dir, "photo.jpg", false), "photo (3).jpg");
        fs::create_dir(dir.join("v1.2")).unwrap();
        assert_eq!(unique_name(&dir, "v1.2", true), "v1.2 (2)");
        assert_eq!(unique_name(&dir, "new", true), "new");
    }
}
