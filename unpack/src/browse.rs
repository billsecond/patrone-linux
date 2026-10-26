//! The archive window: what's inside, shown as folders as Windows shows a
//! zip, with "Extract all…" (pick a folder, as Windows' wizard does) and
//! "Extract here" (next to the archive). Double-clicking a file opens a
//! copy of it.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::archive::{self, Existing, Plan, Progress, Tree};
use crate::util;

/// What to do as soon as the archive is read (from the command line).
#[derive(Debug, Clone)]
pub enum Action {
    /// Extract next to the archive, then close.
    ExtractHere,
    /// Extract into this folder, then close.
    ExtractTo(PathBuf),
}

struct Browser {
    window: gtk::ApplicationWindow,
    archive: PathBuf,
    stack: gtk::Stack,
    error_text: gtk::Label,
    store: gio::ListStore,
    view: gtk::ColumnView,
    up: gtk::Button,
    place: gtk::Label,
    extract_all: gtk::Button,
    extract_here: gtk::Button,
    items: gtk::Label,
    summary: gtk::Label,
    working: gtk::Box,
    progress: gtk::ProgressBar,
    finished: gtk::Box,
    finished_text: gtk::Label,
    tree: RefCell<Option<Rc<Tree>>>,
    raw: Cell<bool>,
    format: RefCell<String>,
    folder: Cell<usize>,
    passphrase: RefCell<Option<String>>,
    running: RefCell<Option<Arc<Progress>>>,
    extracted_to: RefCell<Option<PathBuf>>,
}

pub fn open(app: &gtk::Application, path: &Path, action: Option<Action>) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(file_name(path))
        .default_width(880)
        .default_height(580)
        .build();

    // The command bar
    let extract_all = command_button("folder-download-symbolic", "Extract all…");
    extract_all.add_css_class("suggested-action");
    extract_all.set_tooltip_text(Some("Choose a folder and extract everything into it"));
    let extract_here = command_button("document-save-symbolic", "Extract here");
    extract_here.set_tooltip_text(Some("Extract next to the archive"));
    let up = gtk::Button::builder()
        .icon_name("go-up-symbolic")
        .tooltip_text("Up one folder (Backspace)")
        .sensitive(false)
        .css_classes(["flat"])
        .build();
    let place = gtk::Label::builder()
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Start)
        .css_classes(["unpack-place"])
        .build();
    let bar = gtk::Box::builder()
        .spacing(8)
        .css_classes(["unpack-commandbar"])
        .build();
    bar.append(&extract_all);
    bar.append(&extract_here);
    bar.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    bar.append(&up);
    bar.append(&place);
    extract_all.set_sensitive(false);
    extract_here.set_sensitive(false);

    // The list
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let selection = gtk::SingleSelection::builder()
        .model(&store)
        .autoselect(false)
        .can_unselect(true)
        .build();
    let view = gtk::ColumnView::builder()
        .model(&selection)
        .show_row_separators(false)
        .css_classes(["unpack-list"])
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .child(&view)
        .vexpand(true)
        .build();

    let loading = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Center)
        .build();
    loading.append(
        &gtk::Spinner::builder()
            .spinning(true)
            .width_request(32)
            .height_request(32)
            .build(),
    );
    loading.append(&gtk::Label::new(Some(&format!(
        "Reading “{}”…",
        file_name(path)
    ))));

    let error_text = gtk::Label::builder()
        .wrap(true)
        .justify(gtk::Justification::Center)
        .selectable(true)
        .max_width_chars(60)
        .build();
    let error = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .valign(gtk::Align::Center)
        .halign(gtk::Align::Center)
        .build();
    error.append(
        &gtk::Image::builder()
            .icon_name("dialog-warning-symbolic")
            .pixel_size(48)
            .build(),
    );
    error.append(
        &gtk::Label::builder()
            .label("This archive can't be opened")
            .css_classes(["unpack-title"])
            .build(),
    );
    error.append(&error_text);

    let stack = gtk::Stack::new();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&scroller, Some("list"));
    stack.add_named(&error, Some("error"));

    // The status bar: counts, or progress, or where things went
    let items = gtk::Label::builder().xalign(0.0).build();
    let summary = gtk::Label::builder()
        .xalign(1.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Start)
        .css_classes(["unpack-dim"])
        .build();
    let progress = gtk::ProgressBar::builder()
        .show_text(true)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let working = gtk::Box::builder()
        .spacing(12)
        .hexpand(true)
        .visible(false)
        .build();
    working.append(&progress);
    working.append(&cancel);
    let finished_text = gtk::Label::builder()
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();
    let open_folder = gtk::Button::with_label("Open folder");
    let finished = gtk::Box::builder()
        .spacing(12)
        .hexpand(true)
        .visible(false)
        .build();
    finished.append(&gtk::Image::from_icon_name("emblem-ok-symbolic"));
    finished.append(&finished_text);
    finished.append(&open_folder);
    let status = gtk::Box::builder()
        .spacing(12)
        .css_classes(["unpack-statusbar"])
        .build();
    status.append(&items);
    status.append(&summary);
    status.append(&working);
    status.append(&finished);

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout.append(&bar);
    layout.append(&stack);
    layout.append(&status);
    window.set_child(Some(&layout));

    let this = Rc::new(Browser {
        window: window.clone(),
        archive: path.to_path_buf(),
        stack,
        error_text,
        store,
        view: view.clone(),
        up: up.clone(),
        place,
        extract_all: extract_all.clone(),
        extract_here: extract_here.clone(),
        items,
        summary,
        working,
        progress,
        finished,
        finished_text,
        tree: RefCell::new(None),
        raw: Cell::new(false),
        format: RefCell::new(String::new()),
        folder: Cell::new(0),
        passphrase: RefCell::new(None),
        running: RefCell::new(None),
        extracted_to: RefCell::new(None),
    });
    add_columns(&this);

    let weak = Rc::downgrade(&this);
    view.connect_activate(move |_, position| {
        if let Some(this) = weak.upgrade() {
            activate(&this, position);
        }
    });
    let weak = Rc::downgrade(&this);
    up.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            go_up(&this);
        }
    });
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(&this);
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let Some(this) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let up = key == gdk::Key::BackSpace
            || (key == gdk::Key::Up && modifiers.contains(gdk::ModifierType::ALT_MASK));
        if up && this.folder.get() != 0 {
            go_up(&this);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    let weak = Rc::downgrade(&this);
    extract_all.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            ask_destination(&this);
        }
    });
    let weak = Rc::downgrade(&this);
    extract_here.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            extract_here_now(&this, false);
        }
    });
    let weak = Rc::downgrade(&this);
    cancel.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade()
            && let Some(progress) = &*this.running.borrow()
        {
            progress.cancel.store(true, Ordering::Relaxed);
        }
    });
    let weak = Rc::downgrade(&this);
    open_folder.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade()
            && let Some(folder) = &*this.extracted_to.borrow()
        {
            show_folder_in_files(&this.window, folder);
        }
    });
    // Closing stops an extraction that's running.
    let keep = RefCell::new(Some(this.clone()));
    window.connect_close_request(move |_| {
        if let Some(this) = keep.borrow_mut().take()
            && let Some(progress) = &*this.running.borrow()
        {
            progress.cancel.store(true, Ordering::Relaxed);
        }
        glib::Propagation::Proceed
    });

    window.present();
    load(this, action);
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn command_button(icon: &str, text: &str) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(&gtk::Image::from_icon_name(icon));
    content.append(&gtk::Label::new(Some(text)));
    gtk::Button::builder().child(&content).build()
}

/// The node a list row shows.
fn node_at(item: &glib::Object) -> Option<usize> {
    let list_item = item.downcast_ref::<gtk::ListItem>()?;
    let boxed = list_item.item().and_downcast::<glib::BoxedAnyObject>()?;
    Some(*boxed.borrow::<usize>())
}

fn text_column(
    this: &Rc<Browser>,
    title: &str,
    width: i32,
    right: bool,
    value: fn(&archive::Node) -> String,
) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let label = gtk::Label::builder()
            .xalign(if right { 1.0 } else { 0.0 })
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["unpack-dim"])
            .build();
        if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
            item.set_child(Some(&label));
        }
    });
    let weak = Rc::downgrade(this);
    factory.connect_bind(move |_, item| {
        let (Some(this), Some(index)) = (weak.upgrade(), node_at(item)) else {
            return;
        };
        let Some(label) = item
            .downcast_ref::<gtk::ListItem>()
            .and_then(|item| item.child())
            .and_downcast::<gtk::Label>()
        else {
            return;
        };
        if let Some(tree) = &*this.tree.borrow() {
            label.set_text(&value(&tree.nodes[index]));
        }
    });
    let column = gtk::ColumnViewColumn::new(Some(title), Some(factory));
    column.set_fixed_width(width);
    column.set_resizable(true);
    column
}

fn content_type(name: &str) -> glib::GString {
    gio::content_type_guess(Some(name), None).0
}

fn add_columns(this: &Rc<Browser>) {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&gtk::Image::new());
        row.append(
            &gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build(),
        );
        row.append(
            &gtk::Image::builder()
                .icon_name("changes-prevent-symbolic")
                .tooltip_text("Protected by a password")
                .css_classes(["unpack-dim"])
                .build(),
        );
        if let Some(item) = item.downcast_ref::<gtk::ListItem>() {
            item.set_child(Some(&row));
        }
    });
    let weak = Rc::downgrade(this);
    factory.connect_bind(move |_, item| {
        let (Some(this), Some(index)) = (weak.upgrade(), node_at(item)) else {
            return;
        };
        let Some(row) = item
            .downcast_ref::<gtk::ListItem>()
            .and_then(|item| item.child())
            .and_downcast::<gtk::Box>()
        else {
            return;
        };
        let tree = this.tree.borrow();
        let Some(tree) = tree.as_ref() else {
            return;
        };
        let node = &tree.nodes[index];
        let icon = row.first_child().and_downcast::<gtk::Image>();
        let label = icon
            .as_ref()
            .and_then(|icon| icon.next_sibling())
            .and_downcast::<gtk::Label>();
        let lock = label.as_ref().and_then(|label| label.next_sibling());
        if let Some(icon) = icon {
            if node.folder {
                icon.set_icon_name(Some("folder"));
            } else {
                icon.set_from_gicon(&gio::content_type_get_icon(&content_type(&node.name)));
            }
        }
        if let Some(label) = label {
            label.set_text(&node.name);
        }
        if let Some(lock) = lock {
            lock.set_visible(node.encrypted);
        }
    });
    let name = gtk::ColumnViewColumn::new(Some("Name"), Some(factory));
    name.set_expand(true);
    name.set_resizable(true);
    this.view.append_column(&name);

    this.view
        .append_column(&text_column(this, "Type", 190, false, |node| {
            if node.folder {
                "File folder".into()
            } else if node.link {
                "Link".into()
            } else {
                gio::content_type_get_description(&content_type(&node.name)).to_string()
            }
        }));
    this.view
        .append_column(&text_column(this, "Size", 100, true, |node| {
            match (node.folder, node.size) {
                (false, Some(size)) => util::size(size),
                _ => String::new(),
            }
        }));
    this.view
        .append_column(&text_column(this, "Date modified", 170, false, |node| {
            node.modified.map(util::date_time).unwrap_or_default()
        }));
}

/// Reads the archive off the main thread, then shows its top folder.
fn load(this: Rc<Browser>, action: Option<Action>) {
    let path = this.archive.clone();
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || archive::list(&path, None))
            .await
            .unwrap_or_else(|_| Err(archive::Error::Failed("Reading the archive failed.".into())));
        let listing = match result {
            Ok(listing) => listing,
            Err(error) => {
                this.error_text.set_text(&error.to_string());
                this.stack.set_visible_child_name("error");
                this.items.set_text("");
                return;
            }
        };
        let tree = Tree::build(&listing.entries);
        let (files, folders) = tree.counts(0);
        let mut summary = vec![listing.format.clone()];
        summary.push(match (files, folders) {
            (1, 0) => "1 file".to_string(),
            (files, 0) => format!("{files} files"),
            (files, folders) => format!(
                "{files} {}, {folders} {}",
                if files == 1 { "file" } else { "files" },
                if folders == 1 { "folder" } else { "folders" }
            ),
        });
        let size = tree.total_size();
        if size > 0 {
            summary.push(format!("{} when extracted", util::size(size)));
        }
        if tree.encrypted() {
            summary.push("Protected by a password".into());
        }
        if tree.unsafe_entries > 0 {
            summary.push(format!(
                "{} left out: {} outside the archive's folder",
                tree.unsafe_entries,
                if tree.unsafe_entries == 1 {
                    "its path leads"
                } else {
                    "their paths lead"
                }
            ));
        }
        this.summary.set_text(&summary.join("  ·  "));
        this.raw.set(listing.raw);
        *this.format.borrow_mut() = listing.format;
        *this.tree.borrow_mut() = Some(Rc::new(tree));
        this.stack.set_visible_child_name("list");
        this.extract_all.set_sensitive(true);
        this.extract_here.set_sensitive(true);
        show(&this, 0);
        this.view.grab_focus();
        match action {
            Some(Action::ExtractHere) => extract_here_now(&this, true),
            Some(Action::ExtractTo(folder)) => {
                let tree = this.tree.borrow().clone().expect("just set");
                let mut plan = Plan::new(&this.archive, &folder);
                plan.strip = archive::strip_for(&tree, &folder);
                if this.raw.get() {
                    plan.raw_name = Some(archive::raw_name(&this.archive));
                }
                confirm_and_extract(this.clone(), plan, false, true);
            }
            None => {}
        }
    });
}

/// Shows a folder of the archive (0 is its top).
fn show(this: &Browser, folder: usize) {
    let Some(tree) = this.tree.borrow().clone() else {
        return;
    };
    this.folder.set(folder);
    let children: Vec<glib::BoxedAnyObject> = tree.nodes[folder]
        .children
        .iter()
        .map(|&child| glib::BoxedAnyObject::new(child))
        .collect();
    this.store.splice(0, this.store.n_items(), &children);
    let count = children.len();
    this.items.set_text(&match count {
        1 => "1 item".to_string(),
        count => format!("{count} items"),
    });
    this.up.set_sensitive(folder != 0);
    let mut place = file_name(&this.archive);
    if folder != 0 {
        for name in tree.nodes[folder].path.split('/') {
            place.push_str("  ›  ");
            place.push_str(name);
        }
    }
    this.place.set_text(&place);
}

fn go_up(this: &Browser) {
    let Some(tree) = this.tree.borrow().clone() else {
        return;
    };
    let from = this.folder.get();
    if from == 0 {
        return;
    }
    show(this, tree.nodes[from].parent);
    // Keep the folder we came from selected, as Explorer does.
    let position = tree.nodes[tree.nodes[from].parent]
        .children
        .iter()
        .position(|&child| child == from);
    if let Some(position) = position {
        this.view.scroll_to(
            position as u32,
            None,
            gtk::ListScrollFlags::SELECT | gtk::ListScrollFlags::FOCUS,
            None,
        );
    }
}

fn activate(this: &Rc<Browser>, position: u32) {
    let Some(index) = this
        .store
        .item(position)
        .and_downcast::<glib::BoxedAnyObject>()
        .map(|boxed| *boxed.borrow::<usize>())
    else {
        return;
    };
    let Some(tree) = this.tree.borrow().clone() else {
        return;
    };
    if tree.nodes[index].folder {
        show(this, index);
    } else {
        open_copy(this, index);
    }
}

/// Opens one file from the archive: extracts it to a temporary folder and
/// opens that copy in its usual app, as Windows does from a zip.
fn open_copy(this: &Rc<Browser>, index: usize) {
    let Some(tree) = this.tree.borrow().clone() else {
        return;
    };
    let node = &tree.nodes[index];
    let Some(folder) = util::temporary_folder() else {
        return;
    };
    let mut plan = Plan::new(&this.archive, &folder);
    if this.raw.get() {
        plan.raw_name = Some(node.name.clone());
    } else {
        plan.only = Some(node.path.clone());
    }
    let file = folder.join(&node.path);
    run(this.clone(), plan, Finish::Open(file));
}

/// Windows' "Extract Compressed Folders": the destination, Browse…, and
/// "Show extracted files when complete".
fn ask_destination(this: &Rc<Browser>) {
    let dialog = gtk::Window::builder()
        .title("Extract all")
        .transient_for(&this.window)
        .modal(true)
        .default_width(560)
        .resizable(false)
        .build();
    let entry = gtk::Entry::builder()
        .text(util::display_path(&archive::suggested_folder(
            &this.archive,
        )))
        .hexpand(true)
        .activates_default(true)
        .build();
    let browse = gtk::Button::with_label("Browse…");
    let show_files = gtk::CheckButton::builder()
        .label("Show extracted files when complete")
        .active(true)
        .build();
    let extract = gtk::Button::builder()
        .label("Extract")
        .css_classes(["suggested-action"])
        .build();
    let cancel = gtk::Button::with_label("Cancel");

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&entry);
    row.append(&browse);
    let buttons = gtk::Box::builder()
        .spacing(8)
        .halign(gtk::Align::End)
        .margin_top(12)
        .build();
    buttons.append(&cancel);
    buttons.append(&extract);
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(24)
        .margin_end(24)
        .build();
    content.append(
        &gtk::Label::builder()
            .label("Select a destination and extract files")
            .xalign(0.0)
            .css_classes(["unpack-heading"])
            .build(),
    );
    content.append(
        &gtk::Label::builder()
            .label("Files will be extracted to this folder:")
            .xalign(0.0)
            .build(),
    );
    content.append(&row);
    content.append(&show_files);
    content.append(&buttons);
    dialog.set_child(Some(&content));
    dialog.set_default_widget(Some(&extract));

    let dialog_ref = dialog.clone();
    cancel.connect_clicked(move |_| dialog_ref.close());
    let dialog_ref = dialog.clone();
    let entry_ref = entry.clone();
    browse.connect_clicked(move |_| {
        let current = util::typed_path(&entry_ref.text());
        let start = if current.is_dir() {
            current
        } else {
            current
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(glib::home_dir)
        };
        let chooser = gtk::FileDialog::builder()
            .title("Select a destination")
            .accept_label("Select Folder")
            .initial_folder(&gio::File::for_path(&start))
            .modal(true)
            .build();
        let entry = entry_ref.clone();
        chooser.select_folder(
            Some(&dialog_ref),
            None::<&gio::Cancellable>,
            move |result| {
                if let Ok(folder) = result
                    && let Some(path) = folder.path()
                {
                    entry.set_text(&util::display_path(&path));
                }
            },
        );
    });
    let weak = Rc::downgrade(this);
    let dialog_ref = dialog.clone();
    extract.connect_clicked(move |_| {
        let Some(this) = weak.upgrade() else {
            return;
        };
        let folder = util::typed_path(&entry.text());
        if folder.as_os_str().is_empty() {
            return;
        }
        let folder = if folder.is_relative() {
            this.archive.parent().unwrap_or(Path::new("/")).join(folder)
        } else {
            folder
        };
        dialog_ref.close();
        let Some(tree) = this.tree.borrow().clone() else {
            return;
        };
        let mut plan = Plan::new(&this.archive, &folder);
        plan.strip = archive::strip_for(&tree, &folder);
        if this.raw.get() {
            plan.raw_name = Some(archive::raw_name(&this.archive));
        }
        confirm_and_extract(this, plan, show_files.is_active(), false);
    });
    dialog.present();
}

/// Asks what to do about files already there, then extracts.
fn confirm_and_extract(this: Rc<Browser>, mut plan: Plan, show_files: bool, close: bool) {
    let Some(tree) = this.tree.borrow().clone() else {
        return;
    };
    let conflicts = archive::conflicts(&tree, &plan);
    let finish = Finish::Done { show_files, close };
    if conflicts == 0 {
        run(this, plan, finish);
        return;
    }
    let dialog = gtk::AlertDialog::builder()
        .modal(true)
        .message(if conflicts == 1 {
            "1 item is already there".to_string()
        } else {
            format!("{conflicts} items are already there")
        })
        .detail(format!(
            "The destination, {}, already has files with the same names.",
            util::display_path(&plan.destination)
        ))
        .buttons(["Replace the files", "Skip these files", "Cancel"])
        .default_button(0)
        .cancel_button(2)
        .build();
    let window = this.window.clone();
    glib::spawn_future_local(async move {
        match dialog.choose_future(Some(&window)).await {
            Ok(0) => plan.existing = Existing::Replace,
            Ok(1) => plan.existing = Existing::Skip,
            _ => return,
        }
        run(this, plan, finish);
    });
}

/// "Extract here": next to the archive, under a name not yet taken.
fn extract_here_now(this: &Rc<Browser>, close: bool) {
    let Some(tree) = this.tree.borrow().clone() else {
        return;
    };
    let plan = archive::plan_here(&this.archive, &tree, this.raw.get());
    run(
        this.clone(),
        plan,
        Finish::Done {
            show_files: false,
            close,
        },
    );
}

/// What to do once an extraction has worked.
#[derive(Clone)]
enum Finish {
    Done {
        show_files: bool,
        close: bool,
    },
    /// Open this extracted file.
    Open(PathBuf),
}

/// Asks for the archive's password.
fn ask_password(this: &Rc<Browser>, wrong: bool, then: impl Fn(Rc<Browser>) + 'static) {
    let dialog = gtk::Window::builder()
        .title("Password")
        .transient_for(&this.window)
        .modal(true)
        .default_width(420)
        .resizable(false)
        .build();
    let entry = gtk::PasswordEntry::builder()
        .show_peek_icon(true)
        .activates_default(true)
        .build();
    let ok = gtk::Button::builder()
        .label("OK")
        .css_classes(["suggested-action"])
        .build();
    let cancel = gtk::Button::with_label("Cancel");
    let buttons = gtk::Box::builder()
        .spacing(8)
        .halign(gtk::Align::End)
        .margin_top(8)
        .build();
    buttons.append(&cancel);
    buttons.append(&ok);
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(24)
        .margin_end(24)
        .build();
    content.append(
        &gtk::Label::builder()
            .label(if wrong {
                "That password isn't right. Try again:".to_string()
            } else {
                format!("“{}” is protected by a password.", file_name(&this.archive))
            })
            .xalign(0.0)
            .wrap(true)
            .css_classes(if wrong {
                vec!["unpack-error"]
            } else {
                vec!["unpack-heading"]
            })
            .build(),
    );
    content.append(&entry);
    content.append(&buttons);
    dialog.set_child(Some(&content));
    dialog.set_default_widget(Some(&ok));

    let dialog_ref = dialog.clone();
    cancel.connect_clicked(move |_| dialog_ref.close());
    let weak = Rc::downgrade(this);
    let dialog_ref = dialog.clone();
    ok.connect_clicked(move |_| {
        let Some(this) = weak.upgrade() else {
            return;
        };
        *this.passphrase.borrow_mut() = Some(entry.text().to_string());
        dialog_ref.close();
        then(this);
    });
    dialog.present();
}

/// Extracts on a worker thread, with progress in the status bar.
fn run(this: Rc<Browser>, mut plan: Plan, finish: Finish) {
    let Some(tree) = this.tree.borrow().clone() else {
        return;
    };
    if this.running.borrow().is_some() {
        return;
    }
    if tree.encrypted() && this.passphrase.borrow().is_none() {
        ask_password(&this, false, move |this| {
            run(this, plan.clone(), finish.clone())
        });
        return;
    }
    plan.passphrase = this.passphrase.borrow().clone();

    let total = match &plan.only {
        Some(only) => tree
            .nodes
            .iter()
            .filter(|node| node.path == *only || node.path.starts_with(&format!("{only}/")))
            .filter_map(|node| node.size)
            .sum(),
        None => tree.total_size(),
    };
    let progress = Arc::new(Progress::default());
    *this.running.borrow_mut() = Some(progress.clone());
    this.extract_all.set_sensitive(false);
    this.extract_here.set_sensitive(false);
    this.items.set_visible(false);
    this.summary.set_visible(false);
    this.finished.set_visible(false);
    this.working.set_visible(true);
    this.progress.set_fraction(0.0);

    let ticker = {
        let progress = progress.clone();
        let bar = this.progress.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let current = progress
                .current
                .lock()
                .map(|name| name.clone())
                .unwrap_or_default();
            if total > 0 {
                let done = progress.bytes.load(Ordering::Relaxed) as f64 / total as f64;
                bar.set_fraction(done.min(1.0));
            } else {
                bar.pulse();
            }
            bar.set_text(Some(&format!("Extracting {current}")));
            glib::ControlFlow::Continue
        })
    };

    let worker_plan = plan.clone();
    let worker_progress = progress.clone();
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || archive::extract(&worker_plan, &worker_progress))
            .await
            .unwrap_or_else(|_| Err(archive::Error::Failed("Extracting failed.".into())));
        ticker.remove();
        this.running.borrow_mut().take();
        this.working.set_visible(false);
        this.extract_all.set_sensitive(true);
        this.extract_here.set_sensitive(true);
        match result {
            Ok(outcome) => {
                this.items.set_visible(true);
                this.summary.set_visible(true);
                match finish {
                    Finish::Open(file) => open_file(&this.window, &file),
                    Finish::Done { show_files, close } => {
                        done(&this, &plan, &outcome, show_files, close);
                    }
                }
            }
            Err(error @ (archive::Error::NeedsPassword | archive::Error::WrongPassword)) => {
                this.items.set_visible(true);
                this.summary.set_visible(true);
                let wrong = this.passphrase.borrow_mut().take().is_some()
                    || error == archive::Error::WrongPassword;
                ask_password(&this, wrong, move |this| {
                    run(this, plan.clone(), finish.clone())
                });
            }
            Err(archive::Error::Cancelled) => {
                this.items.set_visible(true);
                this.summary.set_visible(true);
                if let Finish::Done { .. } = finish {
                    this.finished_text
                        .set_text("Cancelled. Files extracted before then were kept.");
                    *this.extracted_to.borrow_mut() = Some(plan.destination.clone());
                    this.finished.set_visible(true);
                    this.items.set_visible(false);
                    this.summary.set_visible(false);
                }
            }
            Err(error) => {
                this.items.set_visible(true);
                this.summary.set_visible(true);
                gtk::AlertDialog::builder()
                    .modal(true)
                    .message("The archive couldn't be extracted")
                    .detail(error.to_string())
                    .build()
                    .show(Some(&this.window));
            }
        }
    });
}

fn done(
    this: &Rc<Browser>,
    plan: &Plan,
    outcome: &archive::Outcome,
    show_files: bool,
    close: bool,
) {
    let folder = plan.destination.clone();
    let mut text = format!("Extracted to {}", util::display_path(&folder));
    if outcome.skipped > 0 {
        text.push_str(&format!(", {} left as they were", outcome.skipped));
    }
    this.finished_text.set_text(&text);
    this.finished_text
        .set_tooltip_text(Some(&folder.to_string_lossy()));
    *this.extracted_to.borrow_mut() = Some(folder.clone());
    this.items.set_visible(false);
    this.summary.set_visible(false);
    this.finished.set_visible(true);
    if !outcome.problems.is_empty() {
        let shown: Vec<&str> = outcome
            .problems
            .iter()
            .take(12)
            .map(String::as_str)
            .collect();
        let mut detail = shown.join("\n");
        if outcome.problems.len() > shown.len() {
            detail.push_str(&format!(
                "\n…and {} more",
                outcome.problems.len() - shown.len()
            ));
        }
        gtk::AlertDialog::builder()
            .modal(true)
            .message(if outcome.problems.len() == 1 {
                "1 item couldn't be extracted".to_string()
            } else {
                format!("{} items couldn't be extracted", outcome.problems.len())
            })
            .detail(detail)
            .build()
            .show(Some(&this.window));
        return;
    }
    if show_files {
        show_folder_in_files(&this.window, &folder);
    }
    if close {
        this.window.close();
    }
}

fn launch_context(window: &gtk::ApplicationWindow) -> gdk::AppLaunchContext {
    WidgetExt::display(window).app_launch_context()
}

/// Opens a folder in the file manager.
fn show_folder_in_files(window: &gtk::ApplicationWindow, folder: &Path) {
    let uri = gio::File::for_path(folder).uri();
    let _ = gio::AppInfo::launch_default_for_uri(&uri, Some(&launch_context(window)));
}

/// Opens a file in its usual app.
fn open_file(window: &gtk::ApplicationWindow, file: &Path) {
    let uri = gio::File::for_path(file).uri();
    if let Err(error) = gio::AppInfo::launch_default_for_uri(&uri, Some(&launch_context(window))) {
        gtk::AlertDialog::builder()
            .modal(true)
            .message("The file couldn't be opened")
            .detail(error.message())
            .build()
            .show(Some(window));
    }
}
