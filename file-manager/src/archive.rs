//! "Compress to ZIP file", "Compress to TAR file" and "Extract All", in pure
//! Rust. These run on a worker thread; progress is shared through atomics.

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use zip::write::SimpleFileOptions;

/// Shared between the worker and the progress window.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancelled: AtomicBool,
}

impl Progress {
    pub fn new() -> Arc<Progress> {
        Arc::new(Progress::default())
    }

    fn check(&self) -> io::Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Zip,
    TarGz,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Zip => ".zip",
            Format::TarGz => ".tar.gz",
        }
    }
}

/// Every file under `sources`, with the name it gets in the archive.
fn walk(sources: &[PathBuf]) -> io::Result<Vec<(PathBuf, String, bool)>> {
    let mut entries = Vec::new();
    for source in sources {
        let base = source.parent().unwrap_or(Path::new("/"));
        let mut stack = vec![source.clone()];
        while let Some(path) = stack.pop() {
            let metadata = fs::symlink_metadata(&path)?;
            let name = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if metadata.is_dir() {
                entries.push((path.clone(), format!("{name}/"), true));
                let mut children: Vec<PathBuf> = fs::read_dir(&path)?
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .collect();
                children.sort();
                stack.extend(children.into_iter().rev());
            } else if metadata.is_file() {
                entries.push((path, name, false));
            }
        }
    }
    Ok(entries)
}

fn total_size(entries: &[(PathBuf, String, bool)]) -> u64 {
    entries
        .iter()
        .filter(|(_, _, dir)| !dir)
        .filter_map(|(path, _, _)| fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .sum()
}

/// Copies with progress and cancellation.
fn copy_counted(
    reader: &mut impl Read,
    writer: &mut impl Write,
    progress: &Progress,
) -> io::Result<()> {
    let mut buffer = vec![0; 256 * 1024];
    loop {
        progress.check()?;
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        writer.write_all(&buffer[..read])?;
        progress.done.fetch_add(read as u64, Ordering::Relaxed);
    }
}

/// Packs `sources` into a new archive at `target`.
pub fn compress(
    sources: &[PathBuf],
    target: &Path,
    format: Format,
    progress: &Progress,
) -> io::Result<()> {
    let entries = walk(sources)?;
    progress
        .total
        .store(total_size(&entries), Ordering::Relaxed);
    // Never replaces an existing file; only a file made here is cleaned up.
    let file = File::create_new(target)?;
    let result = match format {
        Format::Zip => compress_zip(&entries, file, progress),
        Format::TarGz => compress_tar(&entries, file, progress),
    };
    if result.is_err() {
        let _ = fs::remove_file(target);
    }
    result
}

fn compress_zip(
    entries: &[(PathBuf, String, bool)],
    file: File,
    progress: &Progress,
) -> io::Result<()> {
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    for (path, name, is_dir) in entries {
        progress.check()?;
        let metadata = fs::metadata(path)?;
        let mut options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(metadata.len() >= 0xFFFF_FFFF);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            options = options.unix_permissions(metadata.permissions().mode());
        }
        if let Ok(modified) = metadata.modified()
            && let Some(time) = zip_time(modified)
        {
            options = options.last_modified_time(time);
        }
        if *is_dir {
            zip.add_directory(name.trim_end_matches('/'), options)
                .map_err(io::Error::other)?;
        } else {
            zip.start_file(name.as_str(), options)
                .map_err(io::Error::other)?;
            let mut reader = BufReader::new(File::open(path)?);
            copy_counted(&mut reader, &mut zip, progress)?;
        }
    }
    zip.finish().map_err(io::Error::other)?.flush()
}

fn zip_time(time: std::time::SystemTime) -> Option<zip::DateTime> {
    let seconds = time.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    let local = gtk::glib::DateTime::from_unix_local(seconds).ok()?;
    zip::DateTime::from_date_and_time(
        local.year().clamp(1980, 2107) as u16,
        local.month() as u8,
        local.day_of_month() as u8,
        local.hour() as u8,
        local.minute() as u8,
        local.second() as u8,
    )
    .ok()
}

fn compress_tar(
    entries: &[(PathBuf, String, bool)],
    file: File,
    progress: &Progress,
) -> io::Result<()> {
    let encoder =
        flate2::write::GzEncoder::new(BufWriter::new(file), flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    tar.follow_symlinks(false);
    for (path, name, is_dir) in entries {
        progress.check()?;
        if *is_dir {
            tar.append_dir(name.trim_end_matches('/'), path)?;
        } else {
            let mut file = File::open(path)?;
            let metadata = file.metadata()?;
            let mut header = tar::Header::new_gnu();
            header.set_metadata(&metadata);
            header.set_cksum();
            let reader = CountingReader {
                inner: &mut file,
                progress,
            };
            tar.append_data(&mut header, name, reader)?;
        }
    }
    tar.into_inner()?.finish()?.flush()
}

struct CountingReader<'a, R> {
    inner: &'a mut R,
    progress: &'a Progress,
}

impl<R: Read> Read for CountingReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.progress.check()?;
        let read = self.inner.read(buf)?;
        self.progress.done.fetch_add(read as u64, Ordering::Relaxed);
        Ok(read)
    }
}

/// Whether "Extract All" can open this file.
pub fn can_extract(name: &str) -> bool {
    let name = name.to_lowercase();
    [".zip", ".tar", ".tar.gz", ".tgz"]
        .iter()
        .any(|ext| name.ends_with(ext))
}

/// What Patrone Unpack opens (unpack/README.md): archives, and single
/// compressed files such as `notes.txt.gz`. Longest endings first, so
/// `.tar.gz` is found before `.gz`.
const UNPACK_TYPES: &[&str] = &[
    ".tar.gz",
    ".tar.xz",
    ".tar.bz2",
    ".tar.zst",
    ".tar.lzma",
    ".tar.lz",
    ".tgz",
    ".txz",
    ".tbz2",
    ".tbz",
    ".tzst",
    ".tlz",
    ".zip",
    ".tar",
    ".7z",
    ".rar",
    ".cpio",
    ".cab",
    ".gz",
    ".xz",
    ".bz2",
    ".zst",
    ".lzma",
    ".lz",
];

fn archive_ending(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    UNPACK_TYPES
        .iter()
        .copied()
        .find(|ending| lower.ends_with(ending))
}

/// Patrone Unpack, when it's installed.
pub fn unpack() -> Option<PathBuf> {
    gtk::glib::find_program_in_path("patrone-unpack")
}

/// Whether Extract All… and Extract here can take this file: ZIP and TAR
/// here, any other archive through Patrone Unpack when it's installed.
pub fn extractable(name: &str) -> bool {
    can_extract(name) || (archive_ending(name).is_some() && unpack().is_some())
}

/// Starts Patrone Unpack with these arguments (it shows its own progress
/// and questions) and reaps it when it exits.
pub fn run_unpack<S: AsRef<std::ffi::OsStr>>(arguments: &[S]) -> bool {
    let Some(unpack) = unpack() else {
        return false;
    };
    match std::process::Command::new(unpack).args(arguments).spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            true
        }
        Err(_) => false,
    }
}

/// The folder name "Extract All" suggests: the archive's name without its
/// extension (`notes.txt` for `notes.txt.gz`).
pub fn folder_name_for(name: &str) -> String {
    match archive_ending(name) {
        Some(ending) if name.len() > ending.len() => name[..name.len() - ending.len()].to_owned(),
        _ => name.to_owned(),
    }
}

/// Unpacks `archive` into `target` (created if needed). Entries that would
/// land outside `target` are refused.
pub fn extract(archive: &Path, target: &Path, progress: &Progress) -> io::Result<()> {
    fs::create_dir_all(target)?;
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if name.ends_with(".zip") {
        extract_zip(archive, target, progress)
    } else {
        extract_tar(
            archive,
            target,
            progress,
            name.ends_with(".gz") || name.ends_with(".tgz"),
        )
    }
}

fn extract_zip(archive: &Path, target: &Path, progress: &Progress) -> io::Result<()> {
    let mut zip =
        zip::ZipArchive::new(BufReader::new(File::open(archive)?)).map_err(io::Error::other)?;
    let total: u64 = (0..zip.len())
        .filter_map(|index| zip.by_index_raw(index).ok().map(|entry| entry.size()))
        .sum();
    progress.total.store(total, Ordering::Relaxed);
    for index in 0..zip.len() {
        progress.check()?;
        let mut entry = zip.by_index(index).map_err(io::Error::other)?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsafe path in archive: {}",
                    String::from_utf8_lossy(entry.name_raw())
                ),
            ));
        };
        let path = target.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = BufWriter::new(File::create(&path)?);
        copy_counted(&mut entry, &mut out, progress)?;
        out.flush()?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o777));
        }
    }
    Ok(())
}

fn extract_tar(archive: &Path, target: &Path, progress: &Progress, gzip: bool) -> io::Result<()> {
    progress
        .total
        .store(fs::metadata(archive)?.len(), Ordering::Relaxed);
    let file = File::open(archive)?;
    let reader = CountingReader {
        inner: &mut BufReader::new(file),
        progress,
    };
    let reader: Box<dyn Read> = if gzip {
        Box::new(flate2::read::GzDecoder::new(reader))
    } else {
        Box::new(reader)
    };
    let mut tar = tar::Archive::new(reader);
    for entry in tar.entries()? {
        progress.check()?;
        // unpack_in refuses entries that would leave `target`.
        entry?.unpack_in(target)?;
    }
    Ok(())
}

/// The names inside a ZIP file, for searching inside archives.
pub fn zip_entry_names(path: &Path) -> Vec<String> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let Ok(zip) = zip::ZipArchive::new(BufReader::new(file)) else {
        return Vec::new();
    };
    zip.file_names()
        .filter_map(|name| name.ok().map(|name| name.into_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pfm-archive-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn zip_and_tar_round_trip() {
        for format in [Format::Zip, Format::TarGz] {
            let dir = scratch(&format!("{format:?}"));
            let source = dir.join("Project");
            fs::create_dir_all(source.join("sub")).unwrap();
            fs::write(source.join("a.txt"), "alpha").unwrap();
            fs::write(source.join("sub").join("b.txt"), "beta").unwrap();
            let archive = dir.join(format!("Project{}", format.extension()));
            compress(
                std::slice::from_ref(&source),
                &archive,
                format,
                &Progress::default(),
            )
            .unwrap();
            let out = dir.join("out");
            extract(&archive, &out, &Progress::default()).unwrap();
            assert_eq!(
                fs::read_to_string(out.join("Project/a.txt")).unwrap(),
                "alpha"
            );
            assert_eq!(
                fs::read_to_string(out.join("Project/sub/b.txt")).unwrap(),
                "beta"
            );
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn compress_refuses_to_overwrite() {
        let dir = scratch("exists");
        fs::write(dir.join("a.txt"), "a").unwrap();
        fs::write(dir.join("a.zip"), "old").unwrap();
        assert!(
            compress(
                &[dir.join("a.txt")],
                &dir.join("a.zip"),
                Format::Zip,
                &Progress::default()
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(dir.join("a.zip")).unwrap(), "old");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn suggested_folder_names_drop_the_extension() {
        assert_eq!(folder_name_for("Photos.zip"), "Photos");
        assert_eq!(folder_name_for("src.tar.gz"), "src");
        assert_eq!(folder_name_for("backup.tar.zst"), "backup");
        assert_eq!(folder_name_for("Data.7z"), "Data");
        assert_eq!(folder_name_for("notes.txt.gz"), "notes.txt");
        assert!(can_extract("A.TGZ"));
        assert!(!can_extract("a.7z"));
        assert_eq!(archive_ending("a.RAR"), Some(".rar"));
        assert_eq!(archive_ending("readme.txt"), None);
    }
}
