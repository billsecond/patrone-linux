//! Formatting sizes and dates the way Windows does, and small helpers.

use std::cmp::Ordering;

use gtk::{gio, glib, prelude::*};

/// Where a widget sends its events: one handler, set once.
pub type Handler<T> = std::cell::RefCell<Option<std::rc::Rc<dyn Fn(T)>>>;

mod header_box {
    use super::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct HeaderBox;

    #[glib::object_subclass]
    impl ObjectSubclass for HeaderBox {
        const NAME: &'static str = "PfmHeaderBox";
        type Type = super::HeaderBox;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("headerbar");
        }
    }

    impl ObjectImpl for HeaderBox {}
    impl WidgetImpl for HeaderBox {}
    impl BoxImpl for HeaderBox {}
}

glib::wrapper! {
    pub struct HeaderBox(ObjectSubclass<header_box::HeaderBox>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

/// The row of a title bar, named `headerbar` for style sheets, so the title
/// bar style chosen in Patrone Settings (or any GTK theme) draws it as it
/// draws every other app's title bar, active and inactive.
pub fn header_box() -> gtk::Box {
    glib::Object::new::<HeaderBox>().upcast()
}

/// GTK's own minimize, maximize and close buttons for one end of a title
/// bar. Like every other app's, they show in the corner the account chose
/// (GNOME's button-layout; the other end's set is empty and hidden), only
/// those the window can use, and in Patrone Settings' title bar button
/// style.
pub fn window_controls(side: gtk::PackType) -> gtk::WindowControls {
    let controls = gtk::WindowControls::new(side);
    controls.set_valign(gtk::Align::Center);
    controls
}

/// 1234567 → "1,234,567".
pub fn thousands(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The Size column of Details view: whole kilobytes, rounded up.
pub fn size_kb(bytes: u64) -> String {
    format!("{} KB", thousands(bytes.div_ceil(1024)))
}

/// Three significant digits: "512 bytes", "1.20 KB", "12.3 MB", "123 GB".
pub fn size(bytes: u64) -> String {
    if bytes < 1024 {
        return if bytes == 1 {
            "1 byte".into()
        } else {
            format!("{bytes} bytes")
        };
    }
    let units = ["KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let text = if value < 10.0 {
        format!("{:.2}", (value * 100.0).floor() / 100.0)
    } else if value < 100.0 {
        format!("{:.1}", (value * 10.0).floor() / 10.0)
    } else {
        format!("{:.0}", value.floor())
    };
    format!("{text} {}", units[unit])
}

/// "1.20 MB (1,258,291 bytes)", as in Properties.
pub fn size_with_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return size(bytes);
    }
    format!("{} ({} bytes)", size(bytes), thousands(bytes))
}

/// Date and time in the locale's short date, and the time without seconds.
pub fn date_time(time: &glib::DateTime) -> String {
    let local = time.to_local().unwrap_or_else(|_| time.clone());
    let date = local.format("%x").map(String::from).unwrap_or_default();
    format!("{date} {}", clock(&local))
}

/// "3:45 PM" where the locale uses a 12-hour clock, else "15:45".
pub fn clock(time: &glib::DateTime) -> String {
    let twelve_hour = time
        .format("%p")
        .map(|ampm| !ampm.is_empty())
        .unwrap_or(false);
    let format = if twelve_hour { "%l:%M %p" } else { "%H:%M" };
    time.format(format)
        .map(|text| text.trim().to_owned())
        .unwrap_or_default()
}

/// The long form Properties uses: "Monday, October 26, 2026, 3:45:12 PM".
pub fn date_time_long(time: &glib::DateTime) -> String {
    let local = time.to_local().unwrap_or_else(|_| time.clone());
    let date = local
        .format("%A, %B %-d, %Y")
        .map(String::from)
        .unwrap_or_default();
    let twelve_hour = local
        .format("%p")
        .map(|ampm| !ampm.is_empty())
        .unwrap_or(false);
    let format = if twelve_hour {
        "%l:%M:%S %p"
    } else {
        "%H:%M:%S"
    };
    let clock = local
        .format(format)
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    format!("{date}, {clock}")
}

pub fn modified(info: &gio::FileInfo) -> Option<glib::DateTime> {
    info.modification_date_time()
}

pub fn created(info: &gio::FileInfo) -> Option<glib::DateTime> {
    info.creation_date_time()
}

pub fn accessed(info: &gio::FileInfo) -> Option<glib::DateTime> {
    info.access_date_time()
}

/// The `GFile` a `GFileInfo` from a `GtkDirectoryList` (or the search) describes.
pub fn file_of(info: &gio::FileInfo) -> Option<gio::File> {
    info.attribute_object("standard::file")
        .and_then(|object| object.downcast::<gio::File>().ok())
}

pub fn is_dir(info: &gio::FileInfo) -> bool {
    matches!(
        info.file_type(),
        gio::FileType::Directory | gio::FileType::Mountable
    )
}

pub fn content_type(info: &gio::FileInfo) -> String {
    info.content_type()
        .map(String::from)
        .or_else(|| {
            info.attribute_string("standard::fast-content-type")
                .map(String::from)
        })
        .unwrap_or_else(|| "application/octet-stream".to_owned())
}

/// Windows' "natural" order: case-insensitive, with numbers by value, so
/// "file2" comes before "file10".
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    // Case and leading zeros only break ties.
    natural_cmp_folded(a, b).then_with(|| a.cmp(b))
}

fn natural_cmp_folded(a: &str, b: &str) -> Ordering {
    let mut a = a.chars().peekable();
    let mut b = b.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let left = take_digits(&mut a);
                let right = take_digits(&mut b);
                let order = left.len().cmp(&right.len()).then_with(|| left.cmp(&right));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// A run of digits, without its leading zeros.
fn take_digits(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut digits = String::new();
    while let Some(digit) = chars.peek().copied().filter(char::is_ascii_digit) {
        if !(digits.is_empty() && digit == '0') {
            digits.push(digit);
        }
        chars.next();
    }
    digits
}

/// Splits "report.final.pdf" into ("report.final", ".pdf"). Hidden files
/// with no other dot (".bashrc") have no extension.
pub fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(dot) => name.split_at(dot),
    }
}

/// A name that doesn't exist in `folder` yet: "New folder", "New folder (2)"...
pub fn unique_name(folder: &gio::File, base: &str, extension: &str) -> String {
    let mut candidate = format!("{base}{extension}");
    let mut number = 2;
    while folder
        .child(&candidate)
        .query_exists(gio::Cancellable::NONE)
    {
        candidate = format!("{base} ({number}){extension}");
        number += 1;
    }
    candidate
}

/// The name a copy gets in its own folder: "notes - Copy.txt", then
/// "notes - Copy (2).txt".
pub fn copy_name(folder: &gio::File, name: &str, is_dir: bool) -> String {
    let (stem, extension) = if is_dir {
        (name, "")
    } else {
        split_extension(name)
    };
    unique_name(folder, &format!("{stem} - Copy"), extension)
}

/// The path to show for a file: a local path, else its URI.
pub fn display_path(file: &gio::File) -> String {
    match file.path() {
        Some(path) => path.to_string_lossy().into_owned(),
        None => file.parse_name().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_windows() {
        assert_eq!(size(0), "0 bytes");
        assert_eq!(size(1), "1 byte");
        assert_eq!(size(1023), "1023 bytes");
        assert_eq!(size(1024), "1.00 KB");
        assert_eq!(size(1_258_291), "1.19 MB");
        assert_eq!(size(15_000), "14.6 KB");
        assert_eq!(size(300_000), "292 KB");
        assert_eq!(size_kb(0), "0 KB");
        assert_eq!(size_kb(1), "1 KB");
        assert_eq!(size_kb(1_263_616), "1,234 KB");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn natural_order_puts_numbers_by_value() {
        let mut names = vec!["file10", "File2", "file1", "file02", "apple", "Banana"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            ["apple", "Banana", "file1", "File2", "file02", "file10"]
        );
    }

    #[test]
    fn extensions_split_at_the_last_dot() {
        assert_eq!(
            split_extension("report.final.pdf"),
            ("report.final", ".pdf")
        );
        assert_eq!(split_extension(".bashrc"), (".bashrc", ""));
        assert_eq!(split_extension("Makefile"), ("Makefile", ""));
    }
}
