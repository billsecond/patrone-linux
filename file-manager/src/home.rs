//! Home, as in Windows 11: Quick access (pinned and frequent folders as
//! tiles), Favorites, and Recent files, each in a section that folds away.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk::{gdk, gio, glib, prelude::*};

use crate::config;
use crate::filecell;
use crate::folderview::{ATTRIBUTES, ViewEvent};
use crate::glyph::Glyph;
use crate::icons::{self, Kind};
use crate::util;
use crate::viewstate;

/// How many recent files Home lists.
const RECENT_LIMIT: usize = 30;
const FREQUENT_LIMIT: usize = 8;

/// The info a list shows, for a file that may have gone.
pub async fn info_for(file: &gio::File) -> Option<gio::FileInfo> {
    let info = file
        .query_info_future(
            ATTRIBUTES,
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::DEFAULT,
        )
        .await
        .ok()?;
    info.set_attribute_object("standard::file", file);
    Some(info)
}

/// A folding section: a heading with a chevron, and its content.
pub fn section(title: &str, content: &impl IsA<gtk::Widget>) -> (gtk::Box, gtk::Label) {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 4);
    outer.add_css_class("home-section");
    let chevron = Glyph::sized("chevron-down", 12);
    let label = gtk::Label::builder().label(title).xalign(0.0).build();
    label.add_css_class("section-title");
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    heading.append(&chevron);
    heading.append(&label);
    let button = gtk::Button::builder()
        .child(&heading)
        .halign(gtk::Align::Start)
        .build();
    button.add_css_class("flat");
    button.add_css_class("section-heading");
    let revealer = gtk::Revealer::builder()
        .child(content)
        .reveal_child(true)
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .build();
    {
        let revealer = revealer.clone();
        button.connect_clicked(move |_| {
            let open = !revealer.reveals_child();
            revealer.set_reveal_child(open);
            chevron.set_name(if open {
                "chevron-down"
            } else {
                "chevron-right"
            });
        });
    }
    outer.append(&button);
    outer.append(&revealer);
    (outer, label)
}

/// A row of a Home list: icon and name, a date, and where the file is.
fn list_row(
    info: &gio::FileInfo,
    date: Option<glib::DateTime>,
    hide_extensions: bool,
) -> gtk::ListBoxRow {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("home-row");
    let name_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    name_box.set_size_request(160, -1);
    name_box.set_hexpand(true);
    name_box.append(&icons::image(icons::for_info(info), 16));
    let name = gtk::Label::builder()
        .label(filecell::shown_name(info, hide_extensions))
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .hexpand(true)
        .build();
    name_box.append(&name);
    row.append(&name_box);
    let date = gtk::Label::builder()
        .label(date.map(|d| util::date_time(&d)).unwrap_or_default())
        .xalign(0.0)
        .width_request(160)
        .build();
    date.add_css_class("item-detail");
    row.append(&date);
    let location = util::file_of(info)
        .and_then(|file| file.parent())
        .map(|folder| util::display_path(&folder))
        .unwrap_or_default();
    let location = gtk::Label::builder()
        .label(location)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .hexpand(true)
        .width_request(80)
        .build();
    location.add_css_class("item-detail");
    row.append(&location);
    let list_row = gtk::ListBoxRow::builder().child(&row).build();
    if config::flag("show-tooltips") {
        list_row.set_tooltip_text(Some(&filecell::tooltip_text(info)));
    }
    list_row
}

/// Column titles laid out like `list_row`: name and location share the
/// spare width, the date is fixed.
fn header_row(date: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("home-header");
    let name = gtk::Label::builder()
        .label("Name")
        .xalign(0.0)
        .hexpand(true)
        .width_request(160)
        .build();
    let date = gtk::Label::builder()
        .label(date)
        .xalign(0.0)
        .width_request(160)
        .build();
    let location = gtk::Label::builder()
        .label("File location")
        .xalign(0.0)
        .hexpand(true)
        .width_request(80)
        .build();
    row.append(&name);
    row.append(&date);
    row.append(&location);
    row
}

/// A tile in Quick access: a folder icon, its name, where it is, and a pin.
fn tile(info: &gio::FileInfo, file: &gio::File, pinned: bool) -> gtk::FlowBoxChild {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("quick-tile");
    let icon = icons::image(icons::folder_icon(file), 40);
    row.append(&icon);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    let name = gtk::Label::builder()
        .label(info.display_name().as_str())
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(18)
        .build();
    text.append(&name);
    let place = file
        .parent()
        .map(|folder| crate::places::display_name(&folder))
        .unwrap_or_default();
    let detail = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let place = gtk::Label::builder()
        .label(place)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(16)
        .build();
    place.add_css_class("item-detail");
    detail.append(&place);
    if pinned {
        let pin = Glyph::sized("pin", 12);
        pin.add_css_class("dim");
        detail.append(&pin);
    }
    text.append(&detail);
    row.append(&text);
    let child = gtk::FlowBoxChild::builder().child(&row).build();
    child.set_size_request(232, -1);
    child
}

pub struct HomePage {
    pub widget: gtk::ScrolledWindow,
    quick: gtk::FlowBox,
    favorites: gtk::ListBox,
    favorites_empty: gtk::Label,
    recent: gtk::ListBox,
    recent_section: gtk::Box,
    recent_empty: gtk::Label,
    quick_infos: RefCell<Vec<gio::FileInfo>>,
    favorite_infos: RefCell<Vec<gio::FileInfo>>,
    recent_infos: RefCell<Vec<gio::FileInfo>>,
    kinds: RefCell<Vec<Kind>>,
    events: crate::util::Handler<ViewEvent>,
    me: RefCell<Weak<HomePage>>,
}

impl HomePage {
    pub fn new() -> Rc<HomePage> {
        let quick = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::Multiple)
            .activate_on_single_click(false)
            .homogeneous(true)
            .max_children_per_line(8)
            .min_children_per_line(1)
            .column_spacing(4)
            .row_spacing(4)
            .build();
        quick.add_css_class("quick-access");
        let list = || {
            let list = gtk::ListBox::builder()
                .selection_mode(gtk::SelectionMode::Multiple)
                .activate_on_single_click(false)
                .build();
            list.add_css_class("home-list");
            list
        };
        let favorites = list();
        let recent = list();
        let empty = |text: &str| {
            let label = gtk::Label::builder()
                .label(text)
                .xalign(0.0)
                .wrap(true)
                .width_chars(30)
                .visible(false)
                .build();
            label.add_css_class("home-empty");
            label
        };
        let favorites_empty = empty("After you've favorited some files, we'll show them here.");
        let recent_empty =
            empty("After you've opened some files, we'll show the most recent ones here.");

        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("home-page");
        let (quick_section, _) = section("Quick access", &quick);
        content.append(&quick_section);

        let favorites_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        favorites_box.append(&header_row("Date modified"));
        favorites_box.append(&favorites);
        favorites_box.append(&favorites_empty);
        let (favorites_section, _) = section("Favorites", &favorites_box);
        content.append(&favorites_section);

        let recent_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        recent_box.append(&header_row("Date accessed"));
        recent_box.append(&recent);
        recent_box.append(&recent_empty);
        let (recent_section, _) = section("Recent", &recent_box);
        content.append(&recent_section);

        let widget = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .build();
        widget.add_css_class("file-list");

        let page = Rc::new(HomePage {
            widget,
            quick,
            favorites,
            favorites_empty,
            recent,
            recent_section,
            recent_empty,
            quick_infos: RefCell::default(),
            favorite_infos: RefCell::default(),
            recent_infos: RefCell::default(),
            kinds: RefCell::default(),
            events: RefCell::default(),
            me: RefCell::default(),
        });
        *page.me.borrow_mut() = Rc::downgrade(&page);
        page.connect_signals();
        page.reload();
        page
    }

    pub fn connect_events(&self, events: impl Fn(ViewEvent) + 'static) {
        *self.events.borrow_mut() = Some(Rc::new(events));
    }

    fn emit(&self, event: ViewEvent) {
        let events = self.events.borrow().clone();
        if let Some(events) = events {
            events(event);
        }
    }

    fn connect_signals(&self) {
        let weak = self.me.borrow().clone();
        self.quick.connect_child_activated(move |_, child| {
            let Some(page) = weak.upgrade() else { return };
            let info = page
                .quick_infos
                .borrow()
                .get(child.index() as usize)
                .cloned();
            if let Some(info) = info {
                page.emit(ViewEvent::Activate(vec![info]));
            }
        });
        for (list, which) in [(&self.favorites, 0), (&self.recent, 1)] {
            let weak = self.me.borrow().clone();
            list.connect_row_activated(move |_, row| {
                let Some(page) = weak.upgrade() else { return };
                let infos = if which == 0 {
                    page.favorite_infos.borrow()
                } else {
                    page.recent_infos.borrow()
                };
                if let Some(info) = infos.get(row.index() as usize).cloned() {
                    drop(infos);
                    page.emit(ViewEvent::Activate(vec![info]));
                }
            });
        }
        // Selecting in one section clears the others, as on Windows.
        let weak = self.me.borrow().clone();
        self.quick.connect_selected_children_changed(move |quick| {
            let Some(page) = weak.upgrade() else { return };
            if !quick.selected_children().is_empty() {
                page.favorites.unselect_all();
                page.recent.unselect_all();
            }
            page.emit(ViewEvent::SelectionChanged);
        });
        for (list, other) in [
            (&self.favorites, &self.recent),
            (&self.recent, &self.favorites),
        ] {
            let weak = self.me.borrow().clone();
            let other = other.clone();
            list.connect_selected_rows_changed(move |list| {
                let Some(page) = weak.upgrade() else { return };
                if !list.selected_rows().is_empty() {
                    page.quick.unselect_all();
                    other.unselect_all();
                }
                page.emit(ViewEvent::SelectionChanged);
            });
        }
        // Right-click selects what's under the pointer, then opens a menu.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        let weak = self.me.borrow().clone();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(page) = weak.upgrade() else { return };
            let Some(widget) = gesture.widget() else {
                return;
            };
            gesture.set_state(gtk::EventSequenceState::Claimed);
            let on_item = page.select_at(&widget, x, y);
            page.emit(ViewEvent::ContextMenu {
                on_item,
                widget,
                x,
                y,
            });
        });
        self.widget.add_controller(click);

        let weak = self.me.borrow().clone();
        config::settings().connect_changed(None, move |_, key| {
            if matches!(
                key,
                "pinned-folders"
                    | "favorite-files"
                    | "show-recent-files"
                    | "show-frequent-folders"
                    | "hide-extensions"
            ) && let Some(page) = weak.upgrade()
            {
                page.reload();
            }
        });
        let weak = self.me.borrow().clone();
        gtk::RecentManager::default().connect_changed(move |_| {
            if let Some(page) = weak.upgrade() {
                page.reload_recent();
            }
        });
    }

    fn select_at(&self, widget: &gtk::Widget, x: f64, y: f64) -> bool {
        let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) else {
            return false;
        };
        let mut current = Some(picked);
        while let Some(candidate) = current {
            if let Some(child) = candidate.downcast_ref::<gtk::FlowBoxChild>() {
                if !child.is_selected() {
                    self.quick.unselect_all();
                    self.quick.select_child(child);
                }
                return true;
            }
            if let Some(row) = candidate.downcast_ref::<gtk::ListBoxRow>() {
                if let Some(list) = row.parent().and_downcast::<gtk::ListBox>()
                    && !row.is_selected()
                {
                    list.unselect_all();
                    list.select_row(Some(row));
                }
                return true;
            }
            current = candidate.parent();
        }
        self.select_none();
        false
    }

    /// Reads everything again.
    pub fn reload(&self) {
        self.reload_quick();
        self.reload_favorites();
        self.reload_recent();
    }

    fn reload_quick(&self) {
        let weak = self.me.borrow().clone();
        glib::spawn_future_local(async move {
            let pinned = config::pinned();
            let mut folders: Vec<(gio::File, bool)> =
                pinned.iter().map(|f| (f.clone(), true)).collect();
            if config::flag("show-frequent-folders") {
                for frequent in viewstate::frequent_folders(FREQUENT_LIMIT + pinned.len()) {
                    if folders.len() >= pinned.len() + FREQUENT_LIMIT {
                        break;
                    }
                    if !folders.iter().any(|(known, _)| known.equal(&frequent)) {
                        folders.push((frequent, false));
                    }
                }
            }
            let mut tiles = Vec::new();
            for (folder, pinned) in folders {
                if let Some(info) = info_for(&folder).await {
                    tiles.push((info, folder, pinned));
                }
            }
            let Some(page) = weak.upgrade() else { return };
            page.quick.remove_all();
            let mut infos = Vec::new();
            for (info, folder, pinned) in tiles {
                page.quick.append(&tile(&info, &folder, pinned));
                infos.push(info);
            }
            page.quick_infos.replace(infos);
            page.emit(ViewEvent::ItemsChanged);
        });
    }

    fn reload_favorites(&self) {
        let weak = self.me.borrow().clone();
        glib::spawn_future_local(async move {
            let mut infos = Vec::new();
            for file in config::favorites() {
                if let Some(info) = info_for(&file).await {
                    infos.push(info);
                }
            }
            if let Some(page) = weak.upgrade() {
                page.favorite_infos.replace(infos);
                page.fill(true);
            }
        });
    }

    fn reload_recent(&self) {
        let show = config::flag("show-recent-files");
        self.recent_section.set_visible(show);
        if !show {
            self.recent_infos.replace(Vec::new());
            return;
        }
        let weak = self.me.borrow().clone();
        glib::spawn_future_local(async move {
            let mut items = gtk::RecentManager::default().items();
            items.sort_by_key(|item| std::cmp::Reverse(item.modified().to_unix()));
            let mut infos = Vec::new();
            for item in items {
                if infos.len() >= RECENT_LIMIT {
                    break;
                }
                let file = gio::File::for_uri(&item.uri());
                if file.is_native() && !item.exists() {
                    continue;
                }
                if let Some(info) = info_for(&file).await {
                    if util::is_dir(&info) {
                        continue;
                    }
                    info.set_attribute_int64("pfm::recent-time", item.modified().to_unix());
                    infos.push(info);
                }
            }
            if let Some(page) = weak.upgrade() {
                page.recent_infos.replace(infos);
                page.fill(false);
            }
        });
    }

    /// Puts the favorites (or recent files) into their list, filtered.
    fn fill(&self, favorites: bool) {
        let (list, infos, empty) = if favorites {
            (&self.favorites, &self.favorite_infos, &self.favorites_empty)
        } else {
            (&self.recent, &self.recent_infos, &self.recent_empty)
        };
        list.remove_all();
        let hide = config::flag("hide-extensions");
        let kinds = self.kinds.borrow().clone();
        let shown: Vec<gio::FileInfo> = infos
            .borrow()
            .iter()
            .filter(|info| {
                kinds.is_empty()
                    || kinds.contains(&icons::kind_of(
                        &util::content_type(info),
                        util::is_dir(info),
                    ))
            })
            .cloned()
            .collect();
        for info in &shown {
            let date = if favorites {
                util::modified(info)
            } else {
                glib::DateTime::from_unix_local(info.attribute_int64("pfm::recent-time")).ok()
            };
            list.append(&list_row(info, date, hide));
        }
        infos.replace(shown.clone());
        empty.set_visible(shown.is_empty());
        list.set_visible(!shown.is_empty());
        self.emit(ViewEvent::ItemsChanged);
    }

    /// The Filter menu on Home filters Favorites and Recent.
    pub fn set_kind_filter(&self, kinds: Vec<Kind>) {
        self.kinds.replace(kinds);
        self.reload_favorites();
        self.reload_recent();
    }

    pub fn kind_filter(&self) -> Vec<Kind> {
        self.kinds.borrow().clone()
    }

    pub fn selected_infos(&self) -> Vec<gio::FileInfo> {
        let mut infos = Vec::new();
        for child in self.quick.selected_children() {
            if let Some(info) = self.quick_infos.borrow().get(child.index() as usize) {
                infos.push(info.clone());
            }
        }
        for row in self.favorites.selected_rows() {
            if let Some(info) = self.favorite_infos.borrow().get(row.index() as usize) {
                infos.push(info.clone());
            }
        }
        for row in self.recent.selected_rows() {
            if let Some(info) = self.recent_infos.borrow().get(row.index() as usize) {
                infos.push(info.clone());
            }
        }
        infos
    }

    pub fn n_items(&self) -> u32 {
        (self.quick_infos.borrow().len()
            + self.favorite_infos.borrow().len()
            + self.recent_infos.borrow().len()) as u32
    }

    pub fn select_all(&self) {
        self.recent.select_all();
    }

    pub fn select_none(&self) {
        self.quick.unselect_all();
        self.favorites.unselect_all();
        self.recent.unselect_all();
    }

    pub fn focus(&self) {
        self.quick.grab_focus();
    }
}
