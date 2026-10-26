//! Thumbnails, following the freedesktop.org thumbnail spec so they're
//! shared with other apps: `~/.cache/thumbnails/{normal,large,x-large}`,
//! named by the MD5 of the file's URI and checked against its modification
//! time. Pictures are scaled here on a worker thread; other kinds (video,
//! PDF, fonts...) go through the thumbnailers installed in
//! `/usr/share/thumbnailers`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use gtk::{gdk, gdk_pixbuf, gio, glib, prelude::*};

use crate::util;

/// At most this many thumbnails are made at once.
const WORKERS: usize = 4;
const MEMORY_LIMIT: usize = 600;

type Callback = Box<dyn FnOnce(Option<gdk::Texture>)>;

struct Request {
    uri: String,
    path: PathBuf,
    mtime: i64,
    mime: String,
    bucket: Bucket,
    callbacks: Vec<Callback>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Bucket {
    Normal,
    Large,
    ExtraLarge,
}

impl Bucket {
    fn for_size(size: i32) -> Bucket {
        match size {
            0..=128 => Bucket::Normal,
            129..=256 => Bucket::Large,
            _ => Bucket::ExtraLarge,
        }
    }

    fn pixels(self) -> i32 {
        match self {
            Bucket::Normal => 128,
            Bucket::Large => 256,
            Bucket::ExtraLarge => 512,
        }
    }

    fn folder(self) -> &'static str {
        match self {
            Bucket::Normal => "normal",
            Bucket::Large => "large",
            Bucket::ExtraLarge => "x-large",
        }
    }
}

#[derive(Default)]
struct State {
    memory: HashMap<(String, i64, Bucket), gdk::Texture>,
    order: VecDeque<(String, i64, Bucket)>,
    failed: HashSet<(String, i64)>,
    queue: VecDeque<Request>,
    running: Vec<(String, Bucket)>,
    waiting: HashMap<(String, Bucket), Vec<Callback>>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
    static THUMBNAILERS: Vec<Thumbnailer> = load_thumbnailers();
    static ENABLED: Cell<bool> = const { Cell::new(true) };
}

/// Whether a file of this type can have a thumbnail.
pub fn can_thumbnail(mime: &str) -> bool {
    is_picture(mime) || THUMBNAILERS.with(|list| list.iter().any(|t| t.handles(mime)))
}

fn is_picture(mime: &str) -> bool {
    matches!(
        mime,
        "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/bmp"
            | "image/x-bmp"
            | "image/tiff"
            | "image/webp"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
            | "image/svg+xml"
            | "image/avif"
            | "image/heif"
            | "image/x-portable-pixmap"
            | "image/x-tga"
    )
}

/// Looks up or makes a thumbnail for a local file, at least `size` pixels.
/// `callback` runs on the main thread, possibly right away.
pub fn request(
    info: &gio::FileInfo,
    size: i32,
    callback: impl FnOnce(Option<gdk::Texture>) + 'static,
) {
    let Some(file) = util::file_of(info) else {
        return callback(None);
    };
    let (Some(path), Some(time)) = (file.path(), info.modification_date_time()) else {
        return callback(None);
    };
    let mime = util::content_type(info);
    if !can_thumbnail(&mime) {
        return callback(None);
    }
    let uri: String = file.uri().into();
    let mtime = time.to_unix();
    let bucket = Bucket::for_size(size);
    let callback: Callback = Box::new(callback);

    let cached = STATE.with(|state| {
        let state = state.borrow();
        if state.failed.contains(&(uri.clone(), mtime)) {
            return Some(None);
        }
        state
            .memory
            .get(&(uri.clone(), mtime, bucket))
            .cloned()
            .map(Some)
    });
    if let Some(texture) = cached {
        return callback(texture);
    }

    // GIO already found a valid thumbnail for this file.
    if bucket == Bucket::Normal
        && info.boolean("thumbnail::is-valid")
        && let Some(thumb) = info.attribute_byte_string("thumbnail::path")
        && let Ok(texture) = gdk::Texture::from_filename(thumb.as_str())
    {
        remember(&uri, mtime, bucket, &texture);
        return callback(Some(texture));
    }

    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let key = (uri.clone(), bucket);
        if let Some(waiting) = state.waiting.get_mut(&key) {
            waiting.push(callback);
            return;
        }
        state.waiting.insert(key, Vec::new());
        state.queue.push_back(Request {
            uri,
            path,
            mtime,
            mime,
            bucket,
            callbacks: vec![callback],
        });
    });
    pump();
}

/// Forgets queued requests, when a folder view is left.
pub fn cancel_pending() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let queue = std::mem::take(&mut state.queue);
        for request in queue {
            state.waiting.remove(&(request.uri, request.bucket));
        }
    });
}

fn remember(uri: &str, mtime: i64, bucket: Bucket, texture: &gdk::Texture) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let key = (uri.to_owned(), mtime, bucket);
        if state.memory.insert(key.clone(), texture.clone()).is_none() {
            state.order.push_back(key);
        }
        while state.order.len() > MEMORY_LIMIT {
            if let Some(old) = state.order.pop_front() {
                state.memory.remove(&old);
            }
        }
    });
}

fn pump() {
    loop {
        let request = STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.running.len() >= WORKERS {
                return None;
            }
            let request = state.queue.pop_front()?;
            state.running.push((request.uri.clone(), request.bucket));
            Some(request)
        });
        let Some(request) = request else { return };
        glib::spawn_future_local(run(request));
    }
}

async fn run(request: Request) {
    let Request {
        uri,
        path,
        mtime,
        mime,
        bucket,
        mut callbacks,
    } = request;
    let job_uri = uri.clone();
    let made = gio::spawn_blocking(move || find_or_make(&job_uri, &path, mtime, &mime, bucket))
        .await
        .ok()
        .flatten();
    let texture = made.and_then(|thumb| gdk::Texture::from_filename(thumb).ok());
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state
            .running
            .retain(|(running, b)| !(running == &uri && *b == bucket));
        if let Some(more) = state.waiting.remove(&(uri.clone(), bucket)) {
            callbacks.extend(more);
        }
        if texture.is_none() {
            state.failed.insert((uri.clone(), mtime));
        }
    });
    if let Some(texture) = &texture {
        remember(&uri, mtime, bucket, texture);
    }
    for callback in callbacks {
        callback(texture.clone());
    }
    pump();
}

fn cache_dir(bucket: Bucket) -> PathBuf {
    glib::user_cache_dir()
        .join("thumbnails")
        .join(bucket.folder())
}

fn cache_path(uri: &str, bucket: Bucket) -> PathBuf {
    let md5 = glib::compute_checksum_for_string(glib::ChecksumType::Md5, uri).unwrap_or_default();
    cache_dir(bucket).join(format!("{md5}.png"))
}

/// Runs on a worker thread: the path of a valid thumbnail, made if needed.
fn find_or_make(uri: &str, path: &Path, mtime: i64, mime: &str, bucket: Bucket) -> Option<PathBuf> {
    let target = cache_path(uri, bucket);
    if let Ok(existing) = gdk_pixbuf::Pixbuf::from_file(&target)
        && existing.option("tEXt::Thumb::MTime").as_deref() == Some(mtime.to_string().as_str())
    {
        return Some(target);
    }
    // A big thumbnail can stand in for a smaller one.
    if bucket == Bucket::Normal {
        let large = cache_path(uri, Bucket::Large);
        if let Ok(existing) = gdk_pixbuf::Pixbuf::from_file(&large)
            && existing.option("tEXt::Thumb::MTime").as_deref() == Some(mtime.to_string().as_str())
        {
            return Some(large);
        }
    }
    let pixels = bucket.pixels();
    let pixbuf = if is_picture(mime) {
        let pixbuf = gdk_pixbuf::Pixbuf::from_file_at_scale(path, pixels, pixels, true).ok()?;
        pixbuf.apply_embedded_orientation().unwrap_or(pixbuf)
    } else {
        let thumbnailer =
            THUMBNAILERS.with(|list| list.iter().find(|t| t.handles(mime)).cloned())?;
        let scratch = std::env::temp_dir().join(format!(
            "patrone-thumb-{}-{}.png",
            std::process::id(),
            glib::random_int()
        ));
        let ok = thumbnailer.run(uri, path, &scratch, pixels);
        let pixbuf = ok
            .then(|| gdk_pixbuf::Pixbuf::from_file(&scratch).ok())
            .flatten();
        let _ = std::fs::remove_file(&scratch);
        let pixbuf = pixbuf?;
        // Some thumbnailers ignore the size they're given.
        let (width, height) = (pixbuf.width(), pixbuf.height());
        if width > pixels || height > pixels {
            let scale = pixels as f64 / width.max(height) as f64;
            pixbuf.scale_simple(
                ((width as f64 * scale) as i32).max(1),
                ((height as f64 * scale) as i32).max(1),
                gdk_pixbuf::InterpType::Bilinear,
            )?
        } else {
            pixbuf
        }
    };
    save(&pixbuf, uri, mtime, &target).then_some(target)
}

fn save(pixbuf: &gdk_pixbuf::Pixbuf, uri: &str, mtime: i64, target: &Path) -> bool {
    let Some(dir) = target.parent() else {
        return false;
    };
    glib::mkdir_with_parents(dir, 0o700);
    // Written beside it and renamed, so no one reads half a file.
    let partial = target.with_extension(format!("png.{}", std::process::id()));
    let mtime = mtime.to_string();
    let saved = pixbuf
        .savev(
            &partial,
            "png",
            &[
                ("tEXt::Thumb::URI", uri),
                ("tEXt::Thumb::MTime", mtime.as_str()),
            ],
        )
        .is_ok();
    if saved && std::fs::rename(&partial, target).is_ok() {
        return true;
    }
    let _ = std::fs::remove_file(&partial);
    // Can't save to the cache: still show it, from the scratch copy.
    false
}

/// An installed thumbnailer (`/usr/share/thumbnailers/*.thumbnailer`).
#[derive(Clone)]
struct Thumbnailer {
    exec: String,
    mime_types: Vec<String>,
}

impl Thumbnailer {
    fn handles(&self, mime: &str) -> bool {
        self.mime_types.iter().any(|known| known == mime)
    }

    fn run(&self, uri: &str, path: &Path, output: &Path, size: i32) -> bool {
        let Ok(words) = glib::shell_parse_argv(&self.exec) else {
            return false;
        };
        let args: Vec<String> = words
            .iter()
            .map(|word| {
                let word = word.to_string_lossy();
                word.replace("%s", &size.to_string())
                    .replace("%u", uri)
                    .replace("%i", &path.to_string_lossy())
                    .replace("%o", &output.to_string_lossy())
                    .replace("%%", "%")
            })
            .collect();
        let Some((program, rest)) = args.split_first() else {
            return false;
        };
        std::process::Command::new(program)
            .args(rest)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

fn load_thumbnailers() -> Vec<Thumbnailer> {
    let mut dirs: Vec<PathBuf> = vec![glib::user_data_dir().join("thumbnailers")];
    dirs.extend(
        glib::system_data_dirs()
            .iter()
            .map(|dir| dir.join("thumbnailers")),
    );
    let mut list = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("thumbnailer") {
                continue;
            }
            let file = glib::KeyFile::new();
            if file
                .load_from_file(&path, glib::KeyFileFlags::NONE)
                .is_err()
            {
                continue;
            }
            const GROUP: &str = "Thumbnailer Entry";
            let Ok(exec) = file.string(GROUP, "Exec") else {
                continue;
            };
            if let Ok(try_exec) = file.string(GROUP, "TryExec")
                && glib::find_program_in_path(try_exec.as_str()).is_none()
            {
                continue;
            }
            let mime_types = file
                .string_list(GROUP, "MimeType")
                .map(|list| list.iter().map(|m| m.to_string()).collect())
                .unwrap_or_default();
            list.push(Thumbnailer {
                exec: exec.into(),
                mime_types,
            });
        }
    }
    list
}

/// Thumbnails are on unless Folder Options says "Always show icons".
pub fn set_enabled(enabled: bool) {
    ENABLED.with(|cell| cell.set(enabled));
}

pub fn enabled() -> bool {
    ENABLED.with(Cell::get)
}
