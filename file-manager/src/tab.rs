//! One tab: a place with its own back and forward history, shown by the
//! folder view or by Home, This PC or Gallery.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk::{gio, glib, prelude::*};

use crate::dialogs;
use crate::folderview::{FolderView, ViewEvent};
use crate::gallery::GalleryPage;
use crate::home::HomePage;
use crate::icons::Kind;
use crate::location::Location;
use crate::places;
use crate::thispc::ThisPcPage;
use crate::viewstate::{self, ViewState};

type TabHandler = dyn Fn(&Rc<Tab>, TabEvent);

pub enum TabEvent {
    View(ViewEvent),
    LocationChanged,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Folder,
    Home,
    ThisPc,
    Gallery,
}

const HISTORY_LIMIT: usize = 50;

pub struct Tab {
    pub content: gtk::Stack,
    pub folder: Rc<FolderView>,
    home: RefCell<Option<Rc<HomePage>>>,
    this_pc: RefCell<Option<Rc<ThisPcPage>>>,
    gallery: RefCell<Option<Rc<GalleryPage>>>,
    location: RefCell<Location>,
    back: RefCell<Vec<Location>>,
    forward: RefCell<Vec<Location>>,
    /// Bumped on each navigation, so a slow mount can't land late.
    generation: Cell<u64>,
    on_event: RefCell<Option<Rc<TabHandler>>>,
    me: RefCell<Weak<Tab>>,
}

impl Tab {
    pub fn new() -> Rc<Tab> {
        let content = gtk::Stack::builder()
            .hexpand(true)
            .vexpand(true)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .build();
        content.add_css_class("tab-content");
        let folder = FolderView::new();
        content.add_named(&folder.widget, Some("folder"));
        let tab = Rc::new(Tab {
            content,
            folder,
            home: RefCell::default(),
            this_pc: RefCell::default(),
            gallery: RefCell::default(),
            location: RefCell::new(Location::Home),
            back: RefCell::default(),
            forward: RefCell::default(),
            generation: Cell::new(0),
            on_event: RefCell::default(),
            me: RefCell::default(),
        });
        *tab.me.borrow_mut() = Rc::downgrade(&tab);
        let weak = Rc::downgrade(&tab);
        tab.folder.connect_events(move |event| {
            if let Some(tab) = weak.upgrade() {
                tab.emit(TabEvent::View(event));
            }
        });
        tab
    }

    pub fn connect_events(&self, handler: impl Fn(&Rc<Tab>, TabEvent) + 'static) {
        *self.on_event.borrow_mut() = Some(Rc::new(handler));
    }

    fn emit(&self, event: TabEvent) {
        let handler = self.on_event.borrow().clone();
        let me = self.me.borrow().upgrade();
        if let (Some(handler), Some(me)) = (handler, me) {
            handler(&me, event);
        }
    }

    fn forward_events(&self) -> impl Fn(ViewEvent) + 'static {
        let weak = self.me.borrow().clone();
        move |event| {
            if let Some(tab) = weak.upgrade() {
                tab.emit(TabEvent::View(event));
            }
        }
    }

    pub fn home(&self) -> Rc<HomePage> {
        self.home
            .borrow_mut()
            .get_or_insert_with(|| {
                let page = HomePage::new();
                page.connect_events(self.forward_events());
                self.content.add_named(&page.widget, Some("home"));
                page
            })
            .clone()
    }

    pub fn this_pc(&self) -> Rc<ThisPcPage> {
        self.this_pc
            .borrow_mut()
            .get_or_insert_with(|| {
                let page = ThisPcPage::new();
                page.connect_events(self.forward_events());
                self.content.add_named(&page.widget, Some("this-pc"));
                page
            })
            .clone()
    }

    pub fn gallery(&self) -> Rc<GalleryPage> {
        self.gallery
            .borrow_mut()
            .get_or_insert_with(|| {
                let page = GalleryPage::new();
                page.connect_events(self.forward_events());
                self.content.add_named(&page.widget, Some("gallery"));
                page
            })
            .clone()
    }

    pub fn location(&self) -> Location {
        self.location.borrow().clone()
    }

    pub fn page(&self) -> Page {
        match &*self.location.borrow() {
            Location::Home => Page::Home,
            Location::ThisPc => Page::ThisPc,
            Location::Gallery => Page::Gallery,
            _ => Page::Folder,
        }
    }

    /// Goes somewhere new; Back returns here.
    pub fn navigate(&self, location: Location) {
        let current = self.location();
        if current.same(&location) && self.generation.get() > 0 {
            self.refresh();
            return;
        }
        if self.generation.get() > 0 && !matches!(current, Location::Search { .. }) {
            let mut back = self.back.borrow_mut();
            back.push(current);
            if back.len() > HISTORY_LIMIT {
                back.remove(0);
            }
        }
        self.forward.borrow_mut().clear();
        self.show(location);
    }

    /// Shows a place without touching the history.
    fn show(&self, location: Location) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        if let Location::Folder(file) = &location
            && needs_mount_check(file)
        {
            self.mount_then_show(location.clone(), generation);
            return;
        }
        self.display(location);
    }

    fn display(&self, location: Location) {
        let previous = self.location.replace(location.clone());
        if !matches!(previous, Location::Folder(_) | Location::Search { .. })
            || !matches!(location, Location::Folder(_) | Location::Search { .. })
        {
            self.folder.select_none();
        }
        match &location {
            Location::Home => {
                let home = self.home();
                home.reload();
                self.content.set_visible_child_name("home");
                self.folder.release();
            }
            Location::ThisPc => {
                let page = self.this_pc();
                page.reload();
                self.content.set_visible_child_name("this-pc");
                self.folder.release();
            }
            Location::Gallery => {
                self.gallery();
                self.content.set_visible_child_name("gallery");
                self.folder.release();
            }
            Location::Folder(file) => {
                self.folder
                    .show_folder(&location, ViewState::for_location(&location));
                self.content.set_visible_child_name("folder");
                viewstate::record_visit(file);
            }
            Location::Search { .. } => {
                self.folder.show_search(&location);
                self.content.set_visible_child_name("folder");
            }
        }
        self.emit(TabEvent::LocationChanged);
    }

    /// Network places must be mounted before they can be listed.
    fn mount_then_show(&self, location: Location, generation: u64) {
        let Location::Folder(file) = &location else {
            return;
        };
        let weak = self.me.borrow().clone();
        let file = file.clone();
        glib::spawn_future_local(async move {
            let result = file
                .query_info_future(
                    "standard::type",
                    gio::FileQueryInfoFlags::NONE,
                    glib::Priority::DEFAULT,
                )
                .await;
            let Some(tab) = weak.upgrade() else { return };
            if tab.generation.get() != generation {
                return;
            }
            match result {
                Err(error) if error.matches(gio::IOErrorEnum::NotMounted) => {
                    let window = tab.content.root().and_downcast::<gtk::Window>();
                    let operation = gtk::MountOperation::new(window.as_ref());
                    let weak = Rc::downgrade(&tab);
                    let target = file.clone();
                    file.mount_enclosing_volume(
                        gio::MountMountFlags::NONE,
                        Some(&operation),
                        gio::Cancellable::NONE,
                        move |result| {
                            let Some(tab) = weak.upgrade() else { return };
                            if tab.generation.get() != generation {
                                return;
                            }
                            match result {
                                Ok(()) => tab.display(Location::Folder(target)),
                                Err(error) if error.matches(gio::IOErrorEnum::FailedHandled) => {
                                    tab.cancel_navigation()
                                }
                                Err(error) => {
                                    let window = tab.content.root().and_downcast::<gtk::Window>();
                                    dialogs::error(
                                        window.as_ref(),
                                        "Network Error",
                                        &format!("Can't open {}.", target.parse_name()),
                                        &dialogs::describe(&error),
                                    );
                                    tab.cancel_navigation();
                                }
                            }
                        },
                    );
                }
                _ => tab.display(location),
            }
        });
    }

    /// Undoes a navigation that didn't work out.
    fn cancel_navigation(&self) {
        let previous = self.back.borrow_mut().pop();
        if let Some(previous) = previous {
            self.display(previous);
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.borrow().is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.borrow().is_empty()
    }

    pub fn back(&self) {
        let Some(previous) = self.back.borrow_mut().pop() else {
            return;
        };
        let current = self.location();
        if !matches!(current, Location::Search { .. }) {
            self.forward.borrow_mut().push(current);
        }
        self.show(previous);
    }

    pub fn forward(&self) {
        let Some(next) = self.forward.borrow_mut().pop() else {
            return;
        };
        self.back.borrow_mut().push(self.location());
        self.show(next);
    }

    pub fn up(&self) {
        if let Some(parent) = self.location().parent() {
            self.navigate(parent);
        }
    }

    /// The places Back and Forward lead to, nearest first, for the address
    /// bar's "Recent locations".
    pub fn history(&self) -> (Vec<Location>, Vec<Location>) {
        let mut back = self.back.borrow().clone();
        back.reverse();
        let mut forward = self.forward.borrow().clone();
        forward.reverse();
        (back, forward)
    }

    /// Jumps `steps` back (negative) or forward (positive) in history.
    pub fn go_history(&self, steps: i32) {
        for _ in 0..steps.unsigned_abs() {
            if steps < 0 {
                let Some(previous) = self.back.borrow_mut().pop() else {
                    break;
                };
                self.forward
                    .borrow_mut()
                    .push(self.location.replace(previous));
            } else {
                let Some(next) = self.forward.borrow_mut().pop() else {
                    break;
                };
                self.back.borrow_mut().push(self.location.replace(next));
            }
        }
        self.show(self.location());
    }

    pub fn refresh(&self) {
        match self.page() {
            Page::Folder => self.folder.refresh(),
            Page::Home => self.home().reload(),
            Page::ThisPc => self.this_pc().reload(),
            Page::Gallery => self.gallery().reload(),
        }
        self.emit(TabEvent::LocationChanged);
    }

    pub fn selected_infos(&self) -> Vec<gio::FileInfo> {
        match self.page() {
            Page::Folder => self.folder.selected_infos(),
            Page::Home => self.home().selected_infos(),
            Page::ThisPc => self.this_pc().selected_infos(),
            Page::Gallery => self.gallery().selected_infos(),
        }
    }

    pub fn selected_files(&self) -> Vec<gio::File> {
        self.selected_infos()
            .iter()
            .filter_map(crate::util::file_of)
            .collect()
    }

    pub fn n_items(&self) -> u32 {
        match self.page() {
            Page::Folder => self.folder.n_items(),
            Page::Home => self.home().n_items(),
            Page::ThisPc => self.this_pc().n_items(),
            Page::Gallery => self.gallery().n_items(),
        }
    }

    pub fn select_all(&self) {
        match self.page() {
            Page::Folder => self.folder.select_all(),
            Page::Home => self.home().select_all(),
            Page::ThisPc => self.this_pc().select_all(),
            Page::Gallery => self.gallery().select_all(),
        }
    }

    pub fn select_none(&self) {
        match self.page() {
            Page::Folder => self.folder.select_none(),
            Page::Home => self.home().select_none(),
            Page::ThisPc => self.this_pc().select_none(),
            Page::Gallery => self.gallery().select_none(),
        }
    }

    pub fn invert_selection(&self) {
        if self.page() == Page::Folder {
            self.folder.invert_selection();
        }
    }

    pub fn view_state(&self) -> Option<ViewState> {
        (self.page() == Page::Folder).then(|| self.folder.state())
    }

    pub fn set_view_state(&self, state: ViewState) {
        if self.page() == Page::Folder {
            self.folder.set_state(state, true);
        }
    }

    pub fn kind_filter(&self) -> Vec<Kind> {
        match self.page() {
            Page::Home => self.home().kind_filter(),
            _ => self.folder.kind_filter(),
        }
    }

    pub fn set_kind_filter(&self, kinds: Vec<Kind>) {
        match self.page() {
            Page::Home => self.home().set_kind_filter(kinds),
            _ => self.folder.set_kind_filter(kinds),
        }
    }

    pub fn focus(&self) {
        match self.page() {
            Page::Folder => self.folder.focus(),
            Page::Home => self.home().focus(),
            Page::ThisPc => self.this_pc().focus(),
            Page::Gallery => self.gallery().focus(),
        }
    }

    /// Stops watching anything, when the tab closes.
    pub fn release(&self) {
        self.folder.release();
    }
}

/// Whether a folder may need mounting before it can be read.
fn needs_mount_check(file: &gio::File) -> bool {
    !file.is_native()
        && !places::is_trash(file)
        && !matches!(
            file.uri_scheme().as_deref(),
            Some("network" | "recent" | "computer")
        )
}
