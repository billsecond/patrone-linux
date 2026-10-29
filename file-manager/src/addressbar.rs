//! The row under the tabs: Back, Forward, Up and Refresh; the address bar,
//! with breadcrumbs whose chevrons list each folder's subfolders, and which
//! turns into a text field when clicked; and the search box.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::{gdk, gio, glib, prelude::*};

use crate::folderview::ATTRIBUTES;
use crate::glyph::{self, Glyph};
use crate::icons;
use crate::location::Location;
use crate::menu::{Icon, Menu};
use crate::places;
use crate::util;

pub enum AddressEvent {
    Navigate(Location),
    OpenInNewTab(Location),
    /// Text typed into the address bar and confirmed.
    Typed(String),
    Search(String),
    SearchCleared,
    /// Jump this many steps in history (negative is back).
    History(i32),
    Drop {
        files: Vec<gio::File>,
        folder: gio::File,
        action: gdk::DragAction,
    },
}

/// Lays out the breadcrumbs: from the end, as many folders as fit, each
/// with the chevron after it; the rest fold into the « button (the first
/// child), and `hidden` says how many folders it holds.
mod imp {
    use super::*;
    use gtk::subclass::prelude::*;

    #[derive(Default)]
    pub struct CrumbLayout {
        pub hidden: RefCell<Option<Rc<Cell<usize>>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CrumbLayout {
        const NAME: &'static str = "PfmCrumbLayout";
        type Type = super::CrumbLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for CrumbLayout {}

    impl LayoutManagerImpl for CrumbLayout {
        fn measure(
            &self,
            widget: &gtk::Widget,
            orientation: gtk::Orientation,
            _for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let (mut minimum, mut natural_size) = (0, 0);
            for child in children(widget) {
                let (min, nat, _, _) = child.measure(orientation, -1);
                if orientation == gtk::Orientation::Vertical {
                    minimum = minimum.max(min);
                    natural_size = natural_size.max(nat);
                } else if child.has_css_class("crumb-overflow") {
                    // At the least, « and whatever of the end fits.
                    minimum = nat;
                } else {
                    natural_size += nat;
                }
            }
            (minimum, natural_size.max(minimum), -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _baseline: i32) {
            let children = children(widget);
            let Some((overflow, rest)) = children.split_first() else {
                return;
            };
            // A folder and the chevron after it go together; the leading
            // chevron (the top-level places) is a part of its own.
            let mut parts: Vec<Vec<gtk::Widget>> = Vec::new();
            for child in rest {
                match parts.last_mut() {
                    Some(part) if !child.has_css_class("crumb") => part.push(child.clone()),
                    _ => parts.push(vec![child.clone()]),
                }
            }
            let part_width = |part: &[gtk::Widget]| part.iter().map(natural).sum::<i32>();
            let total: i32 = parts.iter().map(|part| part_width(part)).sum();
            let mut first_shown = 0;
            if total > width {
                let room = width - natural(overflow);
                let mut used = 0;
                first_shown = parts.len();
                for (index, part) in parts.iter().enumerate().rev() {
                    let part = part_width(part);
                    // The current folder always shows, cut short if need be.
                    if used + part > room && first_shown < parts.len() {
                        break;
                    }
                    used += part;
                    first_shown = index;
                }
            }
            let mut x = 0;
            overflow.set_child_visible(first_shown > 0);
            if first_shown > 0 {
                let w = natural(overflow);
                overflow.size_allocate(&gtk::Allocation::new(0, 0, w, height), -1);
                x = w;
            }
            let mut folded = 0;
            for (index, part) in parts.iter().enumerate() {
                for child in part {
                    let shown = index >= first_shown;
                    child.set_child_visible(shown);
                    if shown {
                        let w = natural(child);
                        child.size_allocate(&gtk::Allocation::new(x, 0, w, height), -1);
                        x += w;
                    } else if child.has_css_class("crumb") {
                        folded += 1;
                    }
                }
            }
            if let Some(hidden) = self.hidden.borrow().as_ref() {
                hidden.set(folded);
            }
        }
    }

    fn natural(widget: &gtk::Widget) -> i32 {
        widget.measure(gtk::Orientation::Horizontal, -1).1
    }
}

glib::wrapper! {
    pub struct CrumbLayout(ObjectSubclass<imp::CrumbLayout>) @extends gtk::LayoutManager;
}

fn crumb_layout(hidden: Rc<Cell<usize>>) -> CrumbLayout {
    use gtk::subclass::prelude::ObjectSubclassIsExt;
    let layout: CrumbLayout = glib::Object::new();
    layout.imp().hidden.replace(Some(hidden));
    layout
}

fn children(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut children = Vec::new();
    let mut child = widget.first_child();
    while let Some(current) = child {
        child = current.next_sibling();
        children.push(current);
    }
    children
}

pub struct AddressBar {
    pub widget: gtk::Box,
    field: gtk::Overlay,
    stack: gtk::Stack,
    icon: gtk::Image,
    crumbs: gtk::Box,
    /// The « button, and how many of the path's folders it holds.
    overflow: gtk::Button,
    hidden: Rc<Cell<usize>>,
    /// Each breadcrumb's name and place, from the top.
    targets: RefCell<Vec<(String, Location)>>,
    entry: gtk::Entry,
    search: gtk::Entry,
    progress: gtk::ProgressBar,
    pulse: RefCell<Option<glib::SourceId>>,
    location: RefCell<Option<Location>>,
    history: RefCell<(Vec<Location>, Vec<Location>)>,
    search_timer: RefCell<Option<glib::SourceId>>,
    quiet: std::cell::Cell<bool>,
    events: crate::util::Handler<AddressEvent>,
    me: RefCell<Weak<AddressBar>>,
}

impl AddressBar {
    pub fn new() -> Rc<AddressBar> {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        widget.add_css_class("address-row");
        for (glyph, tooltip, action) in [
            ("back", "Back (Alt+Left Arrow)", "win.back"),
            ("forward", "Forward (Alt+Right Arrow)", "win.forward"),
            ("up", "Up to the parent folder (Alt+Up Arrow)", "win.up"),
            ("refresh", "Refresh (F5)", "win.refresh"),
        ] {
            let button = glyph::button(glyph, tooltip);
            button.add_css_class("nav-button");
            button.set_action_name(Some(action));
            widget.append(&button);
        }

        let icon = gtk::Image::new();
        icon.add_css_class("address-icon");
        // The whole path, filled in from the end; what doesn't fit folds
        // into the « button at the start, as on Windows.
        let hidden = Rc::new(Cell::new(0));
        let crumbs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        crumbs.add_css_class("crumbs");
        crumbs.set_hexpand(true);
        crumbs.set_overflow(gtk::Overflow::Hidden);
        crumbs.set_layout_manager(Some(crumb_layout(hidden.clone())));
        let overflow = gtk::Button::with_label("\u{ab}");
        overflow.add_css_class("flat");
        overflow.add_css_class("crumb-overflow");
        overflow.set_tooltip_text(Some("The rest of the path"));
        let history_button = glyph::button("chevron-down", "Previous locations");
        history_button.add_css_class("history-button");
        let crumb_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        crumb_row.append(&icon);
        crumb_row.append(&crumbs);
        let entry = gtk::Entry::builder().hexpand(true).build();
        entry.add_css_class("address-entry");
        let stack = gtk::Stack::builder().hexpand(true).build();
        stack.add_named(&crumb_row, Some("crumbs"));
        stack.add_named(&entry, Some("edit"));
        let field_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        field_box.append(&stack);
        field_box.append(&history_button);
        let progress = gtk::ProgressBar::builder()
            .valign(gtk::Align::End)
            .visible(false)
            .can_target(false)
            .build();
        progress.add_css_class("address-progress");
        let field = gtk::Overlay::builder()
            .child(&field_box)
            .hexpand(true)
            .build();
        field.add_overlay(&progress);
        field.add_css_class("address-field");
        widget.append(&field);

        // A fixed width, as on Windows, so the path gets the rest of the row.
        let search = gtk::Entry::builder()
            .placeholder_text("Search")
            .hexpand(true)
            .width_chars(1)
            .max_width_chars(1)
            .build();
        search.add_css_class("search-entry");
        let search_icon = Glyph::new("search");
        search_icon.add_css_class("search-glyph");
        let search_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        search_box.set_hexpand(false);
        search_box.add_css_class("search-field");
        search_box.append(&search);
        search_box.append(&search_icon);
        widget.append(&search_box);

        let bar = Rc::new(AddressBar {
            widget,
            field,
            stack,
            icon,
            crumbs,
            overflow,
            hidden,
            targets: RefCell::default(),
            entry,
            search,
            progress,
            pulse: RefCell::default(),
            location: RefCell::default(),
            history: RefCell::default(),
            search_timer: RefCell::default(),
            quiet: std::cell::Cell::new(false),
            events: RefCell::default(),
            me: RefCell::default(),
        });
        *bar.me.borrow_mut() = Rc::downgrade(&bar);
        bar.connect_signals(&history_button);
        bar
    }

    pub fn connect_events(&self, events: impl Fn(AddressEvent) + 'static) {
        *self.events.borrow_mut() = Some(Rc::new(events));
    }

    fn emit(&self, event: AddressEvent) {
        let events = self.events.borrow().clone();
        if let Some(events) = events {
            events(event);
        }
    }

    fn connect_signals(&self, history_button: &gtk::Button) {
        // « lists the folders that didn't fit, nearest first, as Windows does.
        let weak = self.me.borrow().clone();
        self.overflow.connect_clicked(move |button| {
            let Some(bar) = weak.upgrade() else { return };
            let menu = Menu::new();
            menu.popover.add_css_class("crumb-menu");
            let targets = bar.targets.borrow().clone();
            let hidden = bar.hidden.get().min(targets.len());
            for (label, target) in targets[..hidden].iter().rev() {
                bar.navigate_item(&menu, target.small_icon(), label, target.clone());
            }
            menu.popup_below(button);
        });

        // Clicking the address bar's empty space edits the path.
        let click = gtk::GestureClick::new();
        let weak = self.me.borrow().clone();
        click.connect_released(move |_, _, _, _| {
            if let Some(bar) = weak.upgrade()
                && bar.stack.visible_child_name().as_deref() == Some("crumbs")
            {
                bar.edit();
            }
        });
        self.field.add_controller(click);

        let weak = self.me.borrow().clone();
        self.entry.connect_activate(move |entry| {
            let Some(bar) = weak.upgrade() else { return };
            let text = entry.text().to_string();
            bar.stop_editing();
            bar.emit(AddressEvent::Typed(text));
        });
        let keys = gtk::EventControllerKey::new();
        let weak = self.me.borrow().clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                if let Some(bar) = weak.upgrade() {
                    bar.stop_editing();
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        self.entry.add_controller(keys);
        let focus = gtk::EventControllerFocus::new();
        let weak = self.me.borrow().clone();
        focus.connect_leave(move |_| {
            if let Some(bar) = weak.upgrade() {
                bar.stop_editing();
            }
        });
        self.entry.add_controller(focus);

        let weak = self.me.borrow().clone();
        history_button.connect_clicked(move |button| {
            if let Some(bar) = weak.upgrade() {
                bar.show_history(button);
            }
        });

        // Searching starts when typing pauses, or on Enter.
        let weak = self.me.borrow().clone();
        self.search.connect_changed(move |search| {
            let Some(bar) = weak.upgrade() else { return };
            if bar.quiet.get() {
                return;
            }
            if let Some(timer) = bar.search_timer.take() {
                timer.remove();
            }
            let text = search.text().to_string();
            if text.trim().is_empty() {
                bar.emit(AddressEvent::SearchCleared);
                return;
            }
            let weak = Rc::downgrade(&bar);
            let timer = glib::timeout_add_local_once(Duration::from_millis(600), move || {
                if let Some(bar) = weak.upgrade() {
                    bar.search_timer.take();
                    bar.emit(AddressEvent::Search(text));
                }
            });
            bar.search_timer.replace(Some(timer));
        });
        let weak = self.me.borrow().clone();
        self.search.connect_activate(move |search| {
            let Some(bar) = weak.upgrade() else { return };
            if let Some(timer) = bar.search_timer.take() {
                timer.remove();
            }
            let text = search.text().to_string();
            if !text.trim().is_empty() {
                bar.emit(AddressEvent::Search(text));
            }
        });
        let keys = gtk::EventControllerKey::new();
        let weak = self.me.borrow().clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(bar) = weak.upgrade()
                && !bar.search.text().is_empty()
            {
                bar.search.set_text("");
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.search.add_controller(keys);
    }

    /// Shows where a tab is.
    pub fn set_location(&self, location: &Location) {
        self.location.replace(Some(location.clone()));
        self.stop_editing();
        icons::set_image(&self.icon, location.small_icon(), 16);
        while let Some(child) = self.crumbs.first_child() {
            self.crumbs.remove(&child);
        }
        self.crumbs.append(&self.overflow);
        // The first chevron lists the top-level places.
        self.crumbs.append(&self.chevron(None));
        let crumbs = location.crumbs();
        self.targets.replace(crumbs.clone());
        for (index, (label, target)) in crumbs.iter().enumerate() {
            let button = gtk::Button::with_label(label);
            button.add_css_class("flat");
            button.add_css_class("crumb");
            if index + 1 == crumbs.len() {
                button.add_css_class("current");
            }
            self.wire_crumb(&button, target);
            self.crumbs.append(&button);
            if matches!(target, Location::Folder(_) | Location::ThisPc)
                && !matches!(location, Location::Search { .. })
            {
                self.crumbs.append(&self.chevron(Some(target.clone())));
            }
        }
        let placeholder = match location {
            Location::Search { root, .. } => format!("Search {}", places::display_name(root)),
            other => format!("Search {}", other.title()),
        };
        self.search.set_placeholder_text(Some(&placeholder));
        self.quiet.set(true);
        match location {
            Location::Search { query, .. } => {
                if self.search.text() != query.as_str() {
                    self.search.set_text(query);
                }
            }
            _ => {
                if !self.search.has_focus() && self.search.focus_child().is_none() {
                    self.search.set_text("");
                }
            }
        }
        self.quiet.set(false);
    }

    fn wire_crumb(&self, button: &gtk::Button, target: &Location) {
        let weak = self.me.borrow().clone();
        let location = target.clone();
        button.connect_clicked(move |_| {
            if let Some(bar) = weak.upgrade() {
                bar.emit(AddressEvent::Navigate(location.clone()));
            }
        });
        let middle = gtk::GestureClick::new();
        middle.set_button(gdk::BUTTON_MIDDLE);
        let weak = self.me.borrow().clone();
        let location = target.clone();
        middle.connect_pressed(move |_, _, _, _| {
            if let Some(bar) = weak.upgrade() {
                bar.emit(AddressEvent::OpenInNewTab(location.clone()));
            }
        });
        button.add_controller(middle);
        if let Location::Folder(folder) = target {
            let drop = gtk::DropTarget::new(
                gdk::FileList::static_type(),
                gdk::DragAction::COPY | gdk::DragAction::MOVE,
            );
            drop.set_preload(true);
            {
                let folder = folder.clone();
                drop.connect_motion(move |drop, _, _| crate::dnd::preferred_action(drop, &folder));
            }
            let weak = self.me.borrow().clone();
            let folder = folder.clone();
            drop.connect_drop(move |drop, value, _, _| {
                let Some(bar) = weak.upgrade() else {
                    return false;
                };
                let Ok(list) = value.get::<gdk::FileList>() else {
                    return false;
                };
                let action = crate::dnd::drop_action(drop, &folder);
                bar.emit(AddressEvent::Drop {
                    files: list.files(),
                    folder: folder.clone(),
                    action,
                });
                true
            });
            button.add_controller(drop);
        }
    }

    /// A breadcrumb chevron: lists the subfolders of `parent`, or the
    /// top-level places for the first one.
    fn chevron(&self, parent: Option<Location>) -> gtk::Button {
        let glyph = Glyph::sized("chevron-right", 12);
        let button = gtk::Button::builder().child(&glyph).build();
        button.add_css_class("flat");
        button.add_css_class("crumb-chevron");
        let weak = self.me.borrow().clone();
        button.connect_clicked(move |button| {
            let Some(bar) = weak.upgrade() else { return };
            glyph.set_name("chevron-down");
            let menu = Menu::new();
            menu.popover.add_css_class("crumb-menu");
            {
                let glyph = glyph.clone();
                menu.popover
                    .connect_closed(move |_| glyph.set_name("chevron-right"));
            }
            match &parent {
                None => bar.fill_places(&menu),
                Some(Location::ThisPc) => bar.fill_drives(&menu),
                Some(Location::Folder(folder)) => {
                    bar.fill_subfolders(&menu, folder.clone(), button.clone());
                    return;
                }
                Some(_) => {}
            }
            menu.popup_below(button);
        });
        button
    }

    fn navigate_item(
        self: &Rc<Self>,
        menu: &Rc<Menu>,
        icon: &'static str,
        label: &str,
        location: Location,
    ) {
        let weak = Rc::downgrade(self);
        menu.item(Icon::Color(icon), label, None, true, move || {
            if let Some(bar) = weak.upgrade() {
                bar.emit(AddressEvent::Navigate(location.clone()));
            }
        });
    }

    fn fill_places(&self, menu: &Rc<Menu>) {
        let Some(me) = self.me.borrow().upgrade() else {
            return;
        };
        me.navigate_item(menu, "pfm-nav-home", "Home", Location::Home);
        me.navigate_item(menu, "pfm-nav-gallery", "Gallery", Location::Gallery);
        let home = places::home();
        me.navigate_item(
            menu,
            "pfm-nav-user",
            &places::display_name(&home),
            Location::Folder(home),
        );
        for (special, _) in places::USER_FOLDERS.iter().take(6) {
            if let Some(folder) = places::user_folder(*special) {
                me.navigate_item(
                    menu,
                    icons::small_folder_icon(&folder),
                    &places::display_name(&folder),
                    Location::Folder(folder),
                );
            }
        }
        me.navigate_item(menu, "pfm-nav-computer", "Computer", Location::ThisPc);
        me.navigate_item(
            menu,
            "pfm-nav-network",
            "Network",
            Location::Folder(places::network()),
        );
        let trash = places::trash();
        me.navigate_item(
            menu,
            icons::small_folder_icon(&trash),
            "Recycle Bin",
            Location::Folder(trash),
        );
    }

    fn fill_drives(&self, menu: &Rc<Menu>) {
        let Some(me) = self.me.borrow().upgrade() else {
            return;
        };
        me.navigate_item(
            menu,
            "pfm-nav-drive-system",
            &places::root_name(),
            Location::Folder(places::root()),
        );
        for mount in places::mounts() {
            me.navigate_item(
                menu,
                icons::small_drive_icon(&mount),
                &mount.name(),
                Location::Folder(mount.root()),
            );
        }
    }

    fn fill_subfolders(&self, menu: &Rc<Menu>, folder: gio::File, button: gtk::Button) {
        let weak = self.me.borrow().clone();
        let menu = menu.clone();
        glib::spawn_future_local(async move {
            let show_hidden = crate::config::flag("show-hidden");
            let mut folders = Vec::new();
            if let Ok(enumerator) = folder
                .enumerate_children_future(
                    ATTRIBUTES,
                    gio::FileQueryInfoFlags::NONE,
                    glib::Priority::DEFAULT,
                )
                .await
            {
                while let Ok(infos) = enumerator
                    .next_files_future(200, glib::Priority::DEFAULT)
                    .await
                {
                    if infos.is_empty() {
                        break;
                    }
                    for info in infos {
                        if util::is_dir(&info)
                            && (show_hidden || !(info.is_hidden() || info.is_backup()))
                        {
                            folders
                                .push((info.display_name().to_string(), folder.child(info.name())));
                        }
                    }
                }
            }
            folders.sort_by(|a, b| util::natural_cmp(&a.0, &b.0));
            let Some(bar) = weak.upgrade() else { return };
            if folders.is_empty() {
                menu.item(Icon::None, "No subfolders", None, false, || {});
            }
            for (name, child) in folders.into_iter().take(500) {
                bar.navigate_item(
                    &menu,
                    icons::small_folder_icon(&child),
                    &name,
                    Location::Folder(child),
                );
            }
            menu.popup_below(&button);
        });
    }

    fn show_history(&self, button: &gtk::Button) {
        let menu = Menu::new();
        let (back, forward) = self.history.borrow().clone();
        let weak = self.me.borrow().clone();
        let mut entries: Vec<(i32, Location)> = Vec::new();
        for (index, location) in forward.iter().enumerate().rev() {
            entries.push((index as i32 + 1, location.clone()));
        }
        if let Some(current) = self.location.borrow().clone() {
            entries.push((0, current));
        }
        for (index, location) in back.iter().enumerate() {
            entries.push((-(index as i32) - 1, location.clone()));
        }
        for (steps, location) in entries {
            let weak = weak.clone();
            let label = location.title();
            let action = move || {
                if let Some(bar) = weak.upgrade()
                    && steps != 0
                {
                    bar.emit(AddressEvent::History(steps));
                }
            };
            if steps == 0 {
                menu.radio(
                    Icon::Color(location.small_icon()),
                    &label,
                    None,
                    true,
                    action,
                );
            } else {
                menu.item(
                    Icon::Color(location.small_icon()),
                    &label,
                    None,
                    true,
                    action,
                );
            }
        }
        menu.popup_below(button);
    }

    /// The tab's history, for "Previous locations".
    pub fn set_history(&self, back: Vec<Location>, forward: Vec<Location>) {
        self.history.replace((back, forward));
    }

    /// Ctrl+L, Alt+D, F4: type a path.
    pub fn edit(&self) {
        let text = self
            .location
            .borrow()
            .as_ref()
            .map(Location::edit_text)
            .unwrap_or_default();
        self.entry.set_text(&text);
        self.stack.set_visible_child_name("edit");
        self.field.add_css_class("editing");
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
    }

    fn stop_editing(&self) {
        if self.stack.visible_child_name().as_deref() == Some("edit") {
            self.stack.set_visible_child_name("crumbs");
            self.field.remove_css_class("editing");
        }
    }

    /// Ctrl+E, Ctrl+F: search, optionally starting with some text.
    pub fn focus_search(&self, text: Option<&str>) {
        self.search.grab_focus();
        if let Some(text) = text {
            self.search
                .set_text(&format!("{}{text}", self.search.text()));
            self.search.set_position(-1);
        } else {
            self.search.select_region(0, -1);
        }
    }

    /// Shows a search in progress in the address bar.
    pub fn set_busy(&self, busy: bool) {
        self.progress.set_visible(busy);
        if busy {
            if self.pulse.borrow().is_none() {
                let progress = self.progress.clone();
                let id = glib::timeout_add_local(Duration::from_millis(120), move || {
                    progress.pulse();
                    glib::ControlFlow::Continue
                });
                self.pulse.replace(Some(id));
            }
        } else if let Some(id) = self.pulse.take() {
            id.remove();
        }
    }
}
