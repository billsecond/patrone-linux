//! The file list of a folder, the Recycle Bin or search results, in any of
//! Windows' eight views.
//!
//! Items flow from a `GtkDirectoryList` (or the search results) through a
//! filter (hidden files, the Filter menu), a sorter (folders first, names
//! in natural order) and, when grouping, a section sorter, into one
//! selection model that every view shares, so switching views keeps the
//! selection.

use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::rc::{Rc, Weak};

use gtk::{gdk, gio, glib, prelude::*};

use crate::config;
use crate::desktopgrid::DesktopGrid;
use crate::filecell::{self, CellContext, CellKind, FileCell};
use crate::icons::{self, Kind};
use crate::location::Location;
use crate::places;
use crate::search;
use crate::util;
use crate::viewstate::{self, Column, View, ViewState};

/// What every list asks GIO for.
pub const ATTRIBUTES: &str = "standard::*,time::*,unix::mode,unix::uid,owner::user,owner::group,\
thumbnail::path,thumbnail::is-valid,trash::*,access::*,id::filesystem,filesystem::readonly";

/// What happens in a view that the tab or window handles.
pub enum ViewEvent {
    /// Open these items (double-click, or Enter).
    Activate(Vec<gio::FileInfo>),
    /// A right-click, on an item (now selected) or on the background.
    ContextMenu {
        on_item: bool,
        widget: gtk::Widget,
        x: f64,
        y: f64,
    },
    SelectionChanged,
    ItemsChanged,
    Drop {
        files: Vec<gio::File>,
        folder: gio::File,
        action: gdk::DragAction,
    },
    Rename {
        file: gio::File,
        name: String,
    },
    /// The view, sort or grouping changed (from a column header or Ctrl+wheel).
    StateChanged,
    /// Typing with "type into the search box" on.
    TypeToSearch(String),
    SearchRunning(bool),
}

struct FilterState {
    show_hidden: Cell<bool>,
    kinds: RefCell<Vec<Kind>>,
    /// The folder shown. A directory monitor can report the folder itself
    /// (or, while switching, a neighbor); only its children belong here.
    folder: RefCell<Option<gio::File>>,
}

struct Pending {
    names: Vec<String>,
    rename: bool,
    /// When the items first showed up. GIO often reports a new file twice
    /// (created, then changed), and the second report replaces the item and
    /// drops its selection, so the selection is put back for a moment.
    found: Option<std::time::Instant>,
    /// Whether the view has been scrolled to the items yet.
    scrolled: bool,
}

pub struct FolderView {
    pub widget: gtk::Overlay,
    scrolled: gtk::ScrolledWindow,
    empty: gtk::Label,
    banner: crate::hints::Banner,
    dir: gtk::DirectoryList,
    results: gio::ListStore,
    filter_state: Rc<FilterState>,
    filter: gtk::CustomFilter,
    filter_model: gtk::FilterListModel,
    state: Rc<Cell<ViewState>>,
    sorter: gtk::CustomSorter,
    group_sorter: gtk::CustomSorter,
    sort_model: gtk::SortListModel,
    pub selection: gtk::MultiSelection,
    pub context: Rc<CellContext>,
    location: RefCell<Option<Location>>,
    view: RefCell<Option<gtk::Widget>>,
    built: Cell<Option<(View, Option<Column>, bool)>>,
    pending: RefCell<Option<Pending>>,
    search: RefCell<Option<gio::Cancellable>>,
    searching: Cell<bool>,
    typed: RefCell<String>,
    typed_timer: RefCell<Option<glib::SourceId>>,
    events: crate::util::Handler<ViewEvent>,
    /// Showing the desktop (desktop.rs): the desktop's own grid and sort,
    /// no banner or "This folder is empty."
    desktop: Cell<bool>,
    desktop_grid: RefCell<Option<DesktopGrid>>,
    me: RefCell<Weak<FolderView>>,
}

impl FolderView {
    pub fn new() -> Rc<FolderView> {
        let dir = gtk::DirectoryList::new(Some(ATTRIBUTES), None::<&gio::File>);
        dir.set_monitored(true);
        dir.set_io_priority(glib::Priority::DEFAULT_IDLE);
        let results = gio::ListStore::new::<gio::FileInfo>();

        let filter_state = Rc::new(FilterState {
            show_hidden: Cell::new(config::flag("show-hidden")),
            kinds: RefCell::default(),
            folder: RefCell::default(),
        });
        let filter = {
            let filter_state = filter_state.clone();
            gtk::CustomFilter::new(move |item| {
                let Some(info) = item.downcast_ref::<gio::FileInfo>() else {
                    return false;
                };
                if !filter_state.show_hidden.get() && (info.is_hidden() || info.is_backup()) {
                    return false;
                }
                if let Some(folder) = filter_state.folder.borrow().as_ref() {
                    let inside = util::file_of(info)
                        .and_then(|file| file.parent())
                        .is_some_and(|parent| parent.equal(folder));
                    if !inside {
                        return false;
                    }
                }
                let kinds = filter_state.kinds.borrow();
                kinds.is_empty()
                    || kinds.contains(&icons::kind_of(
                        &util::content_type(info),
                        util::is_dir(info),
                    ))
            })
        };
        let filter_model = gtk::FilterListModel::new(Some(dir.clone()), Some(filter.clone()));

        let state = Rc::new(Cell::new(ViewState::default_for(&Location::Home)));
        let sorter = {
            let state = state.clone();
            gtk::CustomSorter::new(move |a, b| {
                let (Some(a), Some(b)) = (
                    a.downcast_ref::<gio::FileInfo>(),
                    b.downcast_ref::<gio::FileInfo>(),
                ) else {
                    return gtk::Ordering::Equal;
                };
                compare(a, b, &state.get()).into()
            })
        };
        let group_sorter = {
            let state = state.clone();
            gtk::CustomSorter::new(move |a, b| {
                let state = state.get();
                let Some(column) = state.group else {
                    return gtk::Ordering::Equal;
                };
                let (Some(a), Some(b)) = (
                    a.downcast_ref::<gio::FileInfo>(),
                    b.downcast_ref::<gio::FileInfo>(),
                ) else {
                    return gtk::Ordering::Equal;
                };
                let order = group_key(a, column).cmp(&group_key(b, column));
                if state.descending && state.sort == column {
                    order.reverse()
                } else {
                    order
                }
                .into()
            })
        };
        let sort_model = gtk::SortListModel::new(Some(filter_model.clone()), Some(sorter.clone()));
        let selection = gtk::MultiSelection::new(Some(sort_model.clone()));
        let context = CellContext::new(&selection);

        let scrolled = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .build();
        scrolled.add_css_class("file-list");
        // In the middle of the list, a little above center.
        let empty = gtk::Label::builder()
            .label("This folder is empty.")
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .margin_bottom(64)
            .visible(false)
            .can_target(false)
            .build();
        empty.add_css_class("empty-folder");
        let banner = crate::hints::Banner::new();
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.append(&banner.widget);
        body.append(&scrolled);
        let widget = gtk::Overlay::builder().child(&body).build();
        widget.add_overlay(&empty);

        let view = Rc::new(FolderView {
            widget,
            scrolled,
            empty,
            banner,
            dir,
            results,
            filter_state,
            filter,
            filter_model,
            state,
            sorter,
            group_sorter,
            sort_model,
            selection,
            context,
            location: RefCell::default(),
            view: RefCell::default(),
            built: Cell::new(None),
            pending: RefCell::default(),
            search: RefCell::default(),
            searching: Cell::new(false),
            typed: RefCell::default(),
            typed_timer: RefCell::default(),
            events: RefCell::default(),
            desktop: Cell::new(false),
            desktop_grid: RefCell::default(),
            me: RefCell::default(),
        });
        *view.me.borrow_mut() = Rc::downgrade(&view);
        view.connect_models();
        view.add_background_drop();
        view.apply_settings();
        view
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

    fn weak(&self) -> Weak<FolderView> {
        self.me.borrow().clone()
    }

    /// Makes this the desktop's view, before it shows the Desktop folder.
    pub fn set_desktop_mode(&self) {
        self.desktop.set(true);
        self.context.launchers.set(true);
        self.dir
            .set_attributes(Some(&format!("{ATTRIBUTES},metadata::trusted")));
        self.banner.widget.set_visible(false);
        self.scrolled
            .set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
    }

    /// The desktop's grid, in desktop mode.
    pub fn desktop_grid(&self) -> Option<DesktopGrid> {
        self.desktop_grid.borrow().clone()
    }

    fn connect_models(&self) {
        let weak = self.weak();
        self.sort_model
            .connect_items_changed(move |_, _, _, added| {
                if let Some(view) = weak.upgrade() {
                    view.update_empty();
                    if added > 0 {
                        view.check_pending();
                    }
                    view.emit(ViewEvent::ItemsChanged);
                }
            });
        let weak = self.weak();
        self.dir.connect_loading_notify(move |dir| {
            if let Some(view) = weak.upgrade() {
                view.update_empty();
                if !dir.is_loading() {
                    view.check_pending();
                }
                view.emit(ViewEvent::ItemsChanged);
            }
        });
        let weak = self.weak();
        self.dir.connect_error_notify(move |_| {
            if let Some(view) = weak.upgrade() {
                view.update_empty();
            }
        });
        let weak = self.weak();
        self.selection.connect_selection_changed(move |_, _, _| {
            if let Some(view) = weak.upgrade() {
                view.emit(ViewEvent::SelectionChanged);
            }
        });
        let weak = self.weak();
        *self.context.on_drop.borrow_mut() = Some(Rc::new(move |files, folder, action| {
            if let Some(view) = weak.upgrade() {
                view.emit(ViewEvent::Drop {
                    files,
                    folder,
                    action,
                });
            }
        }));
    }

    /// Shows a folder (or the Recycle Bin), in its remembered view.
    pub fn show_folder(&self, location: &Location, state: ViewState) {
        let Location::Folder(file) = location else {
            return;
        };
        self.stop_search();
        crate::thumbs::cancel_pending();
        self.pending.take();
        self.context.in_trash.set(places::is_trash(file));
        self.location.replace(Some(location.clone()));
        if self.filter_model.model().as_ref() != Some(self.dir.upcast_ref::<gio::ListModel>()) {
            self.filter_model.set_model(Some(&self.dir));
        }
        self.filter_state.folder.replace(Some(file.clone()));
        self.dir.set_file(Some(file));
        if self.desktop.get() {
            if let Some(grid) = self.desktop_grid() {
                grid.set_folder(file);
            }
            self.set_state(self.state.get(), false);
            self.update_empty();
            return;
        }
        self.banner.show(Some(file));
        self.empty.set_label("This folder is empty.");
        self.set_state(state, false);
        self.update_empty();
        self.scroll_to_top();
    }

    /// Searches from `root` and shows what's found as it's found.
    pub fn show_search(&self, location: &Location) {
        let Location::Search { root, query } = location else {
            return;
        };
        self.stop_search();
        crate::thumbs::cancel_pending();
        self.pending.take();
        self.context.in_trash.set(false);
        self.location.replace(Some(location.clone()));
        self.results.remove_all();
        self.banner.show(None);
        self.filter_state.folder.replace(None);
        self.filter_model.set_model(Some(&self.results));
        self.dir.set_file(None::<&gio::File>);
        self.empty.set_label("Searching…");
        self.set_state(ViewState::for_location(location), false);
        let cancellable = gio::Cancellable::new();
        self.search.replace(Some(cancellable.clone()));
        self.searching.set(true);
        self.emit(ViewEvent::SearchRunning(true));
        self.update_empty();
        let weak = self.weak();
        let results = self.results.clone();
        let root = root.clone();
        let query = query.clone();
        glib::spawn_future_local(async move {
            search::run(
                root,
                query,
                results,
                search::Options::from_settings(),
                cancellable.clone(),
            )
            .await;
            if cancellable.is_cancelled() {
                return;
            }
            if let Some(view) = weak.upgrade() {
                view.searching.set(false);
                view.empty.set_label("No items match your search.");
                view.update_empty();
                view.emit(ViewEvent::SearchRunning(false));
                view.emit(ViewEvent::ItemsChanged);
            }
        });
    }

    fn stop_search(&self) {
        if let Some(cancellable) = self.search.take() {
            cancellable.cancel();
        }
        if self.searching.replace(false) {
            self.emit(ViewEvent::SearchRunning(false));
        }
    }

    /// Stops loading and watching, when the tab leaves this view.
    pub fn release(&self) {
        self.stop_search();
        self.dir.set_file(None::<&gio::File>);
        self.location.replace(None);
    }

    pub fn is_searching(&self) -> bool {
        self.searching.get()
    }

    pub fn state(&self) -> ViewState {
        self.state.get()
    }

    /// Changes the view, sort or grouping. `remember` saves it for the folder.
    pub fn set_state(&self, state: ViewState, remember: bool) {
        let old = self.state.replace(state);
        if old.sort != state.sort || old.descending != state.descending {
            self.sorter.changed(gtk::SorterChange::Different);
        }
        if old.group != state.group || old.descending != state.descending {
            if state.group.is_some() {
                self.sort_model.set_section_sorter(Some(&self.group_sorter));
                self.group_sorter.changed(gtk::SorterChange::Different);
            } else {
                self.sort_model.set_section_sorter(None::<&gtk::Sorter>);
            }
        } else if state.group.is_some() && self.sort_model.section_sorter().is_none() {
            self.sort_model.set_section_sorter(Some(&self.group_sorter));
        }
        let in_trash = self.context.in_trash.get();
        if self.built.get() != Some((state.view, state.group, in_trash)) {
            self.build_view();
        } else if let Some(columns) = self
            .view
            .borrow()
            .as_ref()
            .and_then(|w| w.downcast_ref::<gtk::ColumnView>().cloned())
        {
            self.show_sort_in_headers(&columns);
        }
        if remember && let Some(location) = self.location.borrow().as_ref() {
            viewstate::remember(location, &state);
        }
    }

    /// Reads Folder Options again: hidden files, extensions, single-click,
    /// check boxes, tooltips.
    pub fn apply_settings(&self) {
        let show_hidden = config::flag("show-hidden");
        if self.filter_state.show_hidden.replace(show_hidden) != show_hidden {
            self.filter.changed(gtk::FilterChange::Different);
        }
        self.context
            .hide_extensions
            .set(config::flag("hide-extensions"));
        self.context.tooltips.set(config::flag("show-tooltips"));
        self.context.checkboxes.set(config::flag("item-checkboxes"));
        let hints = crate::hints::enabled();
        if self.context.hints.replace(hints) != hints && self.view.borrow().is_some() {
            // Details gains or loses its Purpose column.
            self.built.set(None);
            self.set_state(self.state.get(), false);
        }
        let folder = match self.location.borrow().as_ref() {
            Some(Location::Folder(file)) => Some(file.clone()),
            _ => None,
        };
        if !self.desktop.get() {
            self.banner.show(folder.as_ref());
        }
        crate::thumbs::set_enabled(config::flag("show-thumbnails"));
        if let Some(view) = self.view.borrow().as_ref() {
            set_single_click(view, config::flag("single-click"));
        }
        let underline =
            config::flag("single-click") && config::string("underline-titles") == "always";
        if underline {
            self.widget.add_css_class("underline-titles");
        } else {
            self.widget.remove_css_class("underline-titles");
        }
        if config::flag("single-click") {
            self.widget.add_css_class("single-click");
        } else {
            self.widget.remove_css_class("single-click");
        }
        self.rebind();
    }

    /// Redraws every visible item, after a setting or the cut items change.
    pub fn rebind(&self) {
        let cells: Vec<FileCell> = self
            .context
            .cells
            .borrow()
            .iter()
            .filter_map(|weak| weak.upgrade())
            .collect();
        for cell in cells {
            if let Some(info) = cell.info().filter(|_| cell.is_bound()) {
                let position = cell.position();
                cell.bind(&info, position, self.selection.is_selected(position));
            }
        }
        self.context
            .cells
            .borrow_mut()
            .retain(|weak| weak.upgrade().is_some());
    }

    /// The Filter menu: only these kinds of items (none means all).
    pub fn set_kind_filter(&self, kinds: Vec<Kind>) {
        self.filter_state.kinds.replace(kinds);
        self.filter.changed(gtk::FilterChange::Different);
    }

    pub fn kind_filter(&self) -> Vec<Kind> {
        self.filter_state.kinds.borrow().clone()
    }

    fn update_empty(&self) {
        if self.desktop.get() {
            self.empty.set_visible(false);
            return;
        }
        let loading = self.dir.is_loading()
            && self.filter_model.model().as_ref() == Some(self.dir.upcast_ref());
        let empty = self.filter_model.n_items() == 0;
        if let Some(error) = self
            .dir
            .error()
            .filter(|_| self.filter_model.model().as_ref() == Some(self.dir.upcast_ref()))
        {
            let text = if error.matches(gio::IOErrorEnum::PermissionDenied) {
                "You don't currently have permission to access this folder.".to_owned()
            } else if error.matches(gio::IOErrorEnum::NotFound) {
                "This folder no longer exists.".to_owned()
            } else {
                error.message().to_owned()
            };
            self.empty.set_label(&text);
            self.empty.set_visible(true);
            return;
        }
        if self.searching.get() {
            self.empty.set_label("Searching…");
        } else if matches!(
            self.location.borrow().as_ref(),
            Some(Location::Search { .. })
        ) {
            self.empty.set_label("No items match your search.");
        } else if !self.filter_state.kinds.borrow().is_empty() {
            self.empty.set_label("No items match the filter.");
        } else {
            self.empty.set_label("This folder is empty.");
        }
        self.empty.set_visible(empty && !loading);
    }

    // --- Selection -------------------------------------------------------

    pub fn n_items(&self) -> u32 {
        self.sort_model.n_items()
    }

    pub fn selected_positions(&self) -> Vec<u32> {
        let selected = self.selection.selection();
        (0..selected.size())
            .map(|index| selected.nth(index as u32))
            .collect()
    }

    pub fn selected_infos(&self) -> Vec<gio::FileInfo> {
        self.selected_positions()
            .into_iter()
            .filter_map(|position| {
                self.selection
                    .item(position)
                    .and_downcast::<gio::FileInfo>()
            })
            .collect()
    }

    pub fn select_all(&self) {
        self.selection.select_all();
    }

    pub fn select_none(&self) {
        self.selection.unselect_all();
    }

    pub fn invert_selection(&self) {
        let n = self.n_items();
        let selected = self.selection.selection();
        let inverted = gtk::Bitset::new_range(0, n);
        inverted.difference(&selected);
        self.selection
            .set_selection(&inverted, &gtk::Bitset::new_range(0, n));
    }

    /// Selects the items with these names as soon as they appear (after a
    /// paste, a new folder or a rename), then renames the one if asked.
    pub fn select_when_present(&self, names: Vec<String>, rename: bool) {
        if names.is_empty() {
            return;
        }
        self.pending.replace(Some(Pending {
            names,
            rename,
            found: None,
            scrolled: false,
        }));
        self.check_pending();
        let weak = self.weak();
        glib::timeout_add_local_once(std::time::Duration::from_secs(5), move || {
            if let Some(view) = weak.upgrade() {
                view.pending.take();
            }
        });
    }

    fn check_pending(&self) {
        let (names, rename, found, scrolled) = match self.pending.borrow().as_ref() {
            Some(pending) => (
                pending.names.clone(),
                pending.rename,
                pending.found,
                pending.scrolled,
            ),
            None => return,
        };
        if scrolled && found.is_some_and(|at| at.elapsed() > std::time::Duration::from_millis(1500))
        {
            self.pending.take();
            return;
        }
        let positions = self.positions_of(&names);
        if positions.len() < names.len() {
            return;
        }
        let first_time = found.is_none();
        if let Some(pending) = self.pending.borrow_mut().as_mut() {
            pending.found.get_or_insert_with(std::time::Instant::now);
        }
        let already = positions
            .iter()
            .all(|position| self.selection.is_selected(*position))
            && self.selected_positions().len() == positions.len();
        if !already {
            self.selection.unselect_all();
            for position in &positions {
                self.selection.select_item(*position, false);
            }
        }
        // Items still loading in above these would push them out of view,
        // so the scroll waits for the folder to finish.
        if !scrolled && !self.dir.is_loading() {
            if let Some(pending) = self.pending.borrow_mut().as_mut() {
                pending.scrolled = true;
            }
            self.scroll_when_laid_out(names.clone());
        }
        if first_time && rename {
            // After GIO's follow-up report, so the editor isn't reset.
            let weak = self.weak();
            glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
                if let Some(view) = weak.upgrade() {
                    view.start_rename();
                }
            });
        }
    }

    /// Scrolls to the first of these items (and gives it the keyboard focus)
    /// once the rows that just arrived are laid out. Before that they have
    /// no height, and GTK stops a row short or puts the item at the top
    /// with the ones above it out of sight. A tick callback runs as each
    /// frame starts, so on the second one the first layout is done.
    fn scroll_when_laid_out(&self, names: Vec<String>) {
        let Some(widget) = self.view.borrow().clone() else {
            return;
        };
        let weak = self.weak();
        let frames = Cell::new(0);
        widget.add_tick_callback(move |_, _| {
            frames.set(frames.get() + 1);
            if frames.get() < 2 {
                return glib::ControlFlow::Continue;
            }
            if let Some(view) = weak.upgrade()
                && let Some(position) = view.positions_of(&names).first()
            {
                view.scroll_to(*position, true);
            }
            glib::ControlFlow::Break
        });
    }

    /// Where the items with these names are, in view order.
    fn positions_of(&self, names: &[String]) -> Vec<u32> {
        (0..self.sort_model.n_items())
            .filter(|position| {
                self.sort_model
                    .item(*position)
                    .and_downcast::<gio::FileInfo>()
                    .is_some_and(|info| {
                        names
                            .iter()
                            .any(|name| *name == info.name().to_string_lossy())
                    })
            })
            .collect()
    }

    /// Selects the item whose name starts with what was typed.
    fn select_typed(&self, typed: &str) {
        let typed = typed.to_lowercase();
        let n = self.sort_model.n_items();
        let current = self.selected_positions().first().copied();
        // Typing the same letter again moves to the next match, as on Windows.
        let repeat = typed.chars().count() > 1 && typed.chars().all(|c| typed.starts_with(c));
        let prefix: String = if repeat {
            typed.chars().take(1).collect()
        } else {
            typed.clone()
        };
        let start = match (repeat, current) {
            (true, Some(current)) => current + 1,
            _ => 0,
        };
        for offset in 0..n {
            let position = (start + offset) % n.max(1);
            if let Some(info) = self
                .sort_model
                .item(position)
                .and_downcast::<gio::FileInfo>()
                && info.display_name().to_lowercase().starts_with(&prefix)
            {
                self.selection.select_item(position, true);
                self.scroll_to(position, true);
                return;
            }
        }
    }

    // --- Renaming -----------------------------------------------------------

    /// Renames the first selected item in place.
    pub fn start_rename(&self) {
        let Some(position) = self.selected_positions().first().copied() else {
            return;
        };
        self.scroll_to(position, false);
        let weak = self.weak();
        // The item's widget may only exist after the scroll.
        glib::idle_add_local_once(move || {
            let Some(view) = weak.upgrade() else { return };
            let Some(cell) = view.context.cell_at(position) else {
                return;
            };
            let Some(file) = cell.info().and_then(|info| util::file_of(&info)) else {
                return;
            };
            let old = cell
                .info()
                .map(|info| info.edit_name().to_string())
                .unwrap_or_default();
            let weak = Rc::downgrade(&view);
            cell.begin_rename(view.context.hide_extensions.get(), move |name| {
                let Some(view) = weak.upgrade() else { return };
                view.focus();
                if let Some(name) = name.filter(|name| *name != old && !name.trim().is_empty()) {
                    view.emit(ViewEvent::Rename {
                        file: file.clone(),
                        name,
                    });
                }
            });
        });
    }

    // --- Views ----------------------------------------------------------------

    fn build_view(&self) {
        let state = self.state.get();
        if self.desktop.get() {
            self.build_desktop_grid(state);
            return;
        }
        let grouped = state.group.is_some();
        let (widget, horizontal): (gtk::Widget, bool) = match state.view {
            View::Details => (self.build_details(state).upcast(), false),
            View::Content => {
                let list = gtk::ListView::new(
                    Some(self.selection.clone()),
                    Some(self.factory(CellKind::Content)),
                );
                list.add_css_class("content-view");
                list.set_show_separators(true);
                if grouped {
                    list.set_header_factory(Some(&self.header_factory()));
                }
                list.set_enable_rubberband(true);
                (list.upcast(), false)
            }
            View::List => (self.build_grid(CellKind::Row, true).upcast(), true),
            View::Small => (self.build_grid(CellKind::Row, false).upcast(), false),
            View::Tiles => (self.build_grid(CellKind::Tile, false).upcast(), false),
            View::Medium => (self.build_grid(CellKind::Grid(48), false).upcast(), false),
            View::Large => (self.build_grid(CellKind::Grid(96), false).upcast(), false),
            View::ExtraLarge => (self.build_grid(CellKind::Grid(256), false).upcast(), false),
        };
        widget.add_css_class(&format!("view-{}", state.view.id()));
        if horizontal {
            self.scrolled
                .set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        } else {
            self.scrolled
                .set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
            if state.view == View::Details {
                self.scrolled
                    .set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
            }
        }
        self.context.cells.borrow_mut().clear();
        self.scrolled.set_child(Some(&widget));
        set_single_click(&widget, config::flag("single-click"));
        self.add_view_controllers(&widget);
        let had_focus = self
            .view
            .borrow()
            .as_ref()
            .is_some_and(|old| old.has_focus() || old.focus_child().is_some());
        self.view.replace(Some(widget.clone()));
        self.built
            .set(Some((state.view, state.group, self.context.in_trash.get())));
        if had_focus {
            widget.grab_focus();
        }
    }

    /// The desktop's view: one grid for good, whatever the sort.
    fn build_desktop_grid(&self, state: ViewState) {
        if self.desktop_grid.borrow().is_none() {
            let grid = DesktopGrid::new(&self.selection, &self.context, &self.dir);
            if let Some(Location::Folder(folder)) = self.location.borrow().as_ref() {
                grid.set_folder(folder);
            }
            self.context.cells.borrow_mut().clear();
            self.scrolled.set_child(Some(&grid));
            set_single_click(grid.upcast_ref(), config::flag("single-click"));
            self.add_view_controllers(grid.upcast_ref());
            self.view.replace(Some(grid.clone().upcast()));
            self.desktop_grid.replace(Some(grid));
        }
        self.built
            .set(Some((state.view, state.group, self.context.in_trash.get())));
    }

    fn build_grid(&self, kind: CellKind, horizontal: bool) -> gtk::GridView {
        let grid = gtk::GridView::new(Some(self.selection.clone()), Some(self.factory(kind)));
        grid.set_max_columns(256);
        grid.set_min_columns(1);
        grid.set_enable_rubberband(true);
        if horizontal {
            grid.set_orientation(gtk::Orientation::Horizontal);
        }
        grid
    }

    fn factory(&self, kind: CellKind) -> gtk::SignalListItemFactory {
        let factory = gtk::SignalListItemFactory::new();
        let context = self.context.clone();
        factory.connect_setup(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let cell = FileCell::new(kind, &context);
            cell.set_list_item(item);
            item.set_child(Some(&cell));
            let weak = cell.downgrade();
            item.connect_selected_notify(move |item| {
                if let Some(cell) = weak.upgrade() {
                    cell.set_selected(item.is_selected());
                }
            });
        });
        factory.connect_bind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let (Some(cell), Some(info)) = (
                item.child().and_downcast::<FileCell>(),
                item.item().and_downcast::<gio::FileInfo>(),
            ) else {
                return;
            };
            cell.bind(&info, item.position(), item.is_selected());
        });
        factory.connect_unbind(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            if let Some(cell) = item.child().and_downcast::<FileCell>() {
                cell.unbind();
            }
        });
        factory
    }

    fn header_factory(&self) -> gtk::SignalListItemFactory {
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(header) = item.downcast_ref::<gtk::ListHeader>() else {
                return;
            };
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("group-header");
            let label = gtk::Label::builder().xalign(0.0).build();
            label.add_css_class("group-title");
            row.append(&label);
            let line = gtk::Separator::new(gtk::Orientation::Horizontal);
            line.set_hexpand(true);
            line.set_valign(gtk::Align::Center);
            row.append(&line);
            header.set_child(Some(&row));
        });
        let state = self.state.clone();
        factory.connect_bind(move |_, item| {
            let Some(header) = item.downcast_ref::<gtk::ListHeader>() else {
                return;
            };
            let Some(column) = state.get().group else {
                return;
            };
            let label = header
                .child()
                .and_then(|row| row.first_child())
                .and_downcast::<gtk::Label>();
            let info = header.item().and_downcast::<gio::FileInfo>();
            if let (Some(label), Some(info)) = (label, info) {
                let (_, title) = group_key(&info, column);
                label.set_label(&format!("{title} ({})", header.n_items()));
            }
        });
        factory
    }

    /// The columns Details shows here.
    fn detail_columns(&self) -> Vec<Column> {
        if self.context.in_trash.get() {
            return vec![
                Column::Name,
                Column::Origin,
                Column::Deleted,
                Column::Size,
                Column::Type,
                Column::Modified,
            ];
        }
        let mut columns: Vec<Column> = config::settings()
            .strv("details-columns")
            .iter()
            .filter_map(|id| Column::from_id(id))
            .filter(|column| {
                !matches!(
                    column,
                    Column::Deleted | Column::Origin | Column::Folder | Column::Purpose
                )
            })
            .collect();
        if !columns.contains(&Column::Name) {
            columns.insert(0, Column::Name);
        }
        // System folder hints add what each folder is for, after the name.
        if self.context.hints.get() {
            let after_name = columns
                .iter()
                .position(|c| *c == Column::Name)
                .map_or(0, |i| i + 1);
            columns.insert(after_name, Column::Purpose);
        }
        if matches!(
            self.location.borrow().as_ref(),
            Some(Location::Search { .. })
        ) {
            columns.push(Column::Folder);
        }
        columns
    }

    fn build_details(&self, state: ViewState) -> gtk::ColumnView {
        let view = gtk::ColumnView::new(Some(self.selection.clone()));
        view.set_enable_rubberband(true);
        view.set_reorderable(true);
        view.set_show_column_separators(false);
        view.set_show_row_separators(false);
        view.add_css_class("details-view");
        let menu = self.header_menu(&view);
        for column in self.detail_columns() {
            let kind = if column == Column::Name {
                CellKind::DetailName
            } else {
                CellKind::Detail(column)
            };
            let col = gtk::ColumnViewColumn::new(Some(column.label()), Some(self.factory(kind)));
            col.set_id(Some(column.id()));
            col.set_resizable(true);
            col.set_fixed_width(column.width());
            // The sorting itself is done by the folder view's own sorter;
            // this only makes the header clickable and show the arrow.
            col.set_sorter(Some(&gtk::CustomSorter::new(|_, _| gtk::Ordering::Equal)));
            col.set_header_menu(Some(&menu));
            view.append_column(&col);
        }
        if state.group.is_some() {
            view.set_header_factory(Some(&self.header_factory()));
        }
        self.show_sort_in_headers(&view);
        if let Some(sorter) = view.sorter().and_downcast::<gtk::ColumnViewSorter>() {
            let weak = self.weak();
            sorter.connect_changed(move |sorter, _| {
                let Some(folder_view) = weak.upgrade() else {
                    return;
                };
                let Some(column) = sorter.primary_sort_column() else {
                    return;
                };
                let Some(sort) = column.id().and_then(|id| Column::from_id(&id)) else {
                    return;
                };
                let descending = sorter.primary_sort_order() == gtk::SortType::Descending;
                let mut state = folder_view.state.get();
                if state.sort == sort && state.descending == descending {
                    return;
                }
                state.sort = sort;
                state.descending = descending;
                folder_view.set_state(state, true);
                folder_view.emit(ViewEvent::StateChanged);
            });
        }
        view
    }

    fn show_sort_in_headers(&self, view: &gtk::ColumnView) {
        let state = self.state.get();
        let columns = view.columns();
        for index in 0..columns.n_items() {
            let Some(column) = columns.item(index).and_downcast::<gtk::ColumnViewColumn>() else {
                continue;
            };
            if column.id().as_deref() == Some(state.sort.id()) {
                let order = if state.descending {
                    gtk::SortType::Descending
                } else {
                    gtk::SortType::Ascending
                };
                let current = view.sorter().and_downcast::<gtk::ColumnViewSorter>();
                let same = current.as_ref().is_some_and(|sorter| {
                    sorter.primary_sort_column().as_ref() == Some(&column)
                        && sorter.primary_sort_order() == order
                });
                if !same {
                    view.sort_by_column(Some(&column), order);
                }
                return;
            }
        }
        view.sort_by_column(None, gtk::SortType::Ascending);
    }

    /// Right-clicking a column header: which columns to show.
    fn header_menu(&self, view: &gtk::ColumnView) -> gio::Menu {
        let menu = gio::Menu::new();
        let group = gio::SimpleActionGroup::new();
        if self.context.in_trash.get() {
            return menu;
        }
        let shown = self.detail_columns();
        let section = gio::Menu::new();
        for column in [
            Column::Modified,
            Column::Created,
            Column::Accessed,
            Column::Type,
            Column::Size,
            Column::Owner,
            Column::Permissions,
        ] {
            let action = gio::SimpleAction::new_stateful(
                column.id(),
                None,
                &shown.contains(&column).to_variant(),
            );
            let weak = self.weak();
            action.connect_activate(move |action, _| {
                let Some(view) = weak.upgrade() else { return };
                let on = !action
                    .state()
                    .and_then(|s| s.get::<bool>())
                    .unwrap_or(false);
                action.set_state(&on.to_variant());
                let mut columns: Vec<String> = config::settings()
                    .strv("details-columns")
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                columns.retain(|id| id != column.id());
                if on {
                    columns.push(column.id().to_owned());
                }
                let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
                let _ = config::settings().set_strv("details-columns", columns.as_slice());
                view.built.set(None);
                view.set_state(view.state.get(), false);
            });
            group.add_action(&action);
            section.append(
                Some(column.label()),
                Some(&format!("columns.{}", column.id())),
            );
        }
        menu.append_section(None, &section);
        view.insert_action_group("columns", Some(&group));
        menu
    }

    fn add_view_controllers(&self, widget: &gtk::Widget) {
        // Opening items.
        let weak = self.weak();
        let activate = move |position: u32| {
            let Some(view) = weak.upgrade() else { return };
            let items = if view.selection.is_selected(position) {
                view.selected_infos()
            } else {
                view.selection
                    .item(position)
                    .and_downcast::<gio::FileInfo>()
                    .into_iter()
                    .collect()
            };
            if !items.is_empty() {
                view.emit(ViewEvent::Activate(items));
            }
        };
        if let Some(grid) = widget.downcast_ref::<gtk::GridView>() {
            grid.connect_activate(move |_, position| activate(position));
        } else if let Some(list) = widget.downcast_ref::<gtk::ListView>() {
            list.connect_activate(move |_, position| activate(position));
        } else if let Some(columns) = widget.downcast_ref::<gtk::ColumnView>() {
            columns.connect_activate(move |_, position| activate(position));
        } else if let Some(grid) = widget.downcast_ref::<DesktopGrid>() {
            grid.connect_activate(activate);
        }

        // Right-click.
        // In the capture phase: grid items would otherwise take the click.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.weak();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(view) = weak.upgrade() else { return };
            let Some(widget) = gesture.widget() else {
                return;
            };
            if on_column_header(&widget, x, y) {
                // Column headers have their own menu.
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
            match find_cell(&widget, x, y) {
                Some(cell) if cell.is_bound() => {
                    let position = cell.position();
                    if !view.selection.is_selected(position) {
                        view.selection.select_item(position, true);
                    }
                    view.emit(ViewEvent::ContextMenu {
                        on_item: true,
                        widget,
                        x,
                        y,
                    });
                }
                _ => {
                    view.selection.unselect_all();
                    view.emit(ViewEvent::ContextMenu {
                        on_item: false,
                        widget,
                        x,
                        y,
                    });
                }
            }
        });
        widget.add_controller(click);

        // A left click on empty space clears the selection, as on Windows.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let weak = self.weak();
        click.connect_released(move |gesture, _, x, y| {
            let Some(view) = weak.upgrade() else { return };
            let Some(widget) = gesture.widget() else {
                return;
            };
            let state = gesture.current_event_state();
            if find_cell(&widget, x, y).is_none()
                && !state
                    .intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK)
                && is_background(&widget, x, y)
            {
                view.selection.unselect_all();
            }
        });
        widget.add_controller(click);

        // Keys: the context-menu key, Shift+F10, and typing to select.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.weak();
        keys.connect_key_pressed(move |controller, key, _, state| {
            let Some(view) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if view
                .context
                .cells
                .borrow()
                .iter()
                .filter_map(|w| w.upgrade())
                .any(|cell| cell.is_renaming())
            {
                return glib::Propagation::Proceed;
            }
            let menu_key = key == gdk::Key::Menu
                || (key == gdk::Key::F10 && state.contains(gdk::ModifierType::SHIFT_MASK));
            if menu_key {
                let Some(widget) = controller.widget() else {
                    return glib::Propagation::Proceed;
                };
                let (on_item, x, y) = match view
                    .selected_positions()
                    .first()
                    .and_then(|p| view.context.cell_at(*p))
                {
                    Some(cell) => {
                        let bounds = cell.compute_bounds(&widget);
                        let (x, y) = bounds
                            .map(|b| (b.x() as f64 + 16.0, b.y() as f64 + b.height() as f64 / 2.0))
                            .unwrap_or((16.0, 16.0));
                        (true, x, y)
                    }
                    None => (false, 16.0, 16.0),
                };
                view.emit(ViewEvent::ContextMenu {
                    on_item,
                    widget,
                    x,
                    y,
                });
                return glib::Propagation::Stop;
            }
            if state.intersects(
                gdk::ModifierType::CONTROL_MASK
                    | gdk::ModifierType::ALT_MASK
                    | gdk::ModifierType::SUPER_MASK,
            ) {
                return glib::Propagation::Proceed;
            }
            let Some(ch) = key.to_unicode().filter(|c| {
                !c.is_control() && (!c.is_whitespace() || !view.typed.borrow().is_empty())
            }) else {
                return glib::Propagation::Proceed;
            };
            if config::string("typing-in-view") == "search" {
                view.emit(ViewEvent::TypeToSearch(ch.to_string()));
                return glib::Propagation::Stop;
            }
            view.typed.borrow_mut().push(ch);
            if let Some(timer) = view.typed_timer.take() {
                timer.remove();
            }
            let typed = view.typed.borrow().clone();
            view.select_typed(&typed);
            let weak = Rc::downgrade(&view);
            let timer =
                glib::timeout_add_local_once(std::time::Duration::from_millis(1000), move || {
                    if let Some(view) = weak.upgrade() {
                        view.typed_timer.take();
                        view.typed.borrow_mut().clear();
                    }
                });
            view.typed_timer.replace(Some(timer));
            glib::Propagation::Stop
        });
        widget.add_controller(keys);

        // Ctrl+wheel changes the view, as on Windows.
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.weak();
        scroll.connect_scroll(move |controller, _, dy| {
            if !controller
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK)
            {
                return glib::Propagation::Proceed;
            }
            if let Some(view) = weak.upgrade() {
                if view.desktop.get() {
                    crate::desktop::step_icon_size(dy < 0.0);
                    return glib::Propagation::Stop;
                }
                let mut state = view.state.get();
                state.view = state.view.step(dy < 0.0);
                view.set_state(state, true);
                view.emit(ViewEvent::StateChanged);
            }
            glib::Propagation::Stop
        });
        widget.add_controller(scroll);
    }

    fn add_background_drop(&self) {
        let target = gtk::DropTarget::new(
            gdk::FileList::static_type(),
            gdk::DragAction::COPY | gdk::DragAction::MOVE,
        );
        target.set_preload(true);
        let weak = self.weak();
        target.connect_accept(move |_, _| {
            weak.upgrade().is_some_and(|view| {
                matches!(view.location.borrow().as_ref(), Some(Location::Folder(_)))
            })
        });
        let weak = self.weak();
        target.connect_motion(move |target, _, _| {
            let Some(view) = weak.upgrade() else {
                return gdk::DragAction::empty();
            };
            let folder = view
                .location
                .borrow()
                .as_ref()
                .and_then(|l| l.folder().cloned());
            match folder {
                Some(folder) => crate::dnd::preferred_action(target, &folder),
                None => gdk::DragAction::empty(),
            }
        });
        let weak = self.weak();
        target.connect_drop(move |target, value, _, _| {
            let Some(view) = weak.upgrade() else {
                return false;
            };
            let Some(folder) = view
                .location
                .borrow()
                .as_ref()
                .and_then(|l| l.folder().cloned())
            else {
                return false;
            };
            let Ok(list) = value.get::<gdk::FileList>() else {
                return false;
            };
            let files: Vec<gio::File> = list.files();
            // Dropping items back where they are does nothing.
            if files
                .iter()
                .all(|file| file.parent().is_some_and(|parent| parent.equal(&folder)))
            {
                return false;
            }
            let action = crate::dnd::drop_action(target, &folder);
            view.emit(ViewEvent::Drop {
                files,
                folder,
                action,
            });
            true
        });
        self.scrolled.add_controller(target);
    }

    pub fn scroll_to(&self, position: u32, select_focus: bool) {
        let flags = if select_focus {
            gtk::ListScrollFlags::FOCUS
        } else {
            gtk::ListScrollFlags::NONE
        };
        let view = self.view.borrow().clone();
        if let Some(grid) = view.as_ref().and_then(|w| w.downcast_ref::<DesktopGrid>()) {
            grid.focus_item(position, select_focus);
        } else if let Some(grid) = view
            .as_ref()
            .and_then(|w| w.downcast_ref::<gtk::GridView>())
        {
            grid.scroll_to(position, flags, None);
        } else if let Some(list) = view
            .as_ref()
            .and_then(|w| w.downcast_ref::<gtk::ListView>())
        {
            list.scroll_to(position, flags, None);
        } else if let Some(columns) = view
            .as_ref()
            .and_then(|w| w.downcast_ref::<gtk::ColumnView>())
        {
            columns.scroll_to(position, None::<&gtk::ColumnViewColumn>, flags, None);
        }
    }

    pub fn scroll_to_top(&self) {
        self.scrolled.vadjustment().set_value(0.0);
        self.scrolled.hadjustment().set_value(0.0);
    }

    pub fn focus(&self) {
        if let Some(view) = self.view.borrow().as_ref() {
            view.grab_focus();
        }
    }

    /// Reloads the folder from disk (F5).
    pub fn refresh(&self) {
        match self.location.borrow().clone() {
            Some(Location::Folder(file)) => {
                self.dir.set_file(None::<&gio::File>);
                self.dir.set_file(Some(&file));
            }
            Some(location @ Location::Search { .. }) => self.show_search(&location),
            _ => {}
        }
    }
}

fn set_single_click(view: &gtk::Widget, single: bool) {
    if let Some(grid) = view.downcast_ref::<DesktopGrid>() {
        grid.set_single_click(single);
    } else if let Some(grid) = view.downcast_ref::<gtk::GridView>() {
        grid.set_single_click_activate(single);
    } else if let Some(list) = view.downcast_ref::<gtk::ListView>() {
        list.set_single_click_activate(single);
    } else if let Some(columns) = view.downcast_ref::<gtk::ColumnView>() {
        columns.set_single_click_activate(single);
    }
}

/// The file cell under a point of a list widget.
fn find_cell(widget: &gtk::Widget, x: f64, y: f64) -> Option<FileCell> {
    // The desktop places its own cells, and knows which is where.
    if let Some(grid) = widget.downcast_ref::<DesktopGrid>() {
        return grid.cell_at_point(x, y);
    }
    let picked = widget.pick(x, y, gtk::PickFlags::DEFAULT)?;
    let mut current = Some(picked);
    while let Some(candidate) = current {
        if let Some(cell) = candidate.downcast_ref::<FileCell>() {
            return Some(cell.clone());
        }
        // A list item's own widget, or a Details row, holds the cell.
        let mut child = candidate.first_child();
        for _ in 0..2 {
            match child {
                Some(found) if found.is::<FileCell>() => return found.downcast().ok(),
                Some(found) => child = found.first_child(),
                None => break,
            }
        }
        if &candidate == widget {
            break;
        }
        current = candidate.parent();
    }
    None
}

/// Whether a point is on Details' column headers.
fn on_column_header(widget: &gtk::Widget, x: f64, y: f64) -> bool {
    let mut current = widget.pick(x, y, gtk::PickFlags::DEFAULT);
    while let Some(candidate) = current {
        if candidate.css_name() == "header"
            && candidate
                .parent()
                .is_some_and(|p| p.is::<gtk::ColumnView>())
        {
            return true;
        }
        if &candidate == widget {
            break;
        }
        current = candidate.parent();
    }
    false
}

/// Whether a point is on the list's empty background, not a header or scrollbar.
fn is_background(widget: &gtk::Widget, x: f64, y: f64) -> bool {
    let Some(picked) = widget.pick(x, y, gtk::PickFlags::DEFAULT) else {
        return false;
    };
    let name = picked.type_().name();
    !(name.contains("Header") || name.contains("Scrollbar") || name.contains("Button"))
}

/// The order of two items: folders first, then by the sort column; a
/// descending sort reverses everything, as Windows does.
pub fn compare(a: &gio::FileInfo, b: &gio::FileInfo, state: &ViewState) -> Ordering {
    let folders_first = (!util::is_dir(a)).cmp(&!util::is_dir(b));
    let by_name = || util::natural_cmp(&a.display_name(), &b.display_name());
    let time = |time: Option<glib::DateTime>| {
        time.map(|t| (t.to_unix(), t.microsecond()))
            .unwrap_or((i64::MIN, 0))
    };
    let by_key = match state.sort {
        Column::Name => Ordering::Equal,
        Column::Modified => time(util::modified(a)).cmp(&time(util::modified(b))),
        Column::Created => time(util::created(a)).cmp(&time(util::created(b))),
        Column::Accessed => time(util::accessed(a)).cmp(&time(util::accessed(b))),
        Column::Deleted => time(filecell::deletion_date(a)).cmp(&time(filecell::deletion_date(b))),
        Column::Size => a.size().cmp(&b.size()),
        Column::Type => filecell::type_text(a)
            .to_lowercase()
            .cmp(&filecell::type_text(b).to_lowercase()),
        column @ (Column::Owner
        | Column::Permissions
        | Column::Origin
        | Column::Folder
        | Column::Purpose) => util::natural_cmp(
            &filecell::column_text(a, column, false),
            &filecell::column_text(b, column, false),
        ),
    };
    let order = folders_first.then(by_key).then_with(by_name);
    if state.descending {
        order.reverse()
    } else {
        order
    }
}

/// Which group an item falls in, as (order, title).
pub fn group_key(info: &gio::FileInfo, column: Column) -> (i64, String) {
    match column {
        Column::Name => {
            let first = info
                .display_name()
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase())
                .unwrap_or(' ');
            match first {
                '0'..='9' => (0, "0 - 9".into()),
                'A'..='H' => (1, "A - H".into()),
                'I'..='P' => (2, "I - P".into()),
                'Q'..='Z' => (3, "Q - Z".into()),
                _ => (4, "Other".into()),
            }
        }
        Column::Modified | Column::Created | Column::Accessed | Column::Deleted => {
            let time = match column {
                Column::Modified => util::modified(info),
                Column::Created => util::created(info),
                Column::Accessed => util::accessed(info),
                _ => filecell::deletion_date(info),
            };
            date_group(time)
        }
        Column::Size => {
            if util::is_dir(info) {
                return (-1, "Folders".into());
            }
            let size = info.size().max(0) as u64;
            const KB: u64 = 1024;
            const MB: u64 = KB * 1024;
            const GB: u64 = MB * 1024;
            match size {
                0 => (0, "Empty (0 KB)".into()),
                s if s < 16 * KB => (1, "Tiny (0 - 16 KB)".into()),
                s if s < MB => (2, "Small (16 KB - 1 MB)".into()),
                s if s < 128 * MB => (3, "Medium (1 - 128 MB)".into()),
                s if s < GB => (4, "Large (128 MB - 1 GB)".into()),
                s if s < 4 * GB => (5, "Huge (1 - 4 GB)".into()),
                _ => (6, "Gigantic (> 4 GB)".into()),
            }
        }
        Column::Type => {
            let text = filecell::type_text(info);
            (if util::is_dir(info) { -1 } else { 0 }, text)
        }
        column => (0, filecell::column_text(info, column, false)),
    }
}

/// Windows' date groups. Older groups sort first, so a newest-first sort
/// starts with "Today".
fn date_group(time: Option<glib::DateTime>) -> (i64, String) {
    let Some(time) = time.and_then(|t| t.to_local().ok()) else {
        return (-1, "Unspecified".into());
    };
    let Ok(now) = glib::DateTime::now_local() else {
        return (0, "A long time ago".into());
    };
    let day = |t: &glib::DateTime| t.year() as i64 * 400 + t.day_of_year() as i64;
    let today = day(&now);
    let that_day = day(&time);
    let days_ago = today - that_day;
    let week_start = today - (now.day_of_week() as i64 - 1);
    let (rank, title) = if that_day > today {
        (9, "In the future")
    } else if days_ago == 0 {
        (8, "Today")
    } else if days_ago == 1 {
        (7, "Yesterday")
    } else if that_day >= week_start {
        (6, "Earlier this week")
    } else if that_day >= week_start - 7 {
        (5, "Last week")
    } else if time.year() == now.year() && time.month() == now.month() {
        (4, "Earlier this month")
    } else if (time.year() == now.year() && time.month() == now.month() - 1)
        || (now.month() == 1 && time.month() == 12 && time.year() == now.year() - 1)
    {
        (3, "Last month")
    } else if time.year() == now.year() {
        (2, "Earlier this year")
    } else {
        (1, "A long time ago")
    };
    (rank, title.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, dir: bool, size: i64) -> gio::FileInfo {
        let info = gio::FileInfo::new();
        info.set_name(name);
        info.set_display_name(name);
        info.set_file_type(if dir {
            gio::FileType::Directory
        } else {
            gio::FileType::Regular
        });
        info.set_size(size);
        info
    }

    fn state(sort: Column, descending: bool) -> ViewState {
        ViewState {
            view: View::Details,
            sort,
            descending,
            group: None,
        }
    }

    #[test]
    fn folders_come_first_and_names_sort_naturally() {
        let mut items = [
            info("b10.txt", false, 1),
            info("Zoo", true, 0),
            info("b2.txt", false, 5),
            info("apple", true, 0),
        ];
        items.sort_by(|a, b| compare(a, b, &state(Column::Name, false)));
        let names: Vec<String> = items.iter().map(|i| i.display_name().to_string()).collect();
        assert_eq!(names, ["apple", "Zoo", "b2.txt", "b10.txt"]);
    }

    #[test]
    fn descending_reverses_everything() {
        let mut items = [
            info("a.txt", false, 1),
            info("Folder", true, 0),
            info("b.txt", false, 5),
        ];
        items.sort_by(|a, b| compare(a, b, &state(Column::Size, true)));
        let names: Vec<String> = items.iter().map(|i| i.display_name().to_string()).collect();
        assert_eq!(names, ["b.txt", "a.txt", "Folder"]);
    }

    #[test]
    fn sizes_group_like_windows() {
        assert_eq!(
            group_key(&info("x", false, 0), Column::Size).1,
            "Empty (0 KB)"
        );
        assert_eq!(
            group_key(&info("x", false, 2000), Column::Size).1,
            "Tiny (0 - 16 KB)"
        );
        assert_eq!(
            group_key(&info("x", false, 5 << 20), Column::Size).1,
            "Medium (1 - 128 MB)"
        );
        assert_eq!(group_key(&info("d", true, 0), Column::Size).1, "Folders");
    }
}
