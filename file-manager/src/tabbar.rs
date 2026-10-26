//! The title bar: the tabs, a button for a new tab, and the window's
//! minimize, maximize and close buttons, as in Windows 11's File Explorer.
//! Dragging the empty space moves the window; double-clicking maximizes.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk::{gdk, glib, prelude::*};

use crate::glyph::{self, Glyph};
use crate::icons;

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
    /// A tab was dragged to a new place.
    Move {
        from: usize,
        to: usize,
    },
}

struct Item {
    widget: gtk::Box,
    icon: gtk::Image,
    label: gtk::Label,
}

pub struct TabBar {
    pub widget: gtk::WindowHandle,
    tabs: gtk::Box,
    items: RefCell<Vec<Item>>,
    maximize: gtk::Button,
    events: crate::util::Handler<TabBarEvent>,
    me: RefCell<Weak<TabBar>>,
}

impl TabBar {
    pub fn new(window: &gtk::ApplicationWindow) -> Rc<TabBar> {
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tabs.add_css_class("tabs");
        let scroller = gtk::ScrolledWindow::builder()
            .child(&tabs)
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_width(true)
            .build();
        let new_tab = glyph::button("add", "New tab (Ctrl+T)");
        new_tab.add_css_class("new-tab");
        new_tab.set_valign(gtk::Align::Center);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        let minimize = glyph::button("minimize", "Minimize");
        let maximize = glyph::button("maximize", "Maximize");
        let close = glyph::button("close", "Close");
        for button in [&minimize, &maximize] {
            button.add_css_class("window-control");
        }
        close.add_css_class("window-close");
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.add_css_class("tab-bar");
        bar.append(&scroller);
        bar.append(&new_tab);
        bar.append(&spacer);
        bar.append(&minimize);
        bar.append(&maximize);
        bar.append(&close);
        let widget = gtk::WindowHandle::builder().child(&bar).build();

        let tab_bar = Rc::new(TabBar {
            widget,
            tabs,
            items: RefCell::default(),
            maximize: maximize.clone(),
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
        {
            let window = window.clone();
            minimize.connect_clicked(move |_| window.minimize());
        }
        {
            let window = window.clone();
            maximize.connect_clicked(move |_| {
                if window.is_maximized() {
                    window.unmaximize();
                } else {
                    window.maximize();
                }
            });
        }
        {
            let window = window.clone();
            close.connect_clicked(move |_| window.close());
        }
        let weak = Rc::downgrade(&tab_bar);
        window.connect_maximized_notify(move |window| {
            if let Some(bar) = weak.upgrade() {
                let maximized = window.is_maximized();
                if let Some(glyph) = bar.maximize.child().and_downcast::<Glyph>() {
                    glyph.set_name(if maximized { "unmaximize" } else { "maximize" });
                }
                bar.maximize.set_tooltip_text(Some(if maximized {
                    "Restore Down"
                } else {
                    "Maximize"
                }));
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

    /// Adds a tab at `index`.
    pub fn insert(&self, index: usize, title: &str, icon: &str) {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        widget.add_css_class("tab");
        let image = icons::image(icon, 16);
        let label = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(22)
            .width_chars(14)
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
                gdk::BUTTON_PRIMARY => bar.emit(TabBarEvent::Select(index)),
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

        // Tabs can be dragged to a new place.
        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::MOVE);
        let weak = self.me.borrow().clone();
        let tab = widget.clone();
        source.connect_prepare(move |_, _, _| {
            let bar = weak.upgrade()?;
            let index = bar.index_of(&tab)?;
            Some(gdk::ContentProvider::for_value(
                &format!("pfm-tab:{index}").to_value(),
            ))
        });
        widget.add_controller(source);
        let target = gtk::DropTarget::new(glib::types::Type::STRING, gdk::DragAction::MOVE);
        let weak = self.me.borrow().clone();
        let tab = widget.clone();
        target.connect_drop(move |_, value, _, _| {
            let Some(bar) = weak.upgrade() else {
                return false;
            };
            let Some(from) = value.get::<String>().ok().and_then(|text| {
                text.strip_prefix("pfm-tab:")
                    .and_then(|n| n.parse::<usize>().ok())
            }) else {
                return false;
            };
            let Some(to) = bar.index_of(&tab) else {
                return false;
            };
            if from != to {
                bar.emit(TabBarEvent::Move { from, to });
            }
            true
        });
        widget.add_controller(target);

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
