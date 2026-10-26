//! A File Manager window: the tabs in the title bar, the address row, the
//! command bar, the navigation pane, the tab's content, the details or
//! preview pane, and the status bar. Every command is a window action
//! (`win.copy`, `win.view`...) so buttons, menus and shortcuts share them.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk::{gdk, gio, glib, prelude::*};

use crate::addressbar::{AddressBar, AddressEvent};
use crate::clipboard;
use crate::commandbar::{CommandBar, Contextual, MenuKind};
use crate::commands;
use crate::config;
use crate::folderview::ViewEvent;
use crate::glyph::Glyph;
use crate::location::Location;
use crate::menus;
use crate::navpane::{NavEvent, NavPane};
use crate::places;
use crate::sidepane::{Mode, SidePane};
use crate::tab::{Page, Tab, TabEvent};
use crate::tabbar::{TabBar, TabBarEvent};
use crate::undo;
use crate::util;
use crate::viewstate::{Column, VIEWS, View};

#[derive(Clone)]
pub struct Window(pub Rc<Inner>);

#[derive(Clone)]
pub struct WeakWindow(Weak<Inner>);

impl WeakWindow {
    pub fn upgrade(&self) -> Option<Window> {
        self.0.upgrade().map(Window)
    }
}

pub struct Inner {
    pub window: gtk::ApplicationWindow,
    pub tabbar: Rc<TabBar>,
    pub address: Rc<AddressBar>,
    pub commandbar: CommandBar,
    pub nav: Rc<NavPane>,
    pub side: Rc<SidePane>,
    status: StatusBar,
    stack: gtk::Stack,
    outer_paned: gtk::Paned,
    side_paned: gtk::Paned,
    tabs: RefCell<Vec<Rc<Tab>>>,
    current: RefCell<Option<Rc<Tab>>>,
}

impl std::ops::Deref for Window {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

struct StatusBar {
    widget: gtk::Box,
    items: gtk::Label,
    selected: gtk::Label,
    details: gtk::ToggleButton,
    large: gtk::ToggleButton,
}

thread_local! {
    /// Owns each window's state until GTK destroys the window; everything
    /// else refers to it weakly.
    static WINDOWS: RefCell<Vec<Rc<Inner>>> = const { RefCell::new(Vec::new()) };
}

/// Every open window.
pub fn all() -> Vec<Window> {
    WINDOWS.with(|windows| windows.borrow().iter().cloned().map(Window).collect())
}

/// Remembers the folder each window shows, for "Restore previous folder
/// windows at sign-in". A sign-out ends the app without closing windows,
/// so the list is kept current as windows move around.
fn save_open_windows() {
    let settings = config::settings();
    if !settings.boolean("restore-windows") {
        if !settings.strv("open-windows").is_empty() {
            let _ = settings.set_strv("open-windows", &[] as &[&str]);
        }
        return;
    }
    let uris: Vec<String> = all()
        .iter()
        .filter_map(|window| window.current.borrow().as_ref().map(|tab| tab.location()))
        .filter_map(|location| location.folder().map(|folder| folder.uri().to_string()))
        .collect();
    let uris: Vec<&str> = uris.iter().map(String::as_str).collect();
    if settings
        .strv("open-windows")
        .iter()
        .map(|s| s.as_str())
        .ne(uris.iter().copied())
    {
        let _ = settings.set_strv("open-windows", uris.as_slice());
    }
}

/// Where a new window opens, from Folder Options.
pub fn start_location() -> Location {
    if config::string("open-to") == "this-pc" {
        Location::ThisPc
    } else {
        Location::Home
    }
}

impl Window {
    pub fn new(app: &gtk::Application, location: Location) -> Window {
        let settings = config::settings();
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("File Manager")
            .default_width(settings.int("window-width").max(480))
            .default_height(settings.int("window-height").max(360))
            .build();
        window.add_css_class("file-manager");
        if settings.boolean("window-maximized") {
            window.maximize();
        }

        let tabbar = TabBar::new(&window);
        window.set_titlebar(Some(&tabbar.widget));

        let address = AddressBar::new();
        let menus_slot: Rc<RefCell<Option<WeakWindow>>> = Rc::default();
        let commandbar = {
            let slot = menus_slot.clone();
            CommandBar::new(Rc::new(move |kind: MenuKind, menu| {
                if let Some(window) = slot.borrow().as_ref().and_then(WeakWindow::upgrade) {
                    menus::dropdown(&window, kind, menu);
                }
            }))
        };
        let header = gtk::Box::new(gtk::Orientation::Vertical, 0);
        header.add_css_class("header");
        header.append(&address.widget);
        // A narrow window clips the command bar instead of growing.
        let command_clip = gtk::ScrolledWindow::builder()
            .child(&commandbar.widget)
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .build();
        header.append(&command_clip);

        let nav = NavPane::new();
        let side = SidePane::new();
        let stack = gtk::Stack::builder()
            .hexpand(true)
            .vexpand(true)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .build();
        stack.add_css_class("content");
        let side_paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .start_child(&stack)
            .end_child(&side.widget)
            .resize_start_child(true)
            .resize_end_child(false)
            .shrink_end_child(false)
            .build();
        let outer_paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .start_child(&nav.widget)
            .end_child(&side_paned)
            .resize_start_child(false)
            .shrink_start_child(false)
            .position(settings.int("navigation-pane-width"))
            .build();
        outer_paned.add_css_class("body");

        let status = StatusBar::new();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&header);
        content.append(&outer_paned);
        content.append(&status.widget);
        window.set_child(Some(&content));

        let inner = Rc::new(Inner {
            window,
            tabbar,
            address,
            commandbar,
            nav,
            side,
            status,
            stack,
            outer_paned,
            side_paned,
            tabs: RefCell::default(),
            current: RefCell::default(),
        });
        let window = Window(inner);
        menus_slot.replace(Some(window.downgrade()));
        WINDOWS.with(|windows| windows.borrow_mut().push(window.0.clone()));
        let weak = window.downgrade();
        window.window.connect_destroy(move |_| {
            if let Some(window) = weak.upgrade() {
                for tab in window.tabs() {
                    tab.release();
                }
                WINDOWS.with(|windows| {
                    windows
                        .borrow_mut()
                        .retain(|inner| !Rc::ptr_eq(inner, &window.0))
                });
                save_open_windows();
            }
        });
        window.add_actions();
        window.add_shortcuts();
        window.connect_parts();
        window.apply_layout();
        window.new_tab(location, true);
        window
    }

    pub fn downgrade(&self) -> WeakWindow {
        WeakWindow(Rc::downgrade(&self.0))
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn gtk_window(&self) -> gtk::Window {
        self.window.clone().upcast()
    }

    /// The tab on screen.
    pub fn tab(&self) -> Rc<Tab> {
        self.current
            .borrow()
            .clone()
            .expect("a window always has a tab")
    }

    pub fn tabs(&self) -> Vec<Rc<Tab>> {
        self.tabs.borrow().clone()
    }

    fn tab_index(&self, tab: &Rc<Tab>) -> Option<usize> {
        self.tabs.borrow().iter().position(|t| Rc::ptr_eq(t, tab))
    }

    // --- Tabs ---------------------------------------------------------------

    /// Opens a tab after the current one.
    pub fn new_tab(&self, location: Location, activate: bool) -> Rc<Tab> {
        let tab = Tab::new();
        let weak = self.downgrade();
        tab.connect_events(move |tab, event| {
            if let Some(window) = weak.upgrade() {
                window.on_tab_event(tab, event);
            }
        });
        let index = self
            .current
            .borrow()
            .as_ref()
            .and_then(|current| self.tab_index(current))
            .map(|index| index + 1)
            .unwrap_or(self.tabs.borrow().len());
        self.tabs.borrow_mut().insert(index, tab.clone());
        self.stack.add_child(&tab.content);
        self.tabbar
            .insert(index, &location.title(), location.small_icon());
        tab.navigate(location);
        if activate || self.current.borrow().is_none() {
            self.select_tab(index);
        }
        tab
    }

    pub fn select_tab(&self, index: usize) {
        let Some(tab) = self.tabs.borrow().get(index).cloned() else {
            return;
        };
        self.current.replace(Some(tab.clone()));
        self.stack.set_visible_child(&tab.content);
        self.tabbar.set_active(index);
        self.update_all();
        tab.focus();
    }

    pub fn close_tab(&self, index: usize) {
        if self.tabs.borrow().len() <= 1 {
            self.window.close();
            return;
        }
        let tab = self.tabs.borrow_mut().remove(index);
        tab.release();
        self.stack.remove(&tab.content);
        self.tabbar.remove(index);
        let was_current = self
            .current
            .borrow()
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, &tab));
        if was_current {
            let next = index.min(self.tabs.borrow().len() - 1);
            self.select_tab(next);
        } else if let Some(current) = self.current.borrow().clone()
            && let Some(position) = self.tab_index(&current)
        {
            self.tabbar.set_active(position);
        }
    }

    pub fn close_other_tabs(&self, keep: usize) {
        let count = self.tabs.borrow().len();
        for index in (0..count).rev() {
            if index != keep {
                self.close_tab(index);
            }
        }
    }

    pub fn close_tabs_to_the_right(&self, index: usize) {
        while self.tabs.borrow().len() > index + 1 {
            let last = self.tabs.borrow().len() - 1;
            self.close_tab(last);
        }
    }

    fn move_tab(&self, from: usize, to: usize) {
        let mut tabs = self.tabs.borrow_mut();
        if from >= tabs.len() || to >= tabs.len() {
            return;
        }
        let tab = tabs.remove(from);
        tabs.insert(to, tab);
        drop(tabs);
        self.tabbar.move_tab(from, to);
        if let Some(current) = self.current.borrow().clone()
            && let Some(index) = self.tab_index(&current)
        {
            self.tabbar.set_active(index);
        }
    }

    fn cycle_tab(&self, step: i32) {
        let count = self.tabs.borrow().len() as i32;
        let Some(index) = self.tab_index(&self.tab()) else {
            return;
        };
        self.select_tab(((index as i32 + step).rem_euclid(count)) as usize);
    }

    // --- Navigation -----------------------------------------------------------

    /// Goes to a place in the current tab, or a new window when Folder
    /// Options says so.
    pub fn navigate(&self, location: Location) {
        self.tab().navigate(location);
    }

    /// Resolves what was typed in the address bar.
    fn go_to_typed(&self, text: &str) {
        let current = self.tab().location().folder().cloned();
        match crate::location::parse_typed(text, current.as_ref()) {
            Some(crate::location::Typed::Place(location)) => self.navigate(location),
            Some(crate::location::Typed::File(file)) => {
                let window = self.clone();
                let text = text.to_owned();
                glib::spawn_future_local(async move {
                    match crate::home::info_for(&file).await {
                        Some(info) if util::is_dir(&info) => {
                            window.navigate(Location::Folder(file))
                        }
                        Some(info) => {
                            commands::open_infos(&window, vec![info], commands::OpenIn::Default)
                        }
                        None if !file.is_native() => window.navigate(Location::Folder(file)),
                        None => {
                            crate::dialogs::error(
                                Some(&window.gtk_window()),
                                "File Manager",
                                &format!(
                                    "File Manager can't find '{text}'. Check the spelling and try again."
                                ),
                                "",
                            );
                        }
                    }
                });
            }
            None => {}
        }
    }

    // --- Events ---------------------------------------------------------------

    fn on_tab_event(&self, tab: &Rc<Tab>, event: TabEvent) {
        let is_current = self
            .current
            .borrow()
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, tab));
        match event {
            TabEvent::LocationChanged => {
                let location = tab.location();
                if let Some(index) = self.tab_index(tab) {
                    self.tabbar
                        .set(index, &location.title(), location.small_icon());
                }
                if is_current {
                    self.update_all();
                    // As on Windows, the file list has the focus after moving,
                    // unless someone is typing in the address bar or search box.
                    let typing =
                        gtk::prelude::GtkWindowExt::focus(&self.window).is_some_and(|focus| {
                            focus.is::<gtk::Text>()
                                || focus.ancestor(gtk::Entry::static_type()).is_some()
                        });
                    if !typing {
                        tab.focus();
                    }
                }
            }
            TabEvent::View(event) => match event {
                ViewEvent::Activate(infos) => {
                    commands::open_infos(self, infos, commands::OpenIn::Default)
                }
                ViewEvent::ContextMenu {
                    on_item,
                    widget,
                    x,
                    y,
                } => {
                    if on_item {
                        menus::item_menu(self, &widget, x, y);
                    } else {
                        menus::background_menu(self, &widget, x, y);
                    }
                }
                ViewEvent::SelectionChanged | ViewEvent::ItemsChanged => {
                    if is_current {
                        self.update_selection();
                    }
                }
                ViewEvent::Drop {
                    files,
                    folder,
                    action,
                } => commands::drop_files(self, files, folder, action),
                ViewEvent::Rename { file, name } => commands::rename_to(self, file, name),
                ViewEvent::StateChanged => {
                    if is_current {
                        self.update_view_actions();
                    }
                }
                ViewEvent::TypeToSearch(text) => self.address.focus_search(Some(&text)),
                ViewEvent::SearchRunning(running) => {
                    if is_current {
                        self.address.set_busy(running);
                    }
                }
            },
        }
    }

    fn connect_parts(&self) {
        let weak = self.downgrade();
        self.tabbar.connect_events(move |event| {
            let Some(window) = weak.upgrade() else { return };
            match event {
                TabBarEvent::Select(index) => window.select_tab(index),
                TabBarEvent::Close(index) => window.close_tab(index),
                TabBarEvent::New => {
                    window.new_tab(start_location(), true);
                }
                TabBarEvent::Menu {
                    index,
                    widget,
                    x,
                    y,
                } => menus::tab_menu(&window, index, &widget, x, y),
                TabBarEvent::Move { from, to } => window.move_tab(from, to),
            }
        });

        let weak = self.downgrade();
        self.address.connect_events(move |event| {
            let Some(window) = weak.upgrade() else { return };
            match event {
                AddressEvent::Navigate(location) => window.navigate(location),
                AddressEvent::OpenInNewTab(location) => {
                    window.new_tab(location, false);
                }
                AddressEvent::Typed(text) => window.go_to_typed(&text),
                AddressEvent::Search(query) => {
                    let root = window.tab().location().search_root();
                    window.navigate(Location::Search { root, query });
                }
                AddressEvent::SearchCleared => {
                    let tab = window.tab();
                    if let Location::Search { root, .. } = tab.location() {
                        tab.navigate(Location::Folder(root));
                    }
                }
                AddressEvent::History(steps) => window.tab().go_history(steps),
                AddressEvent::Drop {
                    files,
                    folder,
                    action,
                } => commands::drop_files(&window, files, folder, action),
            }
        });

        let weak = self.downgrade();
        self.nav.connect_events(move |event| {
            let Some(window) = weak.upgrade() else { return };
            match event {
                NavEvent::Navigate(location) => window.navigate(location),
                NavEvent::OpenInNewTab(location) => {
                    window.new_tab(location, false);
                }
                NavEvent::OpenVolume(drive) => window.tab().this_pc().open_drive(drive),
                NavEvent::ContextMenu {
                    kind,
                    pinned,
                    widget,
                    x,
                    y,
                } => menus::nav_menu(&window, kind, pinned, &widget, x, y),
                NavEvent::Drop {
                    files,
                    folder,
                    action,
                } => commands::drop_files(&window, files, folder, action),
            }
        });

        let weak = self.downgrade();
        config::settings().connect_changed(None, move |_, key| {
            let Some(window) = weak.upgrade() else { return };
            match key {
                "show-navigation-pane" | "right-pane" | "show-status-bar" => window.apply_layout(),
                "show-full-path-in-title" => window.update_title(),
                _ => {}
            }
            if matches!(
                key,
                "show-hidden"
                    | "hide-extensions"
                    | "single-click"
                    | "underline-titles"
                    | "item-checkboxes"
                    | "show-tooltips"
                    | "show-thumbnails"
                    | "typing-in-view"
            ) {
                for tab in window.tabs() {
                    tab.folder.apply_settings();
                }
                window.update_view_actions();
            }
        });

        let weak = self.downgrade();
        clipboard::connect_cut_changed(move || {
            if let Some(window) = weak.upgrade() {
                for tab in window.tabs() {
                    tab.folder.rebind();
                }
                window.update_actions();
            }
        });
        let weak = self.downgrade();
        undo::connect_changed(move || {
            if let Some(window) = weak.upgrade() {
                window.update_actions();
            }
        });
        let weak = self.downgrade();
        gdk::Display::default()
            .expect("a display")
            .clipboard()
            .connect_changed(move |_| {
                if let Some(window) = weak.upgrade() {
                    window.update_actions();
                }
            });

        // Remember the window's size and panes.
        self.window.connect_close_request(|window| {
            let settings = config::settings();
            let _ = settings.set_boolean("window-maximized", window.is_maximized());
            if !window.is_maximized() {
                let (width, height) = window.default_size();
                let _ = settings.set_int("window-width", width);
                let _ = settings.set_int("window-height", height);
            }
            glib::Propagation::Proceed
        });
        let weak = self.downgrade();
        self.outer_paned.connect_position_notify(move |paned| {
            if weak.upgrade().is_some() && paned.position() > 40 {
                let _ = config::settings().set_int("navigation-pane-width", paned.position());
            }
        });
        let weak = self.downgrade();
        self.side_paned.connect_position_notify(move |paned| {
            let Some(window) = weak.upgrade() else { return };
            if window.side.widget.is_visible() {
                let width = paned.width() - paned.position();
                if width > 120 {
                    let _ = config::settings().set_int("right-pane-width", width);
                }
            }
        });
        let weak = self.downgrade();
        self.status.details.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.set_view(View::Details);
            }
        });
        let weak = self.downgrade();
        self.status.large.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.set_view(View::Large);
            }
        });
    }

    /// Shows or hides the panes and status bar as the settings say.
    fn apply_layout(&self) {
        let settings = config::settings();
        self.nav
            .widget
            .set_visible(settings.boolean("show-navigation-pane"));
        let right = settings.string("right-pane");
        self.side.widget.set_visible(right != "none");
        self.side.set_mode(if right == "preview" {
            Mode::Preview
        } else {
            Mode::Details
        });
        if right != "none" {
            let width = settings.int("right-pane-width").max(200);
            let paned = self.side_paned.clone();
            glib::idle_add_local_once(move || {
                let total = paned.width();
                if total > width + 200 {
                    paned.set_position(total - width);
                }
            });
        }
        self.status
            .widget
            .set_visible(settings.boolean("show-status-bar"));
        self.update_view_actions();
        if self.current.borrow().is_some() {
            self.update_selection();
        }
    }

    // --- Keeping the chrome in step --------------------------------------------

    fn update_all(&self) {
        save_open_windows();
        let tab = self.tab();
        let location = tab.location();
        self.address.set_location(&location);
        let (back, forward) = tab.history();
        self.address.set_history(back, forward);
        self.address
            .set_busy(tab.page() == Page::Folder && tab.folder.is_searching());
        self.nav.select_location(&location);
        self.update_title();
        self.update_selection();
        self.update_view_actions();
    }

    fn update_title(&self) {
        let location = self.tab().location();
        let title = if config::flag("show-full-path-in-title") {
            location.edit_text()
        } else {
            location.title()
        };
        self.window
            .set_title(Some(&format!("{title} - File Manager")));
    }

    fn update_selection(&self) {
        let tab = self.tab();
        let selected = tab.selected_infos();
        let n_items = tab.n_items();
        self.status.items.set_label(&if n_items == 1 {
            "1 item".into()
        } else {
            format!("{} items", util::thousands(n_items as u64))
        });
        let selected_text = match selected.len() {
            0 => String::new(),
            count => {
                let files: Vec<&gio::FileInfo> =
                    selected.iter().filter(|info| !util::is_dir(info)).collect();
                let what = if count == 1 {
                    "1 item selected".to_owned()
                } else {
                    format!("{} items selected", util::thousands(count as u64))
                };
                if files.is_empty() || !config::flag("show-size-in-tips") {
                    what
                } else {
                    let total: u64 = files.iter().map(|info| info.size().max(0) as u64).sum();
                    format!("{what}  {}", util::size(total))
                }
            }
        };
        self.status.selected.set_label(&selected_text);
        self.side.show(&tab.location(), &selected, n_items);
        self.update_contextual(&tab, &selected);
        self.update_actions();
    }

    fn update_contextual(&self, tab: &Rc<Tab>, selected: &[gio::FileInfo]) {
        let mut buttons = Vec::new();
        let location = tab.location();
        if location.is_trash() {
            buttons.push(Contextual {
                glyph: "empty-trash",
                label: "Empty Recycle Bin",
                action: "win.empty-trash",
            });
            if selected.is_empty() {
                buttons.push(Contextual {
                    glyph: "restore",
                    label: "Restore all items",
                    action: "win.restore-all",
                });
            } else {
                buttons.push(Contextual {
                    glyph: "restore",
                    label: "Restore the selected items",
                    action: "win.restore-selected",
                });
            }
        } else if let [info] = selected {
            let name = info.name().to_string_lossy().into_owned();
            let mime = util::content_type(info);
            if crate::archive::can_extract(&name) {
                buttons.push(Contextual {
                    glyph: "extract",
                    label: "Extract all",
                    action: "win.extract",
                });
            } else if mime.starts_with("image/") {
                buttons.push(Contextual {
                    glyph: "picture",
                    label: "Set as background",
                    action: "win.set-background",
                });
            }
        }
        if tab.page() == Page::ThisPc {
            buttons.push(Contextual {
                glyph: "network",
                label: "Connect to server",
                action: "win.connect-server",
            });
            let removable = tab
                .this_pc()
                .selected_drives()
                .iter()
                .any(|drive| match drive {
                    crate::thispc::Drive::Mount(mount) => mount.can_eject() || mount.can_unmount(),
                    _ => false,
                });
            if removable {
                buttons.push(Contextual {
                    glyph: "eject",
                    label: "Eject",
                    action: "win.eject",
                });
            }
        }
        self.commandbar.set_contextual(&buttons);
        self.commandbar
            .set_filter_visible(matches!(tab.page(), Page::Folder | Page::Home));
    }

    /// Enables the commands that make sense for the selection and place.
    pub fn update_actions(&self) {
        let Some(tab) = self.current.borrow().clone() else {
            return;
        };
        let location = tab.location();
        let selected = tab.selected_infos();
        let has_selection = !selected.is_empty();
        let in_folder = location.folder().is_some();
        let in_trash = location.is_trash();
        let writable_folder = in_folder
            && !in_trash
            && !matches!(
                location.folder().and_then(places::special_of),
                Some(places::Special::Network)
            );
        let drives_only = selected.iter().all(|info| info.boolean("pfm::drive"));
        let files_selected = has_selection && !drives_only;
        let set = |name: &str, enabled: bool| {
            if let Some(action) = self
                .window
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_enabled(enabled);
            }
        };
        set("back", tab.can_go_back());
        set("forward", tab.can_go_forward());
        set("up", location.parent().is_some());
        set("cut", files_selected && !in_trash);
        set("copy", files_selected);
        set("paste", writable_folder && clipboard::has_files());
        set("paste-shortcut", writable_folder && clipboard::has_files());
        set(
            "rename",
            selected.len() == 1 && !drives_only && !in_trash && tab.page() == Page::Folder,
        );
        set("delete", files_selected);
        set("delete-permanently", files_selected);
        set(
            "share",
            files_selected && selected.iter().all(|info| !util::is_dir(info)),
        );
        set("copy-path", has_selection || in_folder);
        set("new-folder", writable_folder);
        set("new-file", writable_folder);
        set("undo", undo::can_undo());
        set("redo", undo::can_redo());
        set("select-all", tab.n_items() > 0);
        set("select-none", has_selection);
        set(
            "invert-selection",
            tab.page() == Page::Folder && tab.n_items() > 0,
        );
        set("open", has_selection);
        set(
            "open-new-tab",
            has_selection && selected.iter().all(util::is_dir),
        );
        set(
            "open-new-window",
            has_selection && selected.iter().all(util::is_dir),
        );
        set("compress-zip", files_selected && !in_trash);
        set("compress-tar", files_selected && !in_trash);
        set(
            "extract",
            selected.len() == 1
                && crate::archive::can_extract(&selected[0].name().to_string_lossy()),
        );
        set("restore-selected", in_trash && has_selection);
        set("restore-all", in_trash);
        set("empty-trash", true);
        set(
            "open-terminal",
            in_folder && !in_trash || (selected.len() == 1 && util::is_dir(&selected[0])),
        );
        set("create-shortcut", files_selected && !in_trash);
        set(
            "set-background",
            selected.len() == 1 && util::content_type(&selected[0]).starts_with("image/"),
        );
    }

    /// Puts the view, sort and pane states into their actions.
    fn update_view_actions(&self) {
        let Some(tab) = self.current.borrow().clone() else {
            return;
        };
        let settings = config::settings();
        let set_state = |name: &str, state: glib::Variant| {
            if let Some(action) = self
                .window
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_state(&state);
            }
        };
        if let Some(state) = tab.view_state() {
            set_state("view", state.view.id().to_variant());
            set_state("sort", state.sort.id().to_variant());
            set_state("sort-descending", state.descending.to_variant());
            set_state(
                "group",
                state.group.map(Column::id).unwrap_or("none").to_variant(),
            );
            self.status.details.set_active(state.view == View::Details);
            self.status.large.set_active(state.view == View::Large);
        }
        let right = settings.string("right-pane");
        set_state("details-pane", (right == "details").to_variant());
        set_state("preview-pane", (right == "preview").to_variant());
        set_state(
            "navigation-pane",
            settings.boolean("show-navigation-pane").to_variant(),
        );
        set_state("show-hidden", settings.boolean("show-hidden").to_variant());
        set_state(
            "show-extensions",
            (!settings.boolean("hide-extensions")).to_variant(),
        );
        set_state(
            "item-checkboxes",
            settings.boolean("item-checkboxes").to_variant(),
        );
        set_state("compact", settings.boolean("compact-view").to_variant());
        if settings.boolean("compact-view") {
            self.window.add_css_class("compact");
        } else {
            self.window.remove_css_class("compact");
        }
        let sortable = tab.page() == Page::Folder;
        for name in ["view", "sort", "sort-descending", "group"] {
            if let Some(action) = self
                .window
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_enabled(sortable);
            }
        }
    }

    /// Selects these items if they're in the folder on screen.
    pub fn select_files(&self, files: &[gio::File]) {
        let tab = self.tab();
        let Some(folder) = tab.location().folder().cloned() else {
            return;
        };
        let names: Vec<String> = files
            .iter()
            .filter(|file| file.parent().is_some_and(|parent| parent.equal(&folder)))
            .filter_map(|file| {
                file.basename()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect();
        tab.folder.select_when_present(names, false);
    }

    pub fn set_view(&self, view: View) {
        let tab = self.tab();
        if let Some(mut state) = tab.view_state() {
            state.view = view;
            tab.set_view_state(state);
            self.update_view_actions();
        }
    }

    // --- Actions ---------------------------------------------------------------

    fn add_actions(&self) {
        let window = &self.window;
        let simple = |name: &str, run: fn(&Window)| {
            let action = gio::SimpleAction::new(name, None);
            let weak = self.downgrade();
            action.connect_activate(move |_, _| {
                if let Some(window) = weak.upgrade() {
                    run(&window);
                }
            });
            window.add_action(&action);
        };
        simple("back", |w| w.tab().back());
        simple("forward", |w| w.tab().forward());
        simple("up", |w| w.tab().up());
        simple("refresh", |w| w.tab().refresh());
        simple("focus-address", |w| w.address.edit());
        simple("focus-search", |w| w.address.focus_search(None));
        simple("new-tab", |w| {
            w.new_tab(start_location(), true);
        });
        simple("close-tab", |w| {
            if let Some(index) = w.tab_index(&w.tab()) {
                w.close_tab(index);
            }
        });
        simple("new-window", |w| {
            if let Some(app) = w.window.application() {
                Window::new(&app, w.tab().location()).present();
            }
        });
        simple("next-tab", |w| w.cycle_tab(1));
        simple("previous-tab", |w| w.cycle_tab(-1));
        simple("fullscreen", |w| {
            if w.window.is_fullscreen() {
                w.window.unfullscreen();
            } else {
                w.window.fullscreen();
            }
        });
        simple("expand-to-folder", |w| {
            if let Location::Folder(file) = w.tab().location() {
                w.nav.reveal(&file);
            }
        });
        simple("cut", commands::cut);
        simple("copy", commands::copy);
        simple("paste", |w| commands::paste(w, false));
        simple("paste-shortcut", |w| commands::paste(w, true));
        simple("rename", commands::rename);
        simple("delete", |w| commands::delete(w, false));
        simple("delete-permanently", |w| commands::delete(w, true));
        simple("undo", |w| {
            let window = w.clone();
            glib::spawn_future_local(async move {
                let files = undo::undo(Some(window.gtk_window())).await;
                window.select_files(&files);
            });
        });
        simple("redo", |w| {
            let window = w.clone();
            glib::spawn_future_local(async move {
                let files = undo::redo(Some(window.gtk_window())).await;
                window.select_files(&files);
            });
        });
        simple("select-all", |w| w.tab().select_all());
        simple("select-none", |w| w.tab().select_none());
        simple("invert-selection", |w| w.tab().invert_selection());
        simple("copy-path", commands::copy_path);
        simple("share", commands::share);
        simple("properties", commands::properties);
        simple("open", |w| {
            let infos = w.tab().selected_infos();
            commands::open_infos(w, infos, commands::OpenIn::Default);
        });
        simple("open-new-tab", |w| {
            let infos = w.tab().selected_infos();
            commands::open_infos(w, infos, commands::OpenIn::NewTab);
        });
        simple("open-new-window", |w| {
            let infos = w.tab().selected_infos();
            commands::open_infos(w, infos, commands::OpenIn::NewWindow);
        });
        simple("open-with", commands::open_with_dialog);
        simple("open-terminal", commands::open_terminal);
        simple("pin", commands::toggle_pin);
        simple("favorite", commands::toggle_favorite);
        simple("compress-zip", |w| {
            commands::compress(w, crate::archive::Format::Zip)
        });
        simple("compress-tar", |w| {
            commands::compress(w, crate::archive::Format::TarGz)
        });
        simple("extract", commands::extract);
        simple("create-shortcut", commands::create_shortcut);
        simple("new-folder", commands::new_folder);
        simple("empty-trash", |w| {
            let parent = Some(w.gtk_window());
            glib::spawn_future_local(async move { crate::ops::empty_trash(parent).await });
        });
        simple("restore-all", |w| commands::restore(w, true));
        simple("restore-selected", |w| commands::restore(w, false));
        simple("set-background", commands::set_background);
        simple("connect-server", commands::connect_to_server);
        simple("eject", commands::eject);
        simple("options", crate::options::show);
        simple("context-menu", |w| {
            let tab = w.tab();
            let widget: gtk::Widget = tab.content.clone().upcast();
            if tab.selected_infos().is_empty() {
                menus::background_menu(w, &widget, 24.0, 24.0);
            } else {
                menus::item_menu(w, &widget, 24.0, 24.0);
            }
        });

        let new_file = gio::SimpleAction::new("new-file", Some(glib::VariantTy::STRING));
        let weak = self.downgrade();
        new_file.connect_activate(move |_, parameter| {
            let (Some(window), Some(kind)) =
                (weak.upgrade(), parameter.and_then(|p| p.get::<String>()))
            else {
                return;
            };
            commands::new_file(&window, &kind);
        });
        window.add_action(&new_file);

        let tab_number = gio::SimpleAction::new("tab", Some(glib::VariantTy::INT32));
        let weak = self.downgrade();
        tab_number.connect_activate(move |_, parameter| {
            let (Some(window), Some(number)) =
                (weak.upgrade(), parameter.and_then(|p| p.get::<i32>()))
            else {
                return;
            };
            let count = window.tabs.borrow().len();
            // Ctrl+9 is always the last tab, as in browsers.
            let index = if number >= 9 {
                count - 1
            } else {
                (number as usize).saturating_sub(1)
            };
            if index < count {
                window.select_tab(index);
            }
        });
        window.add_action(&tab_number);

        // Stateful: view, sort, grouping, panes and show/hide.
        let stateful_string = |name: &str, initial: &str, apply: fn(&Window, &str)| {
            let action = gio::SimpleAction::new_stateful(
                name,
                Some(glib::VariantTy::STRING),
                &initial.to_variant(),
            );
            let weak = self.downgrade();
            action.connect_activate(move |action, parameter| {
                let (Some(window), Some(value)) =
                    (weak.upgrade(), parameter.and_then(|p| p.get::<String>()))
                else {
                    return;
                };
                action.set_state(&value.to_variant());
                apply(&window, &value);
            });
            window.add_action(&action);
        };
        stateful_string("view", "details", |w, value| {
            if let Some(view) = View::from_id(value) {
                w.set_view(view);
            }
        });
        stateful_string("sort", "name", |w, value| {
            let tab = w.tab();
            if let (Some(mut state), Some(column)) = (tab.view_state(), Column::from_id(value)) {
                state.sort = column;
                tab.set_view_state(state);
            }
        });
        stateful_string("group", "none", |w, value| {
            let tab = w.tab();
            if let Some(mut state) = tab.view_state() {
                state.group = Column::from_id(value);
                tab.set_view_state(state);
            }
        });
        let stateful_bool = |name: &str, apply: fn(&Window, bool)| {
            let action = gio::SimpleAction::new_stateful(name, None, &false.to_variant());
            let weak = self.downgrade();
            action.connect_activate(move |action, _| {
                let Some(window) = weak.upgrade() else { return };
                let value = !action
                    .state()
                    .and_then(|s| s.get::<bool>())
                    .unwrap_or(false);
                action.set_state(&value.to_variant());
                apply(&window, value);
            });
            window.add_action(&action);
        };
        stateful_bool("sort-descending", |w, descending| {
            let tab = w.tab();
            if let Some(mut state) = tab.view_state() {
                state.descending = descending;
                tab.set_view_state(state);
            }
        });
        stateful_bool("details-pane", |_, on| {
            let _ =
                config::settings().set_string("right-pane", if on { "details" } else { "none" });
        });
        stateful_bool("preview-pane", |_, on| {
            let _ =
                config::settings().set_string("right-pane", if on { "preview" } else { "none" });
        });
        stateful_bool("navigation-pane", |_, on| {
            let _ = config::settings().set_boolean("show-navigation-pane", on);
        });
        stateful_bool("show-hidden", |_, on| {
            let _ = config::settings().set_boolean("show-hidden", on);
        });
        stateful_bool("show-extensions", |_, on| {
            let _ = config::settings().set_boolean("hide-extensions", !on);
        });
        stateful_bool("item-checkboxes", |_, on| {
            let _ = config::settings().set_boolean("item-checkboxes", on);
        });
        stateful_bool("compact", |w, on| {
            let _ = config::settings().set_boolean("compact-view", on);
            w.update_view_actions();
        });
    }

    /// Shortcuts that act on the files, not on text fields: they only work
    /// when the focus is in the file list, so typing in the address bar or
    /// search box keeps its own Ctrl+C, Delete and Backspace.
    fn add_shortcuts(&self) {
        let controller = gtk::ShortcutController::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
        let shortcuts: &[(&str, &str)] = &[
            ("Delete|KP_Delete|<Control>d", "win.delete"),
            ("<Shift>Delete|<Shift>KP_Delete", "win.delete-permanently"),
            ("F2", "win.rename"),
            ("<Control>a", "win.select-all"),
            ("<Control>c", "win.copy"),
            ("<Control>x", "win.cut"),
            ("<Control>v", "win.paste"),
            ("<Control>z", "win.undo"),
            ("<Control>y|<Control><Shift>z", "win.redo"),
            ("BackSpace", "win.back"),
            ("<Alt>Return|<Alt>KP_Enter", "win.properties"),
            ("<Control><Shift>c", "win.copy-path"),
        ];
        for (trigger, action) in shortcuts {
            let trigger = gtk::ShortcutTrigger::parse_string(trigger).expect("a valid trigger");
            controller.add_shortcut(gtk::Shortcut::new(
                Some(trigger),
                Some(gtk::NamedAction::new(action)),
            ));
        }
        self.stack.add_controller(controller);
    }
}

impl StatusBar {
    fn new() -> StatusBar {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        widget.add_css_class("status-bar");
        let items = gtk::Label::builder().xalign(0.0).build();
        let selected = gtk::Label::builder().xalign(0.0).build();
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        let toggle = |glyph: &str, tooltip: &str| {
            let button = gtk::ToggleButton::builder()
                .child(&Glyph::new(glyph))
                .tooltip_text(tooltip)
                .build();
            button.add_css_class("flat");
            button.add_css_class("status-toggle");
            button
        };
        let details = toggle(
            "view-details",
            "Display information about each item in the window. (Ctrl+Shift+6)",
        );
        let large = toggle(
            "view-large",
            "Display items by using large thumbnails. (Ctrl+Shift+2)",
        );
        let toggles = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        toggles.append(&details);
        toggles.append(&large);
        widget.append(&items);
        widget.append(&selected);
        widget.append(&spacer);
        widget.append(&toggles);
        StatusBar {
            widget,
            items,
            selected,
            details,
            large,
        }
    }
}

/// The accelerators for actions that work wherever the focus is.
pub const ACCELS: &[(&str, &[&str])] = &[
    ("win.new-tab", &["<Control>t"]),
    ("win.close-tab", &["<Control>w", "<Control>F4"]),
    ("win.new-window", &["<Control>n"]),
    ("win.new-folder", &["<Control><Shift>n"]),
    ("win.back", &["<Alt>Left", "Back"]),
    ("win.forward", &["<Alt>Right", "Forward"]),
    ("win.up", &["<Alt>Up"]),
    ("win.refresh", &["F5", "<Control>r"]),
    ("win.focus-address", &["<Control>l", "<Alt>d", "F4"]),
    ("win.focus-search", &["<Control>e", "<Control>f", "F3"]),
    ("win.next-tab", &["<Control>Tab", "<Control>Page_Down"]),
    (
        "win.previous-tab",
        &["<Control><Shift>Tab", "<Control>Page_Up"],
    ),
    ("win.fullscreen", &["F11"]),
    ("win.expand-to-folder", &["<Control><Shift>e"]),
    ("win.details-pane", &["<Alt><Shift>p"]),
    ("win.preview-pane", &["<Alt>p"]),
    ("win.navigation-pane", &[]),
    ("win.options", &[]),
];

/// Ctrl+1 to 9 pick a tab; Ctrl+Shift+1 to 8 pick a view.
pub fn set_accels(app: &gtk::Application) {
    for (action, accels) in ACCELS {
        app.set_accels_for_action(action, accels);
    }
    for number in 1..=9 {
        app.set_accels_for_action(
            &format!("win.tab({number})"),
            &[&format!("<Control>{number}")],
        );
    }
    for (index, view) in VIEWS.iter().enumerate() {
        app.set_accels_for_action(
            &format!("win.view('{}')", view.id()),
            &[&format!("<Control><Shift>{}", index + 1)],
        );
    }
}
