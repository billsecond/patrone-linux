//! The title bar: the tabs, a button for a new tab, and the window's
//! minimize, maximize and close buttons, as in Windows 11's File Explorer.
//! Those are GTK's own window controls, so like every other app's they go
//! in the corner the account chose (GNOME's button-layout) and look the way
//! Patrone Settings' title bar button style says. Dragging the empty space
//! moves the window; double-clicking maximizes.
//!
//! Dragging a tab:
//! - when it's the window's only tab, moves the window, like the title bar;
//! - sideways along the tabs, reorders them;
//! - out of the tab strip (or with Ctrl held), picks the tab up: dropped on
//!   another File Manager window's tabs it joins that window, dropped
//!   anywhere else it opens in a window of its own (`window.rs`).
//!
//! Files dragged over a tab switch to it after a moment, and files dropped
//! on a tab go into that tab's folder.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gtk::{gdk, gio, glib, graphene, prelude::*};

use crate::glyph::{self, Glyph};
use crate::icons;
use crate::util;

pub enum TabBarEvent {
    Select(usize),
    Close(usize),
    New,
    Menu {
        index: usize,
        widget: gtk::Widget,
        x: f64,
        y: f64,
    },
    /// A tab was dragged along the strip to a new place.
    Move {
        from: usize,
        to: usize,
    },
    /// A tab was pulled out of the strip: start dragging it between windows.
    /// `x` and `y` are where the drag started, in the window surface's
    /// coordinates.
    PickUp {
        index: usize,
        device: gdk::Device,
        x: f64,
        y: f64,
    },
    /// Files were dropped on a tab.
    DropFiles {
        index: usize,
        files: Vec<gio::File>,
        action: gdk::DragAction,
    },
}

/// What a press on a tab turns into once the pointer moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DragMode {
    Undecided,
    Reorder,
    Done,
}

/// How far the pointer moves before a press becomes a drag.
const THRESHOLD: f64 = 8.0;
/// How far out of the strip a tab is pulled before it's picked up.
const PULL_OUT: f64 = 36.0;

struct Item {
    widget: gtk::Box,
    icon: gtk::Image,
    label: gtk::Label,
}

pub struct TabBar {
    pub widget: gtk::WindowHandle,
    tabs: gtk::Box,
    items: RefCell<Vec<Item>>,
    drag_mode: Cell<DragMode>,
    hover_timer: RefCell<Option<glib::SourceId>>,
    events: crate::util::Handler<TabBarEvent>,
    me: RefCell<Weak<TabBar>>,
}

impl TabBar {
    pub fn new() -> Rc<TabBar> {
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tabs.add_css_class("tabs");
        // Tabs keep their width until the strip is full, then shrink; past
        // their smallest they scroll.
        let scroller = gtk::ScrolledWindow::builder()
            .child(&tabs)
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_width(true)
            .hexpand(false)
            .build();
        let new_tab = glyph::button("add", "New tab (Ctrl+T)");
        new_tab.add_css_class("new-tab");
        new_tab.set_valign(gtk::Align::Center);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.add_css_class("tab-bar");
        // The window's buttons before the tabs or after the spacer, wherever
        // the account's button-layout puts them (each set is empty, and
        // hidden, when it puts none there)
        bar.append(&util::window_controls(gtk::PackType::Start));
        bar.append(&scroller);
        bar.append(&new_tab);
        bar.append(&spacer);
        bar.append(&util::window_controls(gtk::PackType::End));
        let widget = gtk::WindowHandle::builder().child(&bar).build();

        let tab_bar = Rc::new(TabBar {
            widget,
            tabs,
            items: RefCell::default(),
            drag_mode: Cell::new(DragMode::Done),
            hover_timer: RefCell::default(),
            events: RefCell::default(),
            me: RefCell::default(),
        });
        *tab_bar.me.borrow_mut() = Rc::downgrade(&tab_bar);
        let weak = Rc::downgrade(&tab_bar);
        new_tab.connect_clicked(move |_| {
            if let Some(bar) = weak.upgrade() {
                bar.emit(TabBarEvent::New);
            }
        });
        tab_bar
    }

    pub fn connect_events(&self, events: impl Fn(TabBarEvent) + 'static) {
        *self.events.borrow_mut() = Some(Rc::new(events));
    }

    fn emit(&self, event: TabBarEvent) {
        let events = self.events.borrow().clone();
        if let Some(events) = events {
            events(event);
        }
    }

    fn index_of(&self, widget: &gtk::Box) -> Option<usize> {
        self.items
            .borrow()
            .iter()
            .position(|item| &item.widget == widget)
    }

    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// The tab widget at `index`, for the drag icon.
    pub fn tab_widget(&self, index: usize) -> Option<gtk::Box> {
        self.items
            .borrow()
            .get(index)
            .map(|item| item.widget.clone())
    }

    /// Adds a tab at `index`.
    pub fn insert(&self, index: usize, title: &str, icon: &str) {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        widget.add_css_class("tab");
        let image = icons::image(icon, 16);
        // A fixed width, as Windows' tabs have; long titles are cut short.
        let label = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .width_chars(20)
            .max_width_chars(20)
            .build();
        let close = glyph::button("close", "Close tab (Ctrl+W)");
        close.add_css_class("tab-close");
        if let Some(glyph) = close.child().and_downcast::<Glyph>() {
            glyph.set_size(12);
        }
        widget.append(&image);
        widget.append(&label);
        widget.append(&close);
        widget.set_tooltip_text(Some(title));

        let weak = self.me.borrow().clone();
        let tab = widget.clone();
        close.connect_clicked(move |_| {
            let Some(bar) = weak.upgrade() else { return };
            if let Some(index) = bar.index_of(&tab) {
                bar.emit(TabBarEvent::Close(index));
            }
        });

        // Middle-click closes; right-click selects and opens the tab menu.
        let click = gtk::GestureClick::new();
        click.set_button(0);
        let weak = self.me.borrow().clone();
        let tab = widget.clone();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(bar) = weak.upgrade() else { return };
            let Some(index) = bar.index_of(&tab) else {
                return;
            };
            match gesture.current_button() {
                gdk::BUTTON_MIDDLE => bar.emit(TabBarEvent::Close(index)),
                gdk::BUTTON_SECONDARY => {
                    bar.emit(TabBarEvent::Select(index));
                    bar.emit(TabBarEvent::Menu {
                        index,
                        widget: tab.clone().upcast(),
                        x,
                        y,
                    });
                }
                _ => return,
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });
        widget.add_controller(click);

        // A press selects; dragging moves the window, reorders or picks up.
        let drag = gtk::GestureDrag::new();
        drag.set_button(gdk::BUTTON_PRIMARY);
        let weak = self.me.borrow().clone();
        let tab = widget.clone();
        drag.connect_drag_begin(move |_, x, y| {
            let Some(bar) = weak.upgrade() else { return };
            let Some(index) = bar.index_of(&tab) else {
                return;
            };
            // A press on the close button is the button's: claiming it here
            // would cancel the button's click.
            let on_button = tab
                .pick(x, y, gtk::PickFlags::DEFAULT)
                .is_some_and(|widget| widget.ancestor(gtk::Button::static_type()).is_some());
            if on_button {
                bar.drag_mode.set(DragMode::Done);
                return;
            }
            bar.drag_mode.set(DragMode::Undecided);
            bar.emit(TabBarEvent::Select(index));
        });
        let weak = self.me.borrow().clone();
        let tab = widget.clone();
        drag.connect_drag_update(move |gesture, dx, dy| {
            if let Some(bar) = weak.upgrade() {
                bar.drag_update(gesture, &tab, dx, dy);
            }
        });
        let weak = self.me.borrow().clone();
        drag.connect_drag_end(move |_, _, _| {
            if let Some(bar) = weak.upgrade() {
                bar.drag_mode.set(DragMode::Done);
            }
        });
        widget.add_controller(drag);

        // Files dragged over a tab switch to it; dropped, they go into it.
        let files = gtk::DropTarget::new(
            gdk::FileList::static_type(),
            gdk::DragAction::COPY | gdk::DragAction::MOVE,
        );
        let weak = self.me.borrow().clone();
        let tab = widget.clone();
        files.connect_enter(move |_, _, _| {
            if let Some(bar) = weak.upgrade() {
                bar.hover_switch(&tab);
            }
            gdk::DragAction::COPY
        });
        let weak = self.me.borrow().clone();
        files.connect_leave(move |_| {
            if let Some(bar) = weak.upgrade()
                && let Some(timer) = bar.hover_timer.take()
            {
                timer.remove();
            }
        });
        let weak = self.me.borrow().clone();
        let tab = widget.clone();
        files.connect_drop(move |target, value, _, _| {
            let Some(bar) = weak.upgrade() else {
                return false;
            };
            if let Some(timer) = bar.hover_timer.take() {
                timer.remove();
            }
            let (Some(index), Ok(list)) = (bar.index_of(&tab), value.get::<gdk::FileList>()) else {
                return false;
            };
            let state = target.current_event_state();
            let action = if state.contains(gdk::ModifierType::CONTROL_MASK) {
                gdk::DragAction::COPY
            } else if state.contains(gdk::ModifierType::SHIFT_MASK) {
                gdk::DragAction::MOVE
            } else {
                // The window applies Windows' same-drive rule.
                gdk::DragAction::empty()
            };
            bar.emit(TabBarEvent::DropFiles {
                index,
                files: list.files(),
                action,
            });
            true
        });
        widget.add_controller(files);

        let mut items = self.items.borrow_mut();
        let index = index.min(items.len());
        let sibling = if index == 0 {
            None
        } else {
            Some(items[index - 1].widget.clone())
        };
        self.tabs.insert_child_after(&widget, sibling.as_ref());
        items.insert(
            index,
            Item {
                widget,
                icon: image,
                label,
            },
        );
    }

    fn hover_switch(&self, tab: &gtk::Box) {
        if let Some(timer) = self.hover_timer.take() {
            timer.remove();
        }
        let weak = self.me.borrow().clone();
        let tab = tab.clone();
        let timer = glib::timeout_add_local_once(Duration::from_millis(600), move || {
            let Some(bar) = weak.upgrade() else { return };
            bar.hover_timer.take();
            if let Some(index) = bar.index_of(&tab) {
                bar.emit(TabBarEvent::Select(index));
            }
        });
        self.hover_timer.replace(Some(timer));
    }

    fn drag_update(&self, gesture: &gtk::GestureDrag, tab: &gtk::Box, dx: f64, dy: f64) {
        match self.drag_mode.get() {
            DragMode::Done => {}
            DragMode::Undecided => {
                if dx.hypot(dy) < THRESHOLD {
                    return;
                }
                // Claimed only now, so clicks still reach the close button,
                // and before the title bar can start moving the window.
                gesture.set_state(gtk::EventSequenceState::Claimed);
                let ctrl = gesture
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK);
                if ctrl {
                    self.pick_up(gesture, tab);
                } else if self.len() == 1 {
                    self.move_window(gesture, tab);
                } else if dy.abs() > dx.abs() {
                    self.pick_up(gesture, tab);
                } else {
                    self.drag_mode.set(DragMode::Reorder);
                    self.reorder(gesture, tab, dx);
                }
            }
            DragMode::Reorder => {
                if dy.abs() > PULL_OUT {
                    self.pick_up(gesture, tab);
                } else {
                    self.reorder(gesture, tab, dx);
                }
            }
        }
    }

    /// The drag's start point in the window surface's coordinates.
    fn surface_point(&self, gesture: &gtk::GestureDrag, tab: &gtk::Box) -> Option<(f64, f64)> {
        let (x, y) = gesture.start_point()?;
        let native = tab.native()?;
        let point = tab.compute_point(&native, &graphene::Point::new(x as f32, y as f32))?;
        let (tx, ty) = native.surface_transform();
        Some((point.x() as f64 + tx, point.y() as f64 + ty))
    }

    /// With one tab, a tab drag is a title bar drag: the window moves.
    fn move_window(&self, gesture: &gtk::GestureDrag, tab: &gtk::Box) {
        self.drag_mode.set(DragMode::Done);
        let (Some((x, y)), Some(device)) = (self.surface_point(gesture, tab), gesture.device())
        else {
            return;
        };
        let toplevel = tab
            .native()
            .and_then(|native| native.surface())
            .and_downcast::<gdk::Toplevel>();
        if let Some(toplevel) = toplevel {
            toplevel.begin_move(
                &device,
                gesture.current_button() as i32,
                x,
                y,
                gesture.current_event_time(),
            );
        }
        gesture.reset();
    }

    fn pick_up(&self, gesture: &gtk::GestureDrag, tab: &gtk::Box) {
        self.drag_mode.set(DragMode::Done);
        let Some(index) = self.index_of(tab) else {
            return;
        };
        let (Some((x, y)), Some(device)) = (self.surface_point(gesture, tab), gesture.device())
        else {
            return;
        };
        gesture.reset();
        self.emit(TabBarEvent::PickUp {
            index,
            device,
            x,
            y,
        });
    }

    /// Moves the dragged tab past the neighbors whose middle it has crossed.
    fn reorder(&self, gesture: &gtk::GestureDrag, tab: &gtk::Box, dx: f64) {
        let Some((start_x, _)) = gesture.start_point() else {
            return;
        };
        let Some(from) = self.index_of(tab) else {
            return;
        };
        // GTK measures the offset in the tab's own coordinates, and the tab
        // moves as it's reordered, so this is the pointer over the tab now.
        let Some(point) = tab.compute_point(
            &self.tabs,
            &graphene::Point::new((start_x + dx) as f32, 0.0),
        ) else {
            return;
        };
        let to = self.index_at(point.x() as f64, from);
        if to != from {
            self.emit(TabBarEvent::Move { from, to });
        }
    }

    /// Which tab's place a point (in the strip's coordinates) is in, for
    /// reordering: a neighbor's place is taken once its middle is passed.
    fn index_at(&self, x: f64, from: usize) -> usize {
        let items = self.items.borrow();
        let mut target = from;
        for (index, item) in items.iter().enumerate() {
            let Some(bounds) = item.widget.compute_bounds(&self.tabs) else {
                continue;
            };
            let middle = (bounds.x() + bounds.width() / 2.0) as f64;
            if index > from && x > middle {
                target = index;
            }
            if index < from && x < middle && target == from {
                target = index;
            }
        }
        target
    }

    /// Where a tab dropped at `x` (in the title bar's coordinates) goes.
    pub fn insertion_index(&self, x: f64) -> usize {
        let Some(point) = self
            .widget
            .compute_point(&self.tabs, &graphene::Point::new(x as f32, 0.0))
        else {
            return self.len();
        };
        let x = point.x() as f64;
        let items = self.items.borrow();
        for (index, item) in items.iter().enumerate() {
            if let Some(bounds) = item.widget.compute_bounds(&self.tabs)
                && x < (bounds.x() + bounds.width() / 2.0) as f64
            {
                return index;
            }
        }
        items.len()
    }

    /// Shows where a dropped tab would go (`None` hides the marker).
    pub fn show_drop_marker(&self, index: Option<usize>) {
        let items = self.items.borrow();
        for (position, item) in items.iter().enumerate() {
            item.widget.remove_css_class("drop-before");
            item.widget.remove_css_class("drop-after");
            match index {
                Some(index) if index == position => item.widget.add_css_class("drop-before"),
                Some(index) if index == items.len() && position + 1 == items.len() => {
                    item.widget.add_css_class("drop-after")
                }
                _ => {}
            }
        }
    }

    /// Fades the tab being carried to another window.
    pub fn set_carried(&self, index: Option<usize>) {
        for (position, item) in self.items.borrow().iter().enumerate() {
            if Some(position) == index {
                item.widget.add_css_class("carried");
            } else {
                item.widget.remove_css_class("carried");
            }
        }
    }

    pub fn remove(&self, index: usize) {
        let mut items = self.items.borrow_mut();
        if index < items.len() {
            let item = items.remove(index);
            self.tabs.remove(&item.widget);
        }
    }

    pub fn move_tab(&self, from: usize, to: usize) {
        let mut items = self.items.borrow_mut();
        if from >= items.len() || to >= items.len() {
            return;
        }
        let item = items.remove(from);
        items.insert(to, item);
        let sibling = if to == 0 {
            None
        } else {
            Some(items[to - 1].widget.clone())
        };
        self.tabs
            .reorder_child_after(&items[to].widget, sibling.as_ref());
    }

    pub fn set(&self, index: usize, title: &str, icon: &str) {
        if let Some(item) = self.items.borrow().get(index) {
            item.label.set_label(title);
            item.widget.set_tooltip_text(Some(title));
            icons::set_image(&item.icon, icon, 16);
        }
    }

    pub fn set_active(&self, index: usize) {
        for (position, item) in self.items.borrow().iter().enumerate() {
            if position == index {
                item.widget.add_css_class("active");
            } else {
                item.widget.remove_css_class("active");
            }
        }
    }
}
