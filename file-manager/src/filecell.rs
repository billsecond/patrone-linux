//! One item in a file list, in any view: its icon or thumbnail and name, the
//! extra lines Tiles and Content show, one column of Details, the check box
//! of "Item check boxes", and the editor for renaming in place.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, pango, prelude::*};

use crate::glyph::Glyph;
use crate::icons;
use crate::thumbs;
use crate::util;
use crate::viewstate::Column;

/// What a cell shows, which depends on the view.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CellKind {
    /// Extra large, large and medium icons: the icon above the name.
    Grid(i32),
    /// Small icons and List: a small icon beside the name.
    Row,
    /// A medium icon beside the name, type and size.
    Tile,
    /// One wide row with the name, type, date and size.
    Content,
    /// Details' Name column.
    DetailName,
    /// Another column of Details.
    Detail(Column),
}

/// Files dropped on a folder: the files, the folder, and copy or move.
pub type DropHandler = dyn Fn(Vec<gio::File>, gio::File, gdk::DragAction);

/// What every cell of one file list shares.
pub struct CellContext {
    pub selection: gtk::MultiSelection,
    pub hide_extensions: Cell<bool>,
    pub tooltips: Cell<bool>,
    pub checkboxes: Cell<bool>,
    /// "System folder hints": say what system folders are for.
    pub hints: Cell<bool>,
    pub in_trash: Cell<bool>,
    pub cells: RefCell<Vec<glib::WeakRef<FileCell>>>,
    /// Files dropped on a folder: the files, the folder, and copy or move.
    pub on_drop: RefCell<Option<Rc<DropHandler>>>,
}

impl CellContext {
    pub fn new(selection: &gtk::MultiSelection) -> Rc<CellContext> {
        Rc::new(CellContext {
            selection: selection.clone(),
            hide_extensions: Cell::new(true),
            tooltips: Cell::new(true),
            checkboxes: Cell::new(false),
            hints: Cell::new(false),
            in_trash: Cell::new(false),
            cells: RefCell::default(),
            on_drop: RefCell::default(),
        })
    }

    /// The cell showing `position`, if it's on screen.
    pub fn cell_at(&self, position: u32) -> Option<FileCell> {
        self.cells
            .borrow()
            .iter()
            .filter_map(|weak| weak.upgrade())
            .find(|cell| {
                cell.imp().bound.get()
                    && cell.position() == position
                    && !matches!(cell.kind(), CellKind::Detail(_))
            })
    }

    pub fn selected_files(&self) -> Vec<gio::File> {
        let selected = self.selection.selection();
        let mut files = Vec::new();
        for index in 0..selected.size() {
            let position = selected.nth(index as u32);
            if let Some(info) = self
                .selection
                .item(position)
                .and_downcast::<gio::FileInfo>()
                && let Some(file) = util::file_of(&info)
            {
                files.push(file);
            }
        }
        files
    }
}

thread_local! {
    static KNOWN_EXTENSIONS: RefCell<HashMap<String, bool>> = RefCell::default();
}

/// Whether Windows would call this extension "known": one registered to a
/// file type.
fn is_known_extension(extension: &str) -> bool {
    let extension = extension.to_lowercase();
    KNOWN_EXTENSIONS.with(|known| {
        *known
            .borrow_mut()
            .entry(extension.clone())
            .or_insert_with(|| {
                let (content_type, _) =
                    gio::content_type_guess(Some(format!("x{extension}")), None);
                content_type != "application/octet-stream"
            })
    })
}

/// The name a list shows, with the extension hidden when Folder Options
/// says "Hide extensions for known file types".
pub fn shown_name(info: &gio::FileInfo, hide_extensions: bool) -> String {
    let name = info.display_name().to_string();
    if !hide_extensions || util::is_dir(info) {
        return name;
    }
    let (stem, extension) = util::split_extension(&name);
    if !extension.is_empty() && is_known_extension(extension) {
        stem.to_owned()
    } else {
        name
    }
}

/// A system folder hint as a tooltip: what it's for, and what plays the
/// same part on Windows.
fn hint_text(hint: &crate::hints::Hint) -> String {
    let mut text = format!("{}: {}", hint.tag, hint.about);
    if let Some(windows) = hint.windows {
        text.push_str(&format!("\nOn Windows: {windows}"));
    }
    text
}

/// The Type column: "File folder", "PNG image"...
pub fn type_text(info: &gio::FileInfo) -> String {
    if util::is_dir(info) {
        return "File folder".into();
    }
    if info.is_symlink() && info.file_type() == gio::FileType::SymbolicLink {
        return "Shortcut".into();
    }
    let description = gio::content_type_get_description(&util::content_type(info));
    let mut chars = description.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "File".into(),
    }
}

/// The text of one column for an item.
pub fn column_text(info: &gio::FileInfo, column: Column, hide_extensions: bool) -> String {
    let date =
        |time: Option<glib::DateTime>| time.map(|time| util::date_time(&time)).unwrap_or_default();
    match column {
        Column::Name => shown_name(info, hide_extensions),
        Column::Modified => date(util::modified(info)),
        Column::Created => date(util::created(info)),
        Column::Accessed => date(util::accessed(info)),
        Column::Type => type_text(info),
        Column::Size => {
            if util::is_dir(info) {
                String::new()
            } else {
                util::size_kb(info.size().max(0) as u64)
            }
        }
        Column::Owner => info
            .attribute_string("owner::user")
            .map(String::from)
            .unwrap_or_default(),
        Column::Permissions => permissions_text(info),
        Column::Deleted => date(deletion_date(info)),
        Column::Origin => info
            .attribute_byte_string("trash::orig-path")
            .and_then(|path| {
                std::path::Path::new(path.as_str())
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .unwrap_or_default(),
        Column::Folder => util::file_of(info)
            .and_then(|file| file.parent())
            .map(|parent| util::display_path(&parent))
            .unwrap_or_default(),
        Column::Purpose => util::file_of(info)
            .and_then(|file| crate::hints::for_file(&file))
            .map(|hint| hint.tag.to_owned())
            .unwrap_or_default(),
    }
}

/// When an item went into the Recycle Bin.
pub fn deletion_date(info: &gio::FileInfo) -> Option<glib::DateTime> {
    let text = info.attribute_string("trash::deletion-date")?;
    glib::DateTime::from_iso8601(&text, Some(&glib::TimeZone::local())).ok()
}

/// "rwxr-xr-x", as `ls -l` shows it.
pub fn permissions_text(info: &gio::FileInfo) -> String {
    if !info.has_attribute("unix::mode") {
        return String::new();
    }
    let mode = info.attribute_uint32("unix::mode");
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    bits.iter()
        .map(|(bit, letter)| if mode & bit != 0 { *letter } else { '-' })
        .collect()
}

/// The tooltip Windows shows when pointing at an item.
pub fn tooltip_text(info: &gio::FileInfo) -> String {
    let mut lines = vec![format!("Type: {}", type_text(info))];
    if !util::is_dir(info) {
        lines.push(format!("Size: {}", util::size(info.size().max(0) as u64)));
    }
    if let Some(time) = util::modified(info) {
        lines.push(format!("Date modified: {}", util::date_time(&time)));
    }
    lines.join("\n")
}

/// What `begin_rename` hooks onto the window, for `cancel_rename` to undo.
pub struct RenameHooks {
    window: glib::WeakRef<gtk::Window>,
    click: gtk::GestureClick,
    active: glib::SignalHandlerId,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FileCell {
        pub kind: Cell<Option<CellKind>>,
        pub info: RefCell<Option<gio::FileInfo>>,
        /// The list item showing this cell: its position stays current
        /// when items are added above it, without a rebind.
        pub list_item: RefCell<Option<glib::WeakRef<gtk::ListItem>>>,
        pub bound: Cell<bool>,
        /// Bumped on every bind, so a late thumbnail can't land on a reused cell.
        pub generation: Cell<u64>,
        pub icon: RefCell<Option<gtk::Image>>,
        pub link: RefCell<Option<Glyph>>,
        pub name: RefCell<Option<gtk::Label>>,
        pub lines: RefCell<Vec<gtk::Label>>,
        pub check: RefCell<Option<gtk::CheckButton>>,
        pub editor: RefCell<Option<gtk::Entry>>,
        /// The name of the file being renamed, and where it is in the list.
        pub editing: RefCell<Option<(std::ffi::OsString, u32)>>,
        /// While renaming: the window's click watcher and its is-active
        /// handler, which end the edit as Windows does.
        pub rename_hooks: RefCell<Option<RenameHooks>>,
        pub context: RefCell<Option<Rc<CellContext>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FileCell {
        const NAME: &'static str = "PfmFileCell";
        type Type = super::FileCell;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("filecell");
        }
    }

    impl ObjectImpl for FileCell {}
    impl WidgetImpl for FileCell {}
    impl BoxImpl for FileCell {}
}

glib::wrapper! {
    pub struct FileCell(ObjectSubclass<imp::FileCell>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl FileCell {
    pub fn new(kind: CellKind, context: &Rc<CellContext>) -> FileCell {
        let cell: FileCell = glib::Object::new();
        let imp = cell.imp();
        imp.kind.set(Some(kind));
        imp.context.replace(Some(context.clone()));
        context.cells.borrow_mut().push(cell.downgrade());

        let check = gtk::CheckButton::new();
        check.add_css_class("item-check");
        check.set_visible(false);
        check.set_valign(gtk::Align::Start);
        check.set_can_focus(false);
        {
            let cell = cell.downgrade();
            check.connect_toggled(move |check| {
                let Some(cell) = cell.upgrade() else { return };
                let Some(context) = cell.context() else {
                    return;
                };
                if !cell.imp().bound.get() {
                    return;
                }
                let position = cell.position();
                let selected = context.selection.is_selected(position);
                if check.is_active() && !selected {
                    context.selection.select_item(position, false);
                } else if !check.is_active() && selected {
                    context.selection.unselect_item(position);
                }
            });
        }

        let name = gtk::Label::builder().xalign(0.0).build();
        name.add_css_class("item-name");

        match kind {
            CellKind::Grid(size) => {
                cell.set_orientation(gtk::Orientation::Vertical);
                cell.add_css_class("grid-cell");
                cell.add_css_class(match size {
                    256 => "extra-large",
                    96 => "large",
                    _ => "medium",
                });
                let width = match size {
                    256 => 272,
                    96 => 112,
                    _ => 84,
                };
                cell.set_size_request(width, -1);
                let overlay = gtk::Overlay::new();
                overlay.set_halign(gtk::Align::Center);
                let icon = gtk::Image::new();
                icon.set_size_request(size, size);
                overlay.set_child(Some(&icon));
                check.set_halign(gtk::Align::Start);
                overlay.add_overlay(&check);
                let link = Glyph::sized("link-overlay", (size / 4).clamp(10, 32));
                link.set_halign(gtk::Align::Start);
                link.set_valign(gtk::Align::End);
                link.set_visible(false);
                overlay.add_overlay(&link);
                cell.append(&overlay);
                name.set_xalign(0.5);
                name.set_justify(gtk::Justification::Center);
                name.set_wrap(true);
                name.set_wrap_mode(pango::WrapMode::WordChar);
                name.set_lines(2);
                name.set_ellipsize(pango::EllipsizeMode::End);
                name.set_max_width_chars(1);
                name.set_width_request(width - 8);
                cell.append(&name);
                imp.icon.replace(Some(icon));
                imp.link.replace(Some(link));
            }
            CellKind::Row | CellKind::DetailName => {
                cell.set_orientation(gtk::Orientation::Horizontal);
                cell.set_spacing(6);
                cell.add_css_class(if kind == CellKind::Row {
                    "row-cell"
                } else {
                    "name-cell"
                });
                if kind == CellKind::Row {
                    cell.set_size_request(232, -1);
                }
                cell.append(&check);
                let overlay = gtk::Overlay::new();
                let icon = gtk::Image::new();
                overlay.set_child(Some(&icon));
                let link = Glyph::sized("link-overlay", 8);
                link.set_halign(gtk::Align::Start);
                link.set_valign(gtk::Align::End);
                link.set_visible(false);
                overlay.add_overlay(&link);
                cell.append(&overlay);
                name.set_ellipsize(pango::EllipsizeMode::End);
                if kind == CellKind::Row {
                    name.set_max_width_chars(28);
                }
                name.set_hexpand(true);
                cell.append(&name);
                imp.icon.replace(Some(icon));
                imp.link.replace(Some(link));
            }
            CellKind::Tile | CellKind::Content => {
                cell.set_orientation(gtk::Orientation::Horizontal);
                cell.set_spacing(10);
                cell.add_css_class(if kind == CellKind::Tile {
                    "tile-cell"
                } else {
                    "content-cell"
                });
                if kind == CellKind::Tile {
                    cell.set_size_request(264, -1);
                }
                cell.append(&check);
                let icon = gtk::Image::new();
                let size = if kind == CellKind::Tile { 48 } else { 32 };
                icon.set_size_request(size, size);
                icon.set_valign(gtk::Align::Center);
                cell.append(&icon);
                let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
                text.set_valign(gtk::Align::Center);
                text.set_hexpand(true);
                name.set_ellipsize(pango::EllipsizeMode::Middle);
                if kind == CellKind::Tile {
                    name.set_max_width_chars(24);
                }
                text.append(&name);
                let mut lines = Vec::new();
                let count = if kind == CellKind::Tile { 2 } else { 1 };
                for _ in 0..count {
                    let line = gtk::Label::builder()
                        .xalign(0.0)
                        .ellipsize(pango::EllipsizeMode::End)
                        .build();
                    line.add_css_class("item-detail");
                    text.append(&line);
                    lines.push(line);
                }
                cell.append(&text);
                if kind == CellKind::Content {
                    let right = gtk::Box::new(gtk::Orientation::Vertical, 1);
                    right.set_valign(gtk::Align::Center);
                    right.set_size_request(260, -1);
                    for _ in 0..2 {
                        let line = gtk::Label::builder()
                            .xalign(0.0)
                            .ellipsize(pango::EllipsizeMode::End)
                            .build();
                        line.add_css_class("item-detail");
                        right.append(&line);
                        lines.push(line);
                    }
                    cell.append(&right);
                }
                imp.icon.replace(Some(icon));
                imp.lines.replace(lines);
            }
            CellKind::Detail(column) => {
                cell.add_css_class("detail-cell");
                if column == Column::Size {
                    name.set_xalign(1.0);
                }
                name.add_css_class("item-detail");
                name.set_ellipsize(pango::EllipsizeMode::End);
                name.set_hexpand(true);
                cell.append(&name);
            }
        }
        imp.name.replace(Some(name));
        imp.check.replace(Some(check));
        cell.add_drag_and_drop();
        cell
    }

    pub fn kind(&self) -> CellKind {
        self.imp().kind.get().expect("a cell has a kind")
    }

    fn context(&self) -> Option<Rc<CellContext>> {
        self.imp().context.borrow().clone()
    }

    pub fn info(&self) -> Option<gio::FileInfo> {
        self.imp().info.borrow().clone()
    }

    pub fn position(&self) -> u32 {
        self.imp()
            .list_item
            .borrow()
            .as_ref()
            .and_then(|item| item.upgrade())
            .map(|item| item.position())
            .unwrap_or(gtk::INVALID_LIST_POSITION)
    }

    pub fn set_list_item(&self, item: &gtk::ListItem) {
        self.imp().list_item.replace(Some(item.downgrade()));
    }

    pub fn is_bound(&self) -> bool {
        self.imp().bound.get()
    }

    /// Shows `info`, which is at `position` in the list.
    pub fn bind(&self, info: &gio::FileInfo, position: u32, selected: bool) {
        let imp = self.imp();
        // The same file refreshed (GIO reporting a change) keeps its editor.
        let same_file = imp
            .editing
            .borrow()
            .as_ref()
            .is_some_and(|(name, at)| *name == info.name().into_os_string() && *at == position);
        if !same_file {
            self.cancel_rename();
        }
        imp.info.replace(Some(info.clone()));
        imp.bound.set(true);
        imp.generation.set(imp.generation.get() + 1);
        let Some(context) = self.context() else {
            return;
        };
        let hide = context.hide_extensions.get();
        let kind = self.kind();

        if let Some(name) = imp.name.borrow().as_ref() {
            let text = match kind {
                CellKind::Detail(column) => column_text(info, column, hide),
                _ => shown_name(info, hide),
            };
            name.set_label(&text);
        }

        // Hidden items and items that were cut look faded, as on Windows.
        let faded = info.is_hidden() || info.is_backup() || crate::clipboard::is_cut(info);
        if faded {
            self.add_css_class("faded");
        } else {
            self.remove_css_class("faded");
        }

        if let Some(link) = imp.link.borrow().as_ref() {
            link.set_visible(info.is_symlink());
        }

        let hint = context
            .hints
            .get()
            .then(|| util::file_of(info))
            .flatten()
            .and_then(|file| crate::hints::for_file(&file));

        if let Some(check) = imp.check.borrow().as_ref() {
            check.set_visible(context.checkboxes.get());
            check.set_active(selected);
        }

        match kind {
            // The Purpose column explains itself when pointed at.
            CellKind::Detail(Column::Purpose) => {
                self.set_tooltip_text(hint.map(hint_text).as_deref());
            }
            CellKind::Detail(_) => {}
            _ => {
                if context.tooltips.get() && kind != CellKind::DetailName {
                    let mut tip = tooltip_text(info);
                    if let Some(hint) = hint {
                        tip = format!("{}\n{tip}", hint_text(hint));
                    }
                    self.set_tooltip_text(Some(&tip));
                } else {
                    self.set_tooltip_text(None);
                }
            }
        }
        // Tiles and Content say it after the type: "File folder · Settings".
        let type_line = match hint {
            Some(hint) => format!("{} \u{b7} {}", type_text(info), hint.tag),
            None => type_text(info),
        };

        let lines = imp.lines.borrow();
        match kind {
            CellKind::Tile => {
                lines[0].set_label(&type_line);
                lines[1].set_label(&if util::is_dir(info) {
                    String::new()
                } else {
                    util::size(info.size().max(0) as u64)
                });
            }
            CellKind::Content => {
                let in_search = info.has_attribute("pfm::search-result");
                lines[0].set_label(&if in_search {
                    column_text(info, Column::Folder, hide)
                } else {
                    type_line
                });
                lines[1].set_label(
                    &util::modified(info)
                        .map(|time| format!("Date modified: {}", util::date_time(&time)))
                        .unwrap_or_default(),
                );
                lines[2].set_label(&if util::is_dir(info) {
                    String::new()
                } else {
                    format!("Size: {}", util::size(info.size().max(0) as u64))
                });
            }
            _ => {}
        }
        drop(lines);
        self.update_icon();
    }

    /// Sets the icon, then a thumbnail when one arrives.
    pub fn update_icon(&self) {
        let imp = self.imp();
        let Some(icon) = imp.icon.borrow().clone() else {
            return;
        };
        let Some(info) = self.info() else { return };
        let size = match self.kind() {
            CellKind::Grid(size) => size,
            CellKind::Tile => 48,
            CellKind::Content => 32,
            // The artwork has margins: at 20 the item itself is about 16.
            _ => 20,
        };
        icons::set_image(&icon, icons::for_info(&info), size);
        icon.remove_css_class("thumbnail");
        if size < 32 || !thumbs::enabled() || util::is_dir(&info) {
            return;
        }
        let generation = imp.generation.get();
        let weak = self.downgrade();
        let scale = self.scale_factor().max(1);
        thumbs::request(&info, size * scale, move |texture| {
            let Some(cell) = weak.upgrade() else { return };
            if cell.imp().generation.get() != generation {
                return;
            }
            if let (Some(texture), Some(icon)) = (texture, cell.imp().icon.borrow().as_ref()) {
                icon.set_paintable(Some(&texture));
                icon.set_pixel_size(size);
                icon.add_css_class("thumbnail");
            }
        });
    }

    pub fn unbind(&self) {
        let imp = self.imp();
        imp.bound.set(false);
        imp.info.replace(None);
        imp.generation.set(imp.generation.get() + 1);
    }

    pub fn set_selected(&self, selected: bool) {
        if let Some(check) = self.imp().check.borrow().as_ref()
            && check.is_active() != selected
        {
            check.set_active(selected);
        }
    }

    /// Renames in place: an editor replaces the name, with the name selected
    /// up to its extension. `done` gets the new name, or `None` if cancelled.
    pub fn begin_rename(&self, hide_extension: bool, done: impl Fn(Option<String>) + 'static) {
        let imp = self.imp();
        let (Some(info), Some(name_label)) = (self.info(), imp.name.borrow().clone()) else {
            return;
        };
        self.cancel_rename();
        let full_name = info.edit_name().to_string();
        let (stem, extension) = util::split_extension(&full_name);
        let is_dir = util::is_dir(&info);
        let hiding = hide_extension && !is_dir && shown_name(&info, true) != full_name;
        let text = if hiding {
            stem.to_owned()
        } else {
            full_name.clone()
        };
        let entry = gtk::Entry::builder().text(&text).build();
        entry.add_css_class("rename-entry");
        if let CellKind::Grid(_) = self.kind() {
            gtk::prelude::EditableExt::set_alignment(&entry, 0.5);
            entry.set_width_chars(8);
        }
        name_label.set_visible(false);
        self.insert_child_after(&entry, Some(&name_label));
        imp.editor.replace(Some(entry.clone()));
        imp.editing
            .replace(Some((info.name().into_os_string(), self.position())));
        let select_to = if is_dir || hiding || extension.is_empty() {
            text.chars().count() as i32
        } else {
            stem.chars().count() as i32
        };
        entry.grab_focus();
        entry.select_region(0, select_to);

        let done = Rc::new(done);
        let finished = Rc::new(Cell::new(false));
        let finish = {
            let cell = self.downgrade();
            let finished = finished.clone();
            let done = done.clone();
            let extension = extension.to_owned();
            move |commit: bool, text: String| {
                if finished.replace(true) {
                    return;
                }
                if let Some(cell) = cell.upgrade() {
                    cell.cancel_rename();
                }
                if commit {
                    let name = if hiding {
                        format!("{text}{extension}")
                    } else {
                        text
                    };
                    done(Some(name));
                } else {
                    done(None);
                }
            }
        };
        let finish = Rc::new(finish);
        {
            let finish = finish.clone();
            entry.connect_activate(move |entry| finish(true, entry.text().to_string()));
        }
        let keys = gtk::EventControllerKey::new();
        {
            let finish = finish.clone();
            let entry = entry.downgrade();
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    if let Some(entry) = entry.upgrade() {
                        finish(false, entry.text().to_string());
                    }
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
        }
        entry.add_controller(keys);
        let focus = gtk::EventControllerFocus::new();
        {
            let finish = finish.clone();
            let entry_weak = entry.downgrade();
            focus.connect_leave(move |_| {
                let finish = finish.clone();
                let entry_weak = entry_weak.clone();
                // Let a click elsewhere land first, as Windows commits then.
                glib::idle_add_local_once(move || {
                    if let Some(entry) = entry_weak.upgrade() {
                        finish(true, entry.text().to_string());
                    }
                });
            });
        }
        entry.add_controller(focus);

        // As on Windows, a click anywhere else in the window (another item,
        // empty space, the navigation pane) keeps the name as typed. Those
        // clicks don't always take the keyboard focus from the editor, so
        // the window watches for them; switching to another window does
        // the same.
        if let Some(window) = self.root().and_downcast::<gtk::Window>() {
            let click = gtk::GestureClick::new();
            click.set_button(0);
            click.set_propagation_phase(gtk::PropagationPhase::Capture);
            {
                let finish = finish.clone();
                let entry_weak = entry.downgrade();
                click.connect_pressed(move |gesture, _, x, y| {
                    let (Some(entry), Some(window)) = (entry_weak.upgrade(), gesture.widget())
                    else {
                        return;
                    };
                    let point = gtk::graphene::Point::new(x as f32, y as f32);
                    let inside = entry
                        .compute_bounds(&window)
                        .is_some_and(|bounds| bounds.contains_point(&point));
                    if !inside {
                        // After the click has done its own work.
                        let text = entry.text().to_string();
                        let finish = finish.clone();
                        glib::idle_add_local_once(move || finish(true, text));
                    }
                });
            }
            window.add_controller(click.clone());
            let active = {
                let finish = finish.clone();
                let entry_weak = entry.downgrade();
                window.connect_is_active_notify(move |window| {
                    if !window.is_active()
                        && let Some(entry) = entry_weak.upgrade()
                    {
                        finish(true, entry.text().to_string());
                    }
                })
            };
            imp.rename_hooks.replace(Some(RenameHooks {
                window: window.downgrade(),
                click,
                active,
            }));
        }
    }

    pub fn is_renaming(&self) -> bool {
        self.imp().editor.borrow().is_some()
    }

    fn cancel_rename(&self) {
        let imp = self.imp();
        imp.editing.take();
        if let Some(hooks) = imp.rename_hooks.take()
            && let Some(window) = hooks.window.upgrade()
        {
            window.remove_controller(&hooks.click);
            window.disconnect(hooks.active);
        }
        if let Some(entry) = imp.editor.take() {
            entry.unparent();
            if let Some(name) = imp.name.borrow().as_ref() {
                name.set_visible(true);
            }
        }
    }

    fn add_drag_and_drop(&self) {
        if matches!(self.kind(), CellKind::Detail(_)) {
            return;
        }
        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::COPY | gdk::DragAction::MOVE | gdk::DragAction::LINK);
        {
            let cell = self.downgrade();
            source.connect_prepare(move |_, _, _| {
                let cell = cell.upgrade()?;
                let context = cell.context()?;
                if !cell.is_bound() || cell.is_renaming() {
                    return None;
                }
                let position = cell.position();
                if !context.selection.is_selected(position) {
                    context.selection.select_item(position, true);
                }
                let files = context.selected_files();
                if files.is_empty() {
                    return None;
                }
                Some(gdk::ContentProvider::for_value(
                    &gdk::FileList::from_array(&files).to_value(),
                ))
            });
        }
        {
            let cell = self.downgrade();
            source.connect_drag_begin(move |source, _| {
                let Some(cell) = cell.upgrade() else { return };
                let Some(info) = cell.info() else { return };
                let paintable = icons::paintable(icons::for_info(&info), 48, cell.scale_factor());
                source.set_icon(Some(&paintable), 24, 24);
            });
        }
        self.add_controller(source);

        let target = gtk::DropTarget::new(
            gdk::FileList::static_type(),
            gdk::DragAction::COPY | gdk::DragAction::MOVE,
        );
        target.set_preload(true);
        {
            let cell = self.downgrade();
            target.connect_accept(move |_, _| {
                cell.upgrade()
                    .and_then(|cell| cell.info())
                    .is_some_and(|info| util::is_dir(&info))
            });
        }
        {
            let cell = self.downgrade();
            target.connect_motion(move |target, _, _| {
                let Some(folder) = cell
                    .upgrade()
                    .and_then(|cell| cell.info())
                    .and_then(|info| util::file_of(&info))
                else {
                    return gdk::DragAction::empty();
                };
                crate::dnd::preferred_action(target, &folder)
            });
        }
        {
            let cell = self.downgrade();
            target.connect_drop(move |target, value, _, _| {
                let Some(cell) = cell.upgrade() else {
                    return false;
                };
                let (Some(context), Some(info)) = (cell.context(), cell.info()) else {
                    return false;
                };
                let Some(folder) = util::file_of(&info) else {
                    return false;
                };
                let Ok(list) = value.get::<gdk::FileList>() else {
                    return false;
                };
                let files = list.files();
                if files.iter().any(|file| file.equal(&folder)) {
                    return false;
                }
                let action = crate::dnd::preferred_action(target, &folder);
                let on_drop = context.on_drop.borrow().clone();
                if let Some(on_drop) = on_drop {
                    on_drop(files, folder, action);
                }
                true
            });
        }
        self.add_controller(target);
    }
}
