//! Cut, copy and paste of files, in the formats other Linux file managers
//! use (`x-special/gnome-copied-files` and `text/uri-list`), so files move
//! between this and other apps. Cut items look faded until they're pasted
//! or something else is copied.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gtk::{gdk, gio, glib, prelude::*};

use crate::util;

const GNOME_FORMAT: &str = "x-special/gnome-copied-files";

thread_local! {
    static CUT: RefCell<HashSet<String>> = RefCell::default();
    static LISTENERS: RefCell<Vec<Rc<dyn Fn()>>> = RefCell::default();
    static WATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn clipboard() -> gdk::Clipboard {
    gdk::Display::default().expect("a display").clipboard()
}

/// Whether an item was cut and is waiting to be pasted.
pub fn is_cut(info: &gio::FileInfo) -> bool {
    let empty = CUT.with(|cut| cut.borrow().is_empty());
    if empty {
        return false;
    }
    util::file_of(info)
        .is_some_and(|file| CUT.with(|cut| cut.borrow().contains(file.uri().as_str())))
}

/// Runs `listener` whenever the set of cut items changes.
pub fn connect_cut_changed(listener: impl Fn() + 'static) {
    LISTENERS.with(|listeners| listeners.borrow_mut().push(Rc::new(listener)));
}

fn notify() {
    let listeners: Vec<_> = LISTENERS.with(|listeners| listeners.borrow().clone());
    for listener in listeners {
        listener();
    }
}

fn watch() {
    if WATCHING.with(|watching| watching.replace(true)) {
        return;
    }
    clipboard().connect_changed(|clipboard| {
        // Another app took the clipboard: nothing is cut any more.
        if !clipboard.is_local() && CUT.with(|cut| !cut.borrow().is_empty()) {
            CUT.with(|cut| cut.borrow_mut().clear());
            notify();
        }
    });
}

/// Puts files on the clipboard to copy, or to move when `cut`.
pub fn set_files(files: &[gio::File], cut: bool) {
    if files.is_empty() {
        return;
    }
    watch();
    let uris: Vec<String> = files.iter().map(|file| file.uri().to_string()).collect();
    let gnome = format!("{}\n{}", if cut { "cut" } else { "copy" }, uris.join("\n"));
    let paths: Vec<String> = files.iter().map(util::display_path).collect();
    let providers = [
        gdk::ContentProvider::for_value(&gdk::FileList::from_array(files).to_value()),
        gdk::ContentProvider::for_bytes(GNOME_FORMAT, &glib::Bytes::from_owned(gnome.into_bytes())),
        gdk::ContentProvider::for_bytes(
            "text/plain;charset=utf-8",
            &glib::Bytes::from_owned(paths.join("\n").into_bytes()),
        ),
    ];
    let _ = clipboard().set_content(Some(&gdk::ContentProvider::new_union(&providers)));
    CUT.with(|set| {
        let mut set = set.borrow_mut();
        set.clear();
        if cut {
            set.extend(uris);
        }
    });
    notify();
}

/// Copies text, as "Copy as path" does.
pub fn set_text(text: &str) {
    watch();
    clipboard().set_text(text);
    if CUT.with(|cut| !cut.borrow().is_empty()) {
        CUT.with(|cut| cut.borrow_mut().clear());
        notify();
    }
}

/// Whether the clipboard has files to paste.
pub fn has_files() -> bool {
    let formats = clipboard().formats();
    formats.contain_mime_type(GNOME_FORMAT)
        || formats.contain_mime_type("text/uri-list")
        || formats.contains_type(gdk::FileList::static_type())
}

/// The files on the clipboard, and whether they were cut.
pub async fn files() -> Option<(Vec<gio::File>, bool)> {
    let clipboard = clipboard();
    let formats = clipboard.formats();
    if formats.contain_mime_type(GNOME_FORMAT)
        && let Ok((stream, _)) = clipboard
            .read_future(&[GNOME_FORMAT], glib::Priority::DEFAULT)
            .await
        && let Some(text) = read_all(&stream).await
    {
        let mut lines = text.lines();
        let cut = lines.next().map(str::trim) == Some("cut");
        let files: Vec<gio::File> = lines
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(gio::File::for_uri)
            .collect();
        if !files.is_empty() {
            return Some((files, cut));
        }
    }
    let value = clipboard
        .read_value_future(gdk::FileList::static_type(), glib::Priority::DEFAULT)
        .await
        .ok()?;
    let files = value.get::<gdk::FileList>().ok()?.files();
    (!files.is_empty()).then_some((files, false))
}

/// After a cut-and-paste the items are gone from where they were.
pub fn finish_cut() {
    if CUT.with(|cut| !cut.borrow().is_empty()) {
        CUT.with(|cut| cut.borrow_mut().clear());
        let _ = clipboard().set_content(None::<&gdk::ContentProvider>);
        notify();
    }
}

async fn read_all(stream: &gio::InputStream) -> Option<String> {
    let mut data = Vec::new();
    loop {
        let bytes = stream
            .read_bytes_future(65536, glib::Priority::DEFAULT)
            .await
            .ok()?;
        if bytes.is_empty() {
            break;
        }
        data.extend_from_slice(&bytes);
    }
    String::from_utf8(data).ok()
}
