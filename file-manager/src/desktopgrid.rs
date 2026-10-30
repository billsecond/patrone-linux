//! The desktop's icons (desktop.rs), laid out the way Windows lays out its
//! desktop: down the left side first, then the next column, each icon where
//! it was put unless Auto arrange is on. The same view shows the desktop files
//! panel's files, each where it would be on the desktop.
//!
//! It shows the folder view's own list and selection with File Manager's own
//! cells (filecell.rs), so the desktop's icons, thumbnails, names, renaming
//! and dragging are File Manager's. What it adds is what a list widget
//! would do: placing the cells, selecting by click, rubber band and arrow
//! keys, opening, and moving icons by dragging them.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene, gsk, prelude::*};

use crate::config;
use crate::filecell::{CellContext, CellKind, FileCell};

/// Which window the cells are in: the desktop's, or the desktop files
/// panel's, which always shows them, says so when there are none, and takes
/// drops on the Recycle Bin riding on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Layout {
    #[default]
    Desktop,
    Panel,
}

/// x, y, width, height.
pub type Rect = (f64, f64, f64, f64);

/// From the free area's edges to the first icons.
const MARGIN: f64 = 8.0;
/// How far the pointer moves before a press on the desktop draws a rubber
/// band.
const BAND_THRESHOLD: f64 = 4.0;
/// How long icons dropped from elsewhere keep landing where they were
/// dropped, as their copies arrive.
const DROP_ANCHOR_TIME: std::time::Duration = std::time::Duration::from_secs(15);

/// The desktop's icon sizes, as Windows names them.
pub const ICON_SIZES: [(&str, &str, i32); 3] = [
    ("large", "Large icons", 96),
    ("medium", "Medium icons", 48),
    ("small", "Small icons", 32),
];

/// The room each icon takes on the desktop, by icon size.
fn pitch(size: i32) -> (f64, f64) {
    match size {
        96 => (124.0, 152.0),
        32 => (84.0, 82.0),
        _ => (96.0, 102.0),
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && a.0 + a.2 > b.0 && a.1 < b.1 + b.3 && a.1 + a.3 > b.1
}

/// The desktop's icon size from the settings.
pub fn icon_size() -> i32 {
    let id = config::string("desktop-icon-size");
    ICON_SIZES
        .iter()
        .find(|(name, _, _)| *name == id)
        .map_or(48, |(_, _, size)| *size)
}

type Handler<T> = RefCell<Option<Rc<dyn Fn(T)>>>;

/// Where the desktop's grid starts and how many places it has.
#[derive(Clone, Copy, Default, Debug)]
pub struct Grid {
    x: f64,
    y: f64,
    pitch: (f64, f64),
    columns: i32,
    rows: i32,
}

impl Grid {
    fn place(&self, (column, row): (i32, i32)) -> (f64, f64) {
        (
            self.x + column as f64 * self.pitch.0,
            self.y + row as f64 * self.pitch.1,
        )
    }

    /// The nearest place to a cell's top left corner, inside the grid.
    fn nearest(&self, x: f64, y: f64) -> (i32, i32) {
        let column = ((x - self.x) / self.pitch.0).round() as i32;
        let row = ((y - self.y) / self.pitch.1).round() as i32;
        (
            column.clamp(0, (self.columns - 1).max(0)),
            row.clamp(0, (self.rows - 1).max(0)),
        )
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct DesktopGrid {
        pub selection: RefCell<Option<gtk::MultiSelection>>,
        pub context: RefCell<Option<Rc<CellContext>>>,
        pub dir: RefCell<Option<gtk::DirectoryList>>,
        pub folder: RefCell<Option<gio::File>>,
        pub cells: RefCell<Vec<FileCell>>,
        /// The icon size the cells were made for.
        pub built_size: Cell<i32>,
        /// Each item's place: the top left corner of its room.
        pub places: RefCell<Vec<Option<(f64, f64)>>>,
        /// Each item's place on the desktop's grid.
        pub grid_places: RefCell<Vec<Option<(i32, i32)>>>,
        /// Where each cell was drawn, for clicks and the rubber band.
        pub rects: RefCell<Vec<Option<Rect>>>,
        pub grid: Cell<Grid>,
        pub layout: Cell<Layout>,
        /// The screen's free area, in the window.
        pub area: Cell<Option<Rect>>,
        /// The Recycle Bin, while it's in this window: no icon goes under it.
        pub avoid: Cell<Option<Rect>>,
        pub auto_arrange: Cell<bool>,
        pub shown: Cell<bool>,
        pub single_click: Cell<bool>,
        /// Icons' places on the desktop by name, kept in the settings.
        pub saved: RefCell<HashMap<String, (i32, i32)>>,
        pub cursor: Cell<Option<u32>>,
        pub anchor: Cell<Option<u32>>,
        /// The icon pressed last and where: what a drag of it carries.
        pub pressed: Cell<Option<(u32, f64, f64)>>,
        /// An already selected icon clicked without moving: on release, it
        /// alone stays selected (a press keeps them all, to drag them).
        pub reselect: Cell<Option<u32>>,
        pub band_start: Cell<Option<(f64, f64)>>,
        pub band: Cell<Option<Rect>>,
        pub band_base: RefCell<Option<gtk::Bitset>>,
        pub drop_anchor: Cell<Option<((i32, i32), std::time::Instant)>>,
        pub empty: RefCell<Option<gtk::Label>>,
        pub on_activate: Handler<u32>,
        pub on_reveal: Handler<()>,
        pub on_trash: Handler<Vec<gio::File>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DesktopGrid {
        const NAME: &'static str = "PfmDesktopGrid";
        type Type = super::DesktopGrid;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("desktopgrid");
        }
    }

    impl ObjectImpl for DesktopGrid {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for DesktopGrid {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            // It fills whatever window it's in.
            (0, 0, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            self.obj().allocate_cells(width, height);
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.parent_snapshot(snapshot);
            if let Some((x, y, width, height)) = self.band.get() {
                let rect = graphene::Rect::new(x as f32, y as f32, width as f32, height as f32);
                let accent = gdk::RGBA::new(0.157, 0.545, 0.98, 1.0);
                snapshot.append_color(&gdk::RGBA::new(0.157, 0.545, 0.98, 0.15), &rect);
                snapshot.append_border(
                    &gsk::RoundedRect::from_rect(rect, 0.0),
                    &[1.0; 4],
                    &[accent; 4],
                );
            }
        }
    }
}

glib::wrapper! {
    pub struct DesktopGrid(ObjectSubclass<imp::DesktopGrid>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl DesktopGrid {
    pub fn new(
        selection: &gtk::MultiSelection,
        context: &Rc<CellContext>,
        dir: &gtk::DirectoryList,
    ) -> DesktopGrid {
        let grid: DesktopGrid = glib::Object::new();
        grid.set_focusable(true);
        grid.set_hexpand(true);
        grid.set_vexpand(true);
        grid.add_css_class("desktop");
        let imp = grid.imp();
        imp.selection.replace(Some(selection.clone()));
        imp.context.replace(Some(context.clone()));
        imp.dir.replace(Some(dir.clone()));
        imp.auto_arrange.set(config::flag("desktop-auto-arrange"));
        imp.shown.set(config::flag("desktop-show-icons"));
        imp.saved.replace(load_places());

        let weak = grid.downgrade();
        selection.connect_items_changed(move |_, _, _, _| {
            if let Some(grid) = weak.upgrade() {
                grid.rebuild();
            }
        });
        let weak = grid.downgrade();
        selection.connect_selection_changed(move |_, _, _| {
            if let Some(grid) = weak.upgrade() {
                grid.show_selection();
            }
        });
        // Once the folder is read, the icons' places are worth keeping.
        let weak = grid.downgrade();
        dir.connect_loading_notify(move |dir| {
            if !dir.is_loading()
                && let Some(grid) = weak.upgrade()
            {
                grid.rebuild();
            }
        });
        grid.add_controllers();
        grid.rebuild();
        grid
    }

    fn selection(&self) -> gtk::MultiSelection {
        self.imp()
            .selection
            .borrow()
            .clone()
            .expect("the grid has a list")
    }

    fn info(&self, position: u32) -> Option<gio::FileInfo> {
        self.selection().item(position).and_downcast()
    }

    fn name(&self, position: u32) -> String {
        self.info(position)
            .map(|info| info.name().to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    // --- Settings from the desktop -------------------------------------------

    pub fn set_folder(&self, folder: &gio::File) {
        self.imp().folder.replace(Some(folder.clone()));
    }

    pub fn layout(&self) -> Layout {
        self.imp().layout.get()
    }

    pub fn set_layout(&self, layout: Layout) {
        let imp = self.imp();
        if imp.layout.replace(layout) != layout {
            self.rebuild();
        }
    }

    /// The screen's free area (the work area), in the window.
    pub fn set_area(&self, area: Rect) {
        self.imp().area.set(Some(area));
        self.rebuild();
    }

    /// The Recycle Bin: where it is, while it's in this window.
    pub fn set_bin(&self, place: Option<Rect>) {
        self.imp().avoid.set(place);
        self.rebuild();
    }

    pub fn set_single_click(&self, single: bool) {
        self.imp().single_click.set(single);
    }

    /// Re-reads the View settings: icon size, Auto arrange, Show desktop
    /// icons.
    pub fn apply_settings(&self) {
        let imp = self.imp();
        imp.auto_arrange.set(config::flag("desktop-auto-arrange"));
        imp.shown.set(config::flag("desktop-show-icons"));
        self.rebuild();
    }

    /// A renamed icon keeps its place, as on Windows (places are kept by
    /// name).
    pub fn carry_place(&self, old: &str, new: &str) {
        let mut saved = self.imp().saved.borrow_mut();
        if let Some(place) = saved.remove(old) {
            saved.insert(new.to_owned(), place);
            save_places(&saved);
        }
    }

    /// Puts every icon back in the list's order (Sort by, with Auto arrange
    /// off: Windows arranges them once).
    pub fn arrange_in_order(&self) {
        self.imp().saved.borrow_mut().clear();
        save_places(&HashMap::new());
        self.rebuild();
    }

    /// Files dropped on the Recycle Bin while it rides on the panel (its own
    /// drop target is under the panel then).
    pub fn connect_trash(&self, handler: impl Fn(Vec<gio::File>) + 'static) {
        self.imp().on_trash.replace(Some(Rc::new(handler)));
    }

    pub fn connect_activate(&self, handler: impl Fn(u32) + 'static) {
        self.imp().on_activate.replace(Some(Rc::new(handler)));
    }

    /// An item was asked to be shown (a new folder, a paste, a rename).
    pub fn connect_reveal(&self, handler: impl Fn(()) + 'static) {
        self.imp().on_reveal.replace(Some(Rc::new(handler)));
    }

    fn call<T>(handler: &Handler<T>, value: T) {
        let handler = handler.borrow().clone();
        if let Some(handler) = handler {
            handler(value);
        }
    }

    // --- Cells -----------------------------------------------------------------

    /// One cell for each item, shown and placed.
    fn rebuild(&self) {
        let imp = self.imp();
        let Some(context) = imp.context.borrow().clone() else {
            return;
        };
        let selection = self.selection();
        let n = selection.n_items() as usize;
        let size = icon_size();
        {
            let mut cells = imp.cells.borrow_mut();
            if imp.built_size.replace(size) != size {
                for cell in cells.drain(..) {
                    cell.unparent();
                }
            }
            while cells.len() > n {
                if let Some(cell) = cells.pop() {
                    cell.unparent();
                }
            }
            while cells.len() < n {
                let cell = FileCell::new(CellKind::Grid(size), &context);
                cell.set_parent(self);
                cells.push(cell);
            }
        }
        let cells = imp.cells.borrow().clone();
        for (position, cell) in cells.iter().enumerate() {
            let position = position as u32;
            let Some(info) = self.info(position) else {
                continue;
            };
            cell.set_position(position);
            cell.bind(&info, position, selection.is_selected(position));
        }
        if let Some(cursor) = imp.cursor.get()
            && cursor as usize >= n
        {
            imp.cursor.set(n.checked_sub(1).map(|last| last as u32));
        }
        self.show_selection();
        self.arrange();
        self.queue_allocate();
        self.queue_draw();
    }

    /// Selected cells look selected, and the keyboard's place shows.
    fn show_selection(&self) {
        let imp = self.imp();
        let selection = self.selection();
        let cursor = imp.cursor.get();
        for (position, cell) in imp.cells.borrow().iter().enumerate() {
            let position = position as u32;
            if selection.is_selected(position) {
                cell.set_state_flags(gtk::StateFlags::SELECTED, false);
            } else {
                cell.unset_state_flags(gtk::StateFlags::SELECTED);
            }
            cell.set_selected(selection.is_selected(position));
            if cursor == Some(position) {
                cell.add_css_class("cursor");
            } else {
                cell.remove_css_class("cursor");
            }
        }
    }

    // --- Places ------------------------------------------------------------------

    /// Works out every item's place.
    fn arrange(&self) {
        let imp = self.imp();
        let n = imp.cells.borrow().len();
        let mut places = vec![None; n];
        let mut grid_places = vec![None; n];
        let area = imp.area.get().unwrap_or((
            0.0,
            0.0,
            self.width().max(1) as f64,
            self.height().max(1) as f64,
        ));
        let pitch = pitch(icon_size());
        let grid = Grid {
            x: area.0 + MARGIN,
            y: area.1 + MARGIN,
            pitch,
            columns: (((area.2 - 2.0 * MARGIN) / pitch.0).floor() as i32).max(1),
            rows: (((area.3 - 2.0 * MARGIN) / pitch.1).floor() as i32).max(1),
        };
        imp.grid.set(grid);
        let names: Vec<String> = (0..n as u32).map(|p| self.name(p)).collect();
        let assigned = self.assign(&grid, &names);
        for (index, place) in assigned.into_iter().enumerate() {
            grid_places[index] = Some(place);
            places[index] = Some(grid.place(place));
        }
        imp.places.replace(places);
        imp.grid_places.replace(grid_places);
    }

    /// Whether a place on the desktop's grid is under the Recycle Bin.
    fn blocked(&self, grid: &Grid, place: (i32, i32)) -> bool {
        let (x, y) = grid.place(place);
        self.imp()
            .avoid
            .get()
            .is_some_and(|bin| overlaps((x, y, grid.pitch.0, grid.pitch.1), bin))
    }

    /// The desktop's places for these items, in order: where each was put,
    /// else the first free place down the columns. Auto arrange puts them
    /// all in the list's order.
    fn assign(&self, grid: &Grid, names: &[String]) -> Vec<(i32, i32)> {
        let imp = self.imp();
        let auto = imp.auto_arrange.get();
        let saved = imp.saved.borrow().clone();
        let mut taken: HashSet<(i32, i32)> = HashSet::new();
        let mut places: Vec<Option<(i32, i32)>> = vec![None; names.len()];
        if !auto {
            for (index, name) in names.iter().enumerate() {
                if let Some(&place) = saved.get(name)
                    && place.0 < grid.columns
                    && place.1 < grid.rows
                    && !self.blocked(grid, place)
                    && taken.insert(place)
                {
                    places[index] = Some(place);
                }
            }
        }
        let anchor = imp
            .drop_anchor
            .get()
            .filter(|(_, at)| at.elapsed() < DROP_ANCHOR_TIME)
            .map(|(place, _)| place);
        let rows = grid.rows.max(1);
        let index_of = |(column, row): (i32, i32)| column * rows + row;
        for (index, name) in names.iter().enumerate() {
            if places[index].is_some() {
                continue;
            }
            // Dropped here from elsewhere: from where it was dropped.
            let start = match anchor {
                Some(place) if !auto && !saved.contains_key(name) => index_of(place),
                _ => 0,
            };
            let mut slot = start;
            loop {
                let place = (slot / rows, slot % rows);
                // Past the screen's last column (more icons than room),
                // they go on off the screen, as on Windows.
                if !taken.contains(&place)
                    && (place.0 >= grid.columns || !self.blocked(grid, place))
                {
                    taken.insert(place);
                    places[index] = Some(place);
                    break;
                }
                slot += 1;
            }
        }
        let places: Vec<(i32, i32)> = places.into_iter().flatten().collect();
        // Kept once the folder is read, so icons stay where they landed.
        let loaded = imp
            .dir
            .borrow()
            .as_ref()
            .is_some_and(|dir| !dir.is_loading());
        if !auto && loaded {
            let mut keep: HashMap<String, (i32, i32)> =
                names.iter().cloned().zip(places.iter().copied()).collect();
            if keep != saved {
                // Items not here now keep their places too, for a while:
                // a file moved away and back lands where it was.
                for (name, place) in saved.iter() {
                    if keep.len() >= 400 {
                        break;
                    }
                    keep.entry(name.clone()).or_insert(*place);
                }
                save_places(&keep);
                imp.saved.replace(keep);
            }
        }
        places
    }

    /// Gives each cell its room.
    fn allocate_cells(&self, width: i32, height: i32) {
        let imp = self.imp();
        if imp.area.get().is_none() {
            // Until the taskbar says where the free area is, the window.
            let grid = imp.grid.get();
            if grid.columns == 0 || self.width() != width || self.height() != height {
                self.arrange();
            }
        }
        let pitch = pitch(icon_size());
        let shown = imp.shown.get() || imp.layout.get() == Layout::Panel;
        let places = imp.places.borrow().clone();
        let cells = imp.cells.borrow().clone();
        let mut rects = vec![None; cells.len()];
        for (index, cell) in cells.iter().enumerate() {
            let place = places.get(index).copied().flatten().filter(|_| shown);
            let Some((x, y)) = place else {
                cell.set_child_visible(false);
                continue;
            };
            cell.set_child_visible(true);
            let (_, natural_width, _, _) = cell.measure(gtk::Orientation::Horizontal, -1);
            let cell_width = natural_width.min(pitch.0 as i32);
            let (_, natural_height, _, _) = cell.measure(gtk::Orientation::Vertical, cell_width);
            let cell_height = natural_height.min(pitch.1 as i32 - 2);
            let cell_x = (x + (pitch.0 - cell_width as f64) / 2.0).round();
            let cell_y = (y + 2.0).round();
            cell.size_allocate(
                &gtk::Allocation::new(cell_x as i32, cell_y as i32, cell_width, cell_height),
                -1,
            );
            rects[index] = Some((cell_x, cell_y, cell_width as f64, cell_height as f64));
        }
        imp.rects.replace(rects);
        self.allocate_empty();
    }

    /// "Nothing on the desktop", where the first file would be in the empty
    /// panel.
    fn allocate_empty(&self) {
        let imp = self.imp();
        if imp.layout.get() == Layout::Panel && imp.cells.borrow().is_empty() {
            let empty = self.empty_label();
            empty.set_child_visible(true);
            let (_, width, _, _) = empty.measure(gtk::Orientation::Horizontal, -1);
            let (_, height, _, _) = empty.measure(gtk::Orientation::Vertical, width);
            let inset = (2.0 * MARGIN) as i32;
            empty.size_allocate(&gtk::Allocation::new(inset, inset, width, height), -1);
        } else if let Some(empty) = imp.empty.borrow().as_ref() {
            empty.set_child_visible(false);
        }
    }

    fn empty_label(&self) -> gtk::Label {
        let imp = self.imp();
        if let Some(empty) = imp.empty.borrow().as_ref() {
            return empty.clone();
        }
        let empty = gtk::Label::new(Some("Nothing on the desktop"));
        empty.add_css_class("desktop-panel-empty");
        empty.set_parent(self);
        imp.empty.replace(Some(empty.clone()));
        empty
    }

    /// The cell drawn at a point.
    pub fn cell_at_point(&self, x: f64, y: f64) -> Option<FileCell> {
        let position = self.item_at(x, y)?;
        self.imp().cells.borrow().get(position as usize).cloned()
    }

    /// The item drawn at a point.
    pub fn item_at(&self, x: f64, y: f64) -> Option<u32> {
        let rects = self.imp().rects.borrow();
        rects
            .iter()
            .position(|rect| {
                rect.is_some_and(|(rx, ry, rw, rh)| {
                    x >= rx && x < rx + rw && y >= ry && y < ry + rh
                })
            })
            .map(|index| index as u32)
    }

    // --- Selecting and opening ---------------------------------------------------

    /// Moves the keyboard's place to an item, as a list's scroll_to does,
    /// and with the panel on asks for the panel to show.
    pub fn focus_item(&self, position: u32, focus: bool) {
        let imp = self.imp();
        if position as usize >= imp.cells.borrow().len() {
            return;
        }
        imp.cursor.set(Some(position));
        imp.anchor.set(Some(position));
        if focus {
            self.grab_focus();
        }
        self.show_selection();
        if imp.layout.get() == Layout::Panel {
            Self::call(&imp.on_reveal, ());
        }
    }

    fn activate(&self, position: u32) {
        Self::call(&self.imp().on_activate, position);
    }

    fn select_only(&self, position: u32) {
        let imp = self.imp();
        self.selection().select_item(position, true);
        imp.cursor.set(Some(position));
        imp.anchor.set(Some(position));
        self.show_selection();
    }

    /// Shift+click or Shift+arrow: from the anchor to here, in the list's
    /// order.
    fn select_range(&self, position: u32) {
        let imp = self.imp();
        let anchor = imp.anchor.get().unwrap_or(position);
        let (from, to) = (anchor.min(position), anchor.max(position));
        let n = self.selection().n_items();
        let selected = gtk::Bitset::new_range(from, to - from + 1);
        self.selection()
            .set_selection(&selected, &gtk::Bitset::new_range(0, n));
        imp.cursor.set(Some(position));
        self.show_selection();
    }

    fn toggle(&self, position: u32) {
        let imp = self.imp();
        let selection = self.selection();
        if selection.is_selected(position) {
            selection.unselect_item(position);
        } else {
            selection.select_item(position, false);
        }
        imp.cursor.set(Some(position));
        imp.anchor.set(Some(position));
        self.show_selection();
    }

    /// The nearest item from the keyboard's place in a direction.
    fn neighbor(&self, from: u32, dx: f64, dy: f64) -> Option<u32> {
        let rects = self.imp().rects.borrow();
        let center = |rect: Rect| (rect.0 + rect.2 / 2.0, rect.1 + rect.3 / 2.0);
        let (fx, fy) = center((*rects.get(from as usize)?)?);
        rects
            .iter()
            .enumerate()
            .filter_map(|(index, rect)| Some((index, center((*rect)?))))
            .filter(|(index, _)| *index as u32 != from)
            .filter_map(|(index, (x, y))| {
                let along = (x - fx) * dx + (y - fy) * dy;
                let across = ((x - fx) * dy - (y - fy) * dx).abs();
                (along > 1.0).then_some((index, along + 2.0 * across))
            })
            .min_by(|p, q| p.1.total_cmp(&q.1))
            .map(|(index, _)| index as u32)
    }

    /// The first or last item in the desktop's order (down, then across).
    fn end_item(&self, last: bool) -> Option<u32> {
        let rects = self.imp().rects.borrow();
        let placed = rects
            .iter()
            .enumerate()
            .filter_map(|(index, rect)| rect.map(|(x, y, _, _)| (index, x, y)));
        let order = |p: &(usize, f64, f64), q: &(usize, f64, f64)| {
            p.1.total_cmp(&q.1).then(p.2.total_cmp(&q.2))
        };
        let found = if last {
            placed.max_by(order)
        } else {
            placed.min_by(order)
        };
        found.map(|(index, _, _)| index as u32)
    }

    fn is_renaming(&self) -> bool {
        self.imp().cells.borrow().iter().any(FileCell::is_renaming)
    }

    fn add_controllers(&self) {
        // Clicks: select, Ctrl and Shift as in a list, double-click opens.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let weak = self.downgrade();
        click.connect_pressed(move |gesture, presses, x, y| {
            let Some(grid) = weak.upgrade() else { return };
            let imp = grid.imp();
            let state = gesture.current_event_state();
            let control = state.contains(gdk::ModifierType::CONTROL_MASK);
            let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
            imp.reselect.set(None);
            if !grid.is_renaming() {
                grid.grab_focus();
            }
            match grid.item_at(x, y) {
                Some(position) => {
                    imp.pressed.set(Some((position, x, y)));
                    if presses == 2 && !control && !shift {
                        grid.activate(position);
                    } else if control {
                        grid.toggle(position);
                    } else if shift {
                        grid.select_range(position);
                    } else if grid.selection().is_selected(position) {
                        imp.reselect.set(Some(position));
                        imp.cursor.set(Some(position));
                        grid.show_selection();
                    } else {
                        grid.select_only(position);
                    }
                }
                None => {
                    imp.pressed.set(None);
                    if !control && !shift {
                        grid.selection().unselect_all();
                    }
                }
            }
        });
        let weak = self.downgrade();
        click.connect_released(move |gesture, presses, x, y| {
            let Some(grid) = weak.upgrade() else { return };
            let imp = grid.imp();
            let here = grid.item_at(x, y);
            if let Some(position) = imp.reselect.take()
                && here == Some(position)
            {
                grid.select_only(position);
            }
            let state = gesture.current_event_state();
            let plain =
                !state.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK);
            if imp.single_click.get()
                && presses == 1
                && plain
                && let Some(position) = here
            {
                grid.activate(position);
            }
        });
        let weak = self.downgrade();
        click.connect_stopped(move |_| {
            if let Some(grid) = weak.upgrade() {
                grid.imp().reselect.set(None);
            }
        });
        self.add_controller(click);

        // A rubber band from empty space.
        let drag = gtk::GestureDrag::new();
        drag.set_button(gdk::BUTTON_PRIMARY);
        let weak = self.downgrade();
        drag.connect_drag_begin(move |gesture, x, y| {
            let Some(grid) = weak.upgrade() else { return };
            let imp = grid.imp();
            if grid.item_at(x, y).is_some() {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            imp.band_start.set(Some((x, y)));
            let control = gesture
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK);
            imp.band_base
                .replace(control.then(|| grid.selection().selection()));
        });
        let weak = self.downgrade();
        drag.connect_drag_update(move |gesture, dx, dy| {
            let Some(grid) = weak.upgrade() else { return };
            let imp = grid.imp();
            let Some((sx, sy)) = imp.band_start.get() else {
                return;
            };
            if imp.band.get().is_none() && dx.abs() < BAND_THRESHOLD && dy.abs() < BAND_THRESHOLD {
                return;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
            let band = (sx.min(sx + dx), sy.min(sy + dy), dx.abs(), dy.abs());
            imp.band.set(Some(band));
            grid.select_in(band);
            grid.queue_draw();
        });
        let weak = self.downgrade();
        drag.connect_drag_end(move |_, _, _| {
            if let Some(grid) = weak.upgrade() {
                let imp = grid.imp();
                imp.band_start.set(None);
                imp.band_base.replace(None);
                if imp.band.take().is_some() {
                    grid.queue_draw();
                }
            }
        });
        self.add_controller(drag);

        // Keys: arrows, Home and End, Enter, Ctrl+Space.
        let keys = gtk::EventControllerKey::new();
        let weak = self.downgrade();
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(grid) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if grid.is_renaming() {
                return glib::Propagation::Proceed;
            }
            grid.key_pressed(key, state)
        });
        self.add_controller(keys);

        let focus = gtk::EventControllerFocus::new();
        let weak = self.downgrade();
        focus.connect_enter(move |_| {
            if let Some(grid) = weak.upgrade() {
                grid.show_selection();
            }
        });
        self.add_controller(focus);

        self.add_drop_target();
    }

    fn key_pressed(&self, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        let imp = self.imp();
        let control = state.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
        let direction = match key {
            gdk::Key::Left | gdk::Key::KP_Left => Some((-1.0, 0.0)),
            gdk::Key::Right | gdk::Key::KP_Right => Some((1.0, 0.0)),
            gdk::Key::Up | gdk::Key::KP_Up => Some((0.0, -1.0)),
            gdk::Key::Down | gdk::Key::KP_Down => Some((0.0, 1.0)),
            _ => None,
        };
        let target = if let Some((dx, dy)) = direction {
            match imp.cursor.get() {
                Some(from) => self.neighbor(from, dx, dy),
                None => self.end_item(false),
            }
        } else if matches!(key, gdk::Key::Home | gdk::Key::KP_Home) {
            self.end_item(false)
        } else if matches!(key, gdk::Key::End | gdk::Key::KP_End) {
            self.end_item(true)
        } else {
            None
        };
        let moving = direction.is_some()
            || matches!(
                key,
                gdk::Key::Home | gdk::Key::KP_Home | gdk::Key::End | gdk::Key::KP_End
            );
        if moving {
            if let Some(target) = target {
                if control {
                    imp.cursor.set(Some(target));
                    self.show_selection();
                } else if shift {
                    self.select_range(target);
                } else {
                    self.select_only(target);
                }
            }
            return glib::Propagation::Stop;
        }
        match key {
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
                let selected = self.selection().selection();
                if selected.size() > 0 {
                    self.activate(imp.cursor.get().unwrap_or_else(|| selected.nth(0)));
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
            gdk::Key::space if control => {
                if let Some(cursor) = imp.cursor.get() {
                    self.toggle(cursor);
                }
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    }

    /// Selects what the rubber band touches (with Ctrl, as well as what was
    /// selected).
    fn select_in(&self, band: Rect) {
        let imp = self.imp();
        let selection = self.selection();
        let n = selection.n_items();
        let selected = imp
            .band_base
            .borrow()
            .as_ref()
            .map(gtk::Bitset::copy)
            .unwrap_or_else(gtk::Bitset::new_empty);
        for (index, rect) in imp.rects.borrow().iter().enumerate() {
            if rect.is_some_and(|rect| overlaps(rect, band)) {
                selected.add(index as u32);
            }
        }
        selection.set_selection(&selected, &gtk::Bitset::new_range(0, n));
    }

    // --- Dropping ------------------------------------------------------------------

    fn add_drop_target(&self) {
        let target = gtk::DropTarget::new(
            gdk::FileList::static_type(),
            gdk::DragAction::COPY | gdk::DragAction::MOVE | gdk::DragAction::LINK,
        );
        target.set_preload(true);
        let weak = self.downgrade();
        target.connect_motion(move |target, x, y| {
            let Some(grid) = weak.upgrade() else {
                return gdk::DragAction::empty();
            };
            let Some(folder) = grid.imp().folder.borrow().clone() else {
                return gdk::DragAction::empty();
            };
            // The Recycle Bin on the panel takes anything.
            if grid.over_bin(x, y) {
                return gdk::DragAction::MOVE;
            }
            if grid.dragged_from_here(target) {
                return gdk::DragAction::MOVE;
            }
            crate::dnd::preferred_action(target, &folder)
        });
        let weak = self.downgrade();
        target.connect_drop(move |target, value, x, y| {
            let Some(grid) = weak.upgrade() else {
                return false;
            };
            let Some(folder) = grid.imp().folder.borrow().clone() else {
                return false;
            };
            let Ok(list) = value.get::<gdk::FileList>() else {
                return false;
            };
            let files = list.files();
            if grid.over_bin(x, y) {
                Self::call(&grid.imp().on_trash, files);
                return true;
            }
            let here = files
                .iter()
                .all(|file| file.parent().is_some_and(|parent| parent.equal(&folder)));
            if here {
                // Icons moved about the desktop.
                grid.move_icons(&files, x, y);
                return true;
            }
            let action = crate::dnd::drop_action(target, &folder);
            let grid_spec = grid.imp().grid.get();
            let (pw, ph) = grid_spec.pitch;
            let place = grid_spec.nearest(x - pw / 2.0, y - ph / 2.0);
            grid.imp()
                .drop_anchor
                .set(Some((place, std::time::Instant::now())));
            let context = grid.imp().context.borrow().clone();
            let on_drop = context.and_then(|context| context.on_drop.borrow().clone());
            if let Some(on_drop) = on_drop {
                on_drop(files, folder, action);
            }
            true
        });
        self.add_controller(target);
    }

    fn dragged_from_here(&self, target: &gtk::DropTarget) -> bool {
        let Some(folder) = self.imp().folder.borrow().clone() else {
            return false;
        };
        target
            .value()
            .and_then(|value| value.get::<gdk::FileList>().ok())
            .is_some_and(|list| {
                let files = list.files();
                !files.is_empty()
                    && files
                        .iter()
                        .all(|file| file.parent().is_some_and(|parent| parent.equal(&folder)))
            })
    }

    /// Whether a point is on the Recycle Bin riding on the panel (it lies
    /// inside the panel's window).
    fn over_bin(&self, x: f64, y: f64) -> bool {
        let imp = self.imp();
        imp.layout.get() == Layout::Panel
            && imp.avoid.get().is_some_and(|(bx, by, width, height)| {
                x >= bx && x < bx + width && y >= by && y < by + height
            })
    }

    /// Icons dragged about the desktop: the one dragged lands where it was
    /// let go, the others keep their places around it. Auto arrange puts
    /// them back in order, as on Windows.
    fn move_icons(&self, files: &[gio::File], x: f64, y: f64) {
        let imp = self.imp();
        if imp.auto_arrange.get() {
            return;
        }
        let names: Vec<String> = files
            .iter()
            .filter_map(|file| file.basename())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        let n = imp.cells.borrow().len() as u32;
        let moving: Vec<u32> = (0..n).filter(|p| names.contains(&self.name(*p))).collect();
        let grid_places = imp.grid_places.borrow().clone();
        let places = imp.places.borrow().clone();
        let lead = imp
            .pressed
            .get()
            .filter(|(position, _, _)| moving.contains(position))
            .or_else(|| moving.first().map(|position| (*position, x, y)));
        let Some((lead, press_x, press_y)) = lead else {
            return;
        };
        let (Some(lead_place), Some((lead_x, lead_y))) = (
            grid_places.get(lead as usize).copied().flatten(),
            places.get(lead as usize).copied().flatten(),
        ) else {
            return;
        };
        let grid = imp.grid.get();
        let target = grid.nearest(x - (press_x - lead_x), y - (press_y - lead_y));
        let (dc, dr) = (target.0 - lead_place.0, target.1 - lead_place.1);
        if (dc, dr) == (0, 0) {
            return;
        }
        let mut taken: HashSet<(i32, i32)> = (0..n)
            .filter(|p| !moving.contains(p))
            .filter_map(|p| grid_places.get(p as usize).copied().flatten())
            .collect();
        let mut saved = imp.saved.borrow().clone();
        for position in &moving {
            let Some((column, row)) = grid_places.get(*position as usize).copied().flatten() else {
                continue;
            };
            let wanted = (
                (column + dc).clamp(0, grid.columns - 1),
                (row + dr).clamp(0, grid.rows - 1),
            );
            let place = self.free_place_near(&grid, wanted, &taken);
            taken.insert(place);
            saved.insert(self.name(*position), place);
        }
        save_places(&saved);
        imp.saved.replace(saved);
        self.arrange();
        self.queue_allocate();
    }

    /// The free place nearest to one wanted.
    fn free_place_near(
        &self,
        grid: &Grid,
        wanted: (i32, i32),
        taken: &HashSet<(i32, i32)>,
    ) -> (i32, i32) {
        let free = |place: (i32, i32)| !taken.contains(&place) && !self.blocked(grid, place);
        if free(wanted) {
            return wanted;
        }
        let reach = grid.columns.max(grid.rows);
        for distance in 1..=reach {
            let mut best: Option<((i32, i32), i32)> = None;
            for column in (wanted.0 - distance)..=(wanted.0 + distance) {
                for row in (wanted.1 - distance)..=(wanted.1 + distance) {
                    if column < 0 || row < 0 || column >= grid.columns || row >= grid.rows {
                        continue;
                    }
                    let place = (column, row);
                    let away = (column - wanted.0).pow(2) + (row - wanted.1).pow(2);
                    if free(place) && best.is_none_or(|(_, b)| away < b) {
                        best = Some((place, away));
                    }
                }
            }
            if let Some((place, _)) = best {
                return place;
            }
        }
        wanted
    }
}

/// The icons' places kept in the settings: name → (column, row).
fn load_places() -> HashMap<String, (i32, i32)> {
    config::settings()
        .value("desktop-positions")
        .get::<HashMap<String, (i32, i32)>>()
        .unwrap_or_default()
}

fn save_places(places: &HashMap<String, (i32, i32)>) {
    if let Err(error) = config::settings().set_value("desktop-positions", &places.to_variant()) {
        eprintln!("patrone-file-manager: could not save the desktop's icon places: {error}");
    }
}
