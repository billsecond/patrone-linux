//! The pane on the right: Details (a large icon or thumbnail and the
//! selection's properties) or Preview (pictures, text, video and sound).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::{gdk_pixbuf, gio, glib, prelude::*};

use crate::filecell;
use crate::icons;
use crate::location::Location;
use crate::thumbs;
use crate::util;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Details,
    Preview,
}

/// Text files bigger than this preview only their beginning.
const TEXT_LIMIT: usize = 256 * 1024;

pub struct SidePane {
    pub widget: gtk::Stack,
    mode: Cell<Mode>,
    // Details
    icon: gtk::Image,
    picture: gtk::Picture,
    name: gtk::Label,
    kind: gtk::Label,
    properties: gtk::Grid,
    properties_heading: gtk::Label,
    // Preview
    preview: gtk::Stack,
    preview_picture: gtk::Picture,
    preview_text: gtk::TextView,
    preview_video: gtk::Video,
    preview_message: gtk::Label,
    generation: Rc<Cell<u64>>,
    shown: RefCell<Option<String>>,
}

impl SidePane {
    pub fn new() -> Rc<SidePane> {
        let icon = gtk::Image::builder()
            .pixel_size(96)
            .halign(gtk::Align::Center)
            .build();
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .height_request(200)
            .visible(false)
            .build();
        picture.add_css_class("details-picture");
        let name = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .width_chars(16)
            .build();
        name.add_css_class("details-name");
        let kind = gtk::Label::builder().xalign(0.0).build();
        kind.add_css_class("item-detail");
        let properties_heading = gtk::Label::builder()
            .label("Properties")
            .xalign(0.0)
            .build();
        properties_heading.add_css_class("details-heading");
        let properties = gtk::Grid::builder()
            .row_spacing(10)
            .column_spacing(12)
            .build();
        properties.add_css_class("details-properties");
        let details = gtk::Box::new(gtk::Orientation::Vertical, 8);
        details.add_css_class("details-pane");
        details.append(&icon);
        details.append(&picture);
        details.append(&name);
        details.append(&kind);
        details.append(&properties_heading);
        details.append(&properties);
        let details_scroller = gtk::ScrolledWindow::builder()
            .child(&details)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        let preview_picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .build();
        let preview_text = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .build();
        preview_text.add_css_class("preview-text");
        let text_scroller = gtk::ScrolledWindow::builder().child(&preview_text).build();
        let preview_video = gtk::Video::builder().autoplay(false).build();
        let preview_message = gtk::Label::builder()
            .label("Select a file to preview.")
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        preview_message.add_css_class("empty-folder");
        let preview = gtk::Stack::new();
        preview.add_css_class("preview-pane");
        preview.add_named(&preview_message, Some("message"));
        preview.add_named(&preview_picture, Some("picture"));
        preview.add_named(&text_scroller, Some("text"));
        preview.add_named(&preview_video, Some("video"));

        let widget = gtk::Stack::builder()
            .width_request(220)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .build();
        widget.add_css_class("side-pane");
        widget.add_named(&details_scroller, Some("details"));
        widget.add_named(&preview, Some("preview"));
        Rc::new(SidePane {
            widget,
            mode: Cell::new(Mode::Details),
            icon,
            picture,
            name,
            kind,
            properties,
            properties_heading,
            preview,
            preview_picture,
            preview_text,
            preview_video,
            preview_message,
            generation: Rc::new(Cell::new(0)),
            shown: RefCell::default(),
        })
    }

    pub fn set_mode(&self, mode: Mode) {
        self.mode.set(mode);
        self.shown.replace(None);
        self.widget.set_visible_child_name(match mode {
            Mode::Details => "details",
            Mode::Preview => "preview",
        });
        if mode == Mode::Details {
            self.preview_video.set_file(None::<&gio::File>);
        }
    }

    /// Shows what's selected (or the place, when nothing is).
    pub fn show(&self, location: &Location, selected: &[gio::FileInfo], n_items: u32) {
        if !self.widget.is_visible() {
            return;
        }
        let key = format!(
            "{:?}|{}|{}",
            self.mode.get(),
            location.key(),
            selected
                .iter()
                .filter_map(util::file_of)
                .map(|file| file.uri().to_string())
                .collect::<Vec<_>>()
                .join("|")
        );
        if self.shown.borrow().as_deref() == Some(key.as_str()) && !selected.is_empty() {
            return;
        }
        self.shown.replace(Some(key));
        self.generation.set(self.generation.get() + 1);
        match self.mode.get() {
            Mode::Details => self.show_details(location, selected, n_items),
            Mode::Preview => self.show_preview(selected),
        }
    }

    fn clear_properties(&self) {
        while let Some(child) = self.properties.first_child() {
            self.properties.remove(&child);
        }
    }

    fn add_property(&self, row: &mut i32, label: &str, value: &str) {
        if value.is_empty() {
            return;
        }
        let key = gtk::Label::builder()
            .label(label)
            .xalign(0.0)
            .yalign(0.0)
            .build();
        key.add_css_class("item-detail");
        let value = gtk::Label::builder()
            .label(value)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .width_chars(14)
            .hexpand(true)
            .build();
        self.properties.attach(&key, 0, *row, 1, 1);
        self.properties.attach(&value, 1, *row, 1, 1);
        *row += 1;
    }

    fn show_details(&self, location: &Location, selected: &[gio::FileInfo], n_items: u32) {
        self.clear_properties();
        self.picture.set_visible(false);
        self.icon.set_visible(true);
        let mut row = 0;
        match selected {
            [] => {
                let icon = match location {
                    Location::Home => "pfm-home",
                    Location::Gallery => "pfm-gallery",
                    Location::ThisPc => "pfm-computer",
                    Location::Folder(file) => icons::folder_icon(file),
                    Location::Search { .. } => "pfm-folder",
                };
                icons::set_image(&self.icon, icon, 96);
                self.name.set_label(&location.title());
                let count = if n_items == 1 {
                    "1 item".to_owned()
                } else {
                    format!("{} items", util::thousands(n_items as u64))
                };
                self.kind.set_label(&count);
                self.properties_heading.set_visible(false);
                if let Location::Folder(file) = location {
                    self.properties_heading.set_visible(true);
                    self.add_property(&mut row, "Location", &util::display_path(file));
                }
            }
            [info] => {
                icons::set_image(&self.icon, icons::for_info(info), 96);
                let generation = self.generation.get();
                let picture = self.picture.clone();
                let icon = self.icon.clone();
                let current = self.generation.clone();
                thumbs::request(
                    info,
                    256 * self.widget.scale_factor().max(1),
                    move |texture| {
                        if current.get() != generation {
                            return;
                        }
                        if let Some(texture) = texture {
                            picture.set_paintable(Some(&texture));
                            picture.set_visible(true);
                            icon.set_visible(false);
                        }
                    },
                );
                self.name.set_label(&info.display_name());
                self.kind.set_label(&filecell::type_text(info));
                self.properties_heading.set_visible(true);
                let in_trash = info.has_attribute("trash::orig-path");
                if in_trash {
                    self.add_property(
                        &mut row,
                        "Original location",
                        &filecell::column_text(info, crate::viewstate::Column::Origin, false),
                    );
                    self.add_property(
                        &mut row,
                        "Date deleted",
                        &filecell::column_text(info, crate::viewstate::Column::Deleted, false),
                    );
                }
                if !util::is_dir(info) {
                    self.add_property(&mut row, "Size", &util::size(info.size().max(0) as u64));
                }
                if let Some(time) = util::modified(info) {
                    self.add_property(&mut row, "Date modified", &util::date_time(&time));
                }
                if let Some(time) = util::created(info) {
                    self.add_property(&mut row, "Date created", &util::date_time(&time));
                }
                if let Some(path) = util::file_of(info).and_then(|file| file.path())
                    && util::content_type(info).starts_with("image/")
                    && let Some((_, width, height)) = gdk_pixbuf::Pixbuf::file_info(&path)
                {
                    self.add_property(&mut row, "Dimensions", &format!("{width} x {height}"));
                }
                if !in_trash
                    && let Some(folder) = util::file_of(info).and_then(|file| file.parent())
                {
                    self.add_property(&mut row, "Location", &util::display_path(&folder));
                }
                self.add_property(
                    &mut row,
                    "Owner",
                    &filecell::column_text(info, crate::viewstate::Column::Owner, false),
                );
                self.add_property(&mut row, "Permissions", &filecell::permissions_text(info));
            }
            many => {
                icons::set_image(&self.icon, "pfm-file", 96);
                self.name.set_label(&format!(
                    "{} items selected",
                    util::thousands(many.len() as u64)
                ));
                let files: Vec<&gio::FileInfo> =
                    many.iter().filter(|info| !util::is_dir(info)).collect();
                let folders = many.len() - files.len();
                self.kind.set_label(&match (files.len(), folders) {
                    (0, _) => "Folders".to_owned(),
                    (_, 0) => "Files".to_owned(),
                    _ => "Files and folders".to_owned(),
                });
                self.properties_heading.set_visible(true);
                if !files.is_empty() {
                    let total: u64 = files.iter().map(|info| info.size().max(0) as u64).sum();
                    self.add_property(&mut row, "Total size", &util::size(total));
                }
                let times: Vec<i64> = many
                    .iter()
                    .filter_map(util::modified)
                    .map(|t| t.to_unix())
                    .collect();
                if let (Some(first), Some(last)) = (times.iter().min(), times.iter().max()) {
                    let format = |seconds: i64| {
                        glib::DateTime::from_unix_local(seconds)
                            .map(|t| util::date_time(&t))
                            .unwrap_or_default()
                    };
                    let range = if first == last {
                        format(*first)
                    } else {
                        format!("{} - {}", format(*first), format(*last))
                    };
                    self.add_property(&mut row, "Date modified", &range);
                }
            }
        }
    }

    fn show_preview(&self, selected: &[gio::FileInfo]) {
        self.preview_video.set_file(None::<&gio::File>);
        let [info] = selected else {
            self.preview_message.set_label(if selected.is_empty() {
                "Select a file to preview."
            } else {
                "No preview available."
            });
            self.preview.set_visible_child_name("message");
            return;
        };
        if !crate::config::flag("show-preview-handlers") || util::is_dir(info) {
            self.preview_message.set_label("No preview available.");
            self.preview.set_visible_child_name("message");
            return;
        }
        let Some(file) = util::file_of(info) else {
            return;
        };
        let mime = util::content_type(info);
        if mime.starts_with("image/") {
            match gtk::gdk::Texture::from_file(&file) {
                Ok(texture) => {
                    self.preview_picture.set_paintable(Some(&texture));
                    self.preview.set_visible_child_name("picture");
                }
                Err(_) => {
                    self.preview_message
                        .set_label("This file can't be previewed.");
                    self.preview.set_visible_child_name("message");
                }
            }
        } else if mime.starts_with("video/") || mime.starts_with("audio/") {
            self.preview_video.set_file(Some(&file));
            self.preview.set_visible_child_name("video");
        } else if gio::content_type_is_a(&mime, "text/plain") {
            self.preview_message.set_label("Loading…");
            self.preview.set_visible_child_name("message");
            let generation = self.generation.get();
            let current = self.generation.clone();
            let text = self.preview_text.clone();
            let stack = self.preview.clone();
            let message = self.preview_message.clone();
            glib::spawn_future_local(async move {
                let result = file.load_contents_future().await;
                if current.get() != generation {
                    return;
                }
                match result {
                    Ok((bytes, _)) => {
                        let end = bytes.len().min(TEXT_LIMIT);
                        text.buffer()
                            .set_text(&String::from_utf8_lossy(&bytes[..end]));
                        stack.set_visible_child_name("text");
                    }
                    Err(_) => {
                        message.set_label("This file can't be previewed.");
                        stack.set_visible_child_name("message");
                    }
                }
            });
        } else {
            // PDFs, fonts and others: their thumbnail, large.
            self.preview_message.set_label("No preview available.");
            self.preview.set_visible_child_name("message");
            let generation = self.generation.get();
            let current = self.generation.clone();
            let picture = self.preview_picture.clone();
            let stack = self.preview.clone();
            thumbs::request(info, 512, move |texture| {
                if current.get() != generation {
                    return;
                }
                if let Some(texture) = texture {
                    picture.set_paintable(Some(&texture));
                    stack.set_visible_child_name("picture");
                }
            });
        }
    }
}
