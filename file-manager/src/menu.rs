//! Windows 11-style menus: a rounded panel with 16-pixel icons, shortcut
//! hints on the right, submenus that open on hover, and (for right-click
//! menus) a row of icon buttons at the top for cut, copy, rename, share and
//! delete. Used for every menu in the app.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk::{gdk, gio, glib, prelude::*};

use crate::glyph::Glyph;
use crate::icons;

/// An item's icon.
#[derive(Clone)]
pub enum Icon {
    None,
    /// A line glyph (`glyph.rs`).
    Glyph(&'static str),
    /// One of the app's color icons (`icons.rs`).
    Color(&'static str),
    /// An app's or a file's own icon.
    Gicon(gio::Icon),
}

type Action = Rc<dyn Fn()>;
type Builder = dyn Fn(&Rc<Menu>);

thread_local! {
    /// Menus that are open; each stays alive until it closes.
    static OPEN: RefCell<Vec<Rc<Menu>>> = const { RefCell::new(Vec::new()) };
}

pub struct Menu {
    pub popover: gtk::Popover,
    list: gtk::Box,
    root: RefCell<Weak<Menu>>,
    open_submenu: RefCell<Option<Rc<Menu>>>,
    hover_timer: RefCell<Option<glib::SourceId>>,
    has_items: Cell<bool>,
}

impl Menu {
    pub fn new() -> Rc<Menu> {
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.add_css_class("menu-list");
        let popover = gtk::Popover::builder()
            .child(&list)
            .has_arrow(false)
            .position(gtk::PositionType::Bottom)
            .halign(gtk::Align::Start)
            .build();
        popover.add_css_class("fluent-menu");
        let menu = Rc::new(Menu {
            popover,
            list,
            root: RefCell::new(Weak::new()),
            open_submenu: RefCell::default(),
            hover_timer: RefCell::default(),
            has_items: Cell::new(false),
        });
        *menu.root.borrow_mut() = Rc::downgrade(&menu);
        menu.add_keys();
        menu
    }

    fn submenu_of(parent: &Rc<Menu>) -> Rc<Menu> {
        let menu = Menu::new();
        *menu.root.borrow_mut() = parent.root.borrow().clone();
        menu.popover.set_position(gtk::PositionType::Right);
        menu.popover.set_halign(gtk::Align::Fill);
        menu.popover.set_valign(gtk::Align::Start);
        menu
    }

    fn add_keys(self: &Rc<Self>) {
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(menu) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            match key {
                gdk::Key::Down => {
                    menu.list.child_focus(gtk::DirectionType::Down);
                    glib::Propagation::Stop
                }
                gdk::Key::Up => {
                    menu.list.child_focus(gtk::DirectionType::Up);
                    glib::Propagation::Stop
                }
                gdk::Key::Left => {
                    // Closing a submenu returns to its parent.
                    let is_submenu = menu
                        .root
                        .borrow()
                        .upgrade()
                        .is_some_and(|root| !Rc::ptr_eq(&root, &menu));
                    if is_submenu {
                        menu.popover.popdown();
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.popover.add_controller(keys);
    }

    /// The row of icon buttons at the top of a right-click menu.
    pub fn icon_row(self: &Rc<Self>, buttons: Vec<(&'static str, &str, bool, Action)>) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        row.add_css_class("menu-icon-row");
        for (glyph, tooltip, enabled, action) in buttons {
            let button = crate::glyph::button(glyph, tooltip);
            button.add_css_class("menu-icon-button");
            button.set_sensitive(enabled);
            self.connect_action(&button, action);
            row.append(&button);
        }
        self.list.append(&row);
        self.separator();
    }

    fn connect_action(self: &Rc<Self>, button: &gtk::Button, action: Action) {
        let weak = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            if let Some(menu) = weak.upgrade() {
                menu.close_all();
            }
            let action = action.clone();
            // After the menu closes, so focus is back where it was.
            glib::idle_add_local_once(move || action());
        });
    }

    fn row(
        &self,
        indicator: Option<&'static str>,
        icon: &Icon,
        label: &str,
        accel: Option<&str>,
    ) -> (gtk::Button, gtk::Box) {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let indicator_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        indicator_slot.add_css_class("menu-indicator");
        if let Some(name) = indicator {
            indicator_slot.append(&Glyph::new(name));
        }
        content.append(&indicator_slot);
        let icon_slot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        icon_slot.add_css_class("menu-icon");
        match icon {
            Icon::None => {}
            Icon::Glyph(name) => icon_slot.append(&Glyph::new(name)),
            Icon::Color(name) => icon_slot.append(&icons::image(name, 16)),
            Icon::Gicon(gicon) => {
                let image = gtk::Image::from_gicon(gicon);
                image.set_pixel_size(16);
                icon_slot.append(&image);
            }
        }
        content.append(&icon_slot);
        let text = gtk::Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .use_underline(false)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(48)
            .build();
        text.add_css_class("menu-label");
        content.append(&text);
        if let Some(accel) = accel {
            let accel = gtk::Label::new(Some(accel));
            accel.add_css_class("menu-accel");
            content.append(&accel);
        }
        let button = gtk::Button::builder().child(&content).build();
        button.add_css_class("flat");
        button.add_css_class("menu-item");
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        self.list.append(&button);
        self.has_items.set(true);
        (button, content)
    }

    /// An ordinary item.
    pub fn item(
        self: &Rc<Self>,
        icon: Icon,
        label: &str,
        accel: Option<&str>,
        enabled: bool,
        action: impl Fn() + 'static,
    ) {
        let (button, _) = self.row(None, &icon, label, accel);
        button.set_sensitive(enabled);
        self.connect_action(&button, Rc::new(action));
        self.close_submenu_on_hover(&button);
    }

    /// An item with a check mark when `active`.
    pub fn check(
        self: &Rc<Self>,
        icon: Icon,
        label: &str,
        accel: Option<&str>,
        active: bool,
        action: impl Fn() + 'static,
    ) {
        let (button, _) = self.row(active.then_some("check"), &icon, label, accel);
        button.set_accessible_role(gtk::AccessibleRole::MenuItemCheckbox);
        button.update_state(&[gtk::accessible::State::Checked(tristate(active))]);
        self.connect_action(&button, Rc::new(action));
        self.close_submenu_on_hover(&button);
    }

    /// One of a group of choices, with a dot when it's the current one.
    pub fn radio(
        self: &Rc<Self>,
        icon: Icon,
        label: &str,
        accel: Option<&str>,
        active: bool,
        action: impl Fn() + 'static,
    ) {
        let (button, _) = self.row(active.then_some("radio"), &icon, label, accel);
        button.set_accessible_role(gtk::AccessibleRole::MenuItemRadio);
        button.update_state(&[gtk::accessible::State::Checked(tristate(active))]);
        self.connect_action(&button, Rc::new(action));
        self.close_submenu_on_hover(&button);
    }

    /// An item that opens a submenu, built when it opens.
    pub fn submenu(
        self: &Rc<Self>,
        icon: Icon,
        label: &str,
        enabled: bool,
        build: impl Fn(&Rc<Menu>) + 'static,
    ) {
        let (button, content) = self.row(None, &icon, label, None);
        button.set_sensitive(enabled);
        let chevron = Glyph::sized("chevron-right", 12);
        chevron.add_css_class("menu-chevron");
        content.append(&chevron);
        let build: Rc<Builder> = Rc::new(build);

        let open = {
            let weak = Rc::downgrade(self);
            let button = button.downgrade();
            let build = build.clone();
            Rc::new(move || {
                let (Some(menu), Some(button)) = (weak.upgrade(), button.upgrade()) else {
                    return;
                };
                if menu.open_submenu.borrow().as_ref().is_some_and(|sub| {
                    sub.popover.parent().as_ref() == Some(button.upcast_ref::<gtk::Widget>())
                }) {
                    return;
                }
                menu.close_submenu();
                let sub = Menu::submenu_of(&menu);
                build(&sub);
                if !sub.has_items.get() {
                    return;
                }
                sub.popover.set_parent(&button);
                {
                    let weak_sub = Rc::downgrade(&sub);
                    sub.popover.connect_closed(move |popover| {
                        let popover = popover.clone();
                        glib::idle_add_local_once(move || popover.unparent());
                        if let Some(sub) = weak_sub.upgrade() {
                            OPEN.with(|open| {
                                open.borrow_mut().retain(|menu| !Rc::ptr_eq(menu, &sub))
                            });
                        }
                    });
                }
                OPEN.with(|open| open.borrow_mut().push(sub.clone()));
                *menu.open_submenu.borrow_mut() = Some(sub.clone());
                sub.popover.popup();
                sub.list.child_focus(gtk::DirectionType::TabForward);
            })
        };
        {
            let open = open.clone();
            button.connect_clicked(move |_| open());
        }
        let motion = gtk::EventControllerMotion::new();
        {
            let weak = Rc::downgrade(self);
            let open = open.clone();
            motion.connect_enter(move |_, _, _| {
                let Some(menu) = weak.upgrade() else { return };
                menu.cancel_hover_timer();
                let open = open.clone();
                let weak = Rc::downgrade(&menu);
                let id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(250),
                    move || {
                        if let Some(menu) = weak.upgrade() {
                            menu.hover_timer.take();
                        }
                        open();
                    },
                );
                *menu.hover_timer.borrow_mut() = Some(id);
            });
        }
        {
            let weak = Rc::downgrade(self);
            motion.connect_leave(move |_| {
                if let Some(menu) = weak.upgrade() {
                    menu.cancel_hover_timer();
                }
            });
        }
        button.add_controller(motion);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Right {
                open();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        button.add_controller(keys);
    }

    fn cancel_hover_timer(&self) {
        if let Some(id) = self.hover_timer.take() {
            id.remove();
        }
    }

    fn close_submenu(&self) {
        if let Some(sub) = self.open_submenu.take() {
            sub.close_submenu();
            sub.popover.popdown();
        }
    }

    fn close_submenu_on_hover(self: &Rc<Self>, button: &gtk::Button) {
        let motion = gtk::EventControllerMotion::new();
        let weak = Rc::downgrade(self);
        motion.connect_enter(move |_, _, _| {
            if let Some(menu) = weak.upgrade() {
                menu.cancel_hover_timer();
                if menu.open_submenu.borrow().is_some() {
                    let weak = Rc::downgrade(&menu);
                    let id = glib::timeout_add_local_once(
                        std::time::Duration::from_millis(250),
                        move || {
                            if let Some(menu) = weak.upgrade() {
                                menu.hover_timer.take();
                                menu.close_submenu();
                            }
                        },
                    );
                    *menu.hover_timer.borrow_mut() = Some(id);
                }
            }
        });
        button.add_controller(motion);
    }

    pub fn separator(&self) {
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        separator.add_css_class("menu-separator");
        self.list.append(&separator);
    }

    /// A small heading over a group of items.
    pub fn heading(&self, text: &str) {
        let label = gtk::Label::builder().label(text).xalign(0.0).build();
        label.add_css_class("menu-heading");
        self.list.append(&label);
    }

    pub fn is_empty(&self) -> bool {
        !self.has_items.get()
    }

    /// Closes this menu and every menu it opened, back to the first one.
    pub fn close_all(&self) {
        let root = self.root.borrow().upgrade();
        match root {
            Some(root) => {
                root.close_submenu();
                root.popover.popdown();
            }
            None => self.popover.popdown(),
        }
    }

    fn show(self: &Rc<Self>, parent: &impl IsA<gtk::Widget>) {
        self.popover.set_parent(parent);
        let weak = Rc::downgrade(self);
        self.popover.connect_closed(move |popover| {
            let popover = popover.clone();
            // GTK is still finishing the close when this runs.
            glib::idle_add_local_once(move || popover.unparent());
            if let Some(menu) = weak.upgrade() {
                menu.close_submenu();
                OPEN.with(|open| open.borrow_mut().retain(|open| !Rc::ptr_eq(open, &menu)));
            }
        });
        OPEN.with(|open| open.borrow_mut().push(self.clone()));
        self.popover.popup();
    }

    /// Opens at a point in `parent`'s coordinates, as right-click menus do.
    pub fn popup_at(self: &Rc<Self>, parent: &impl IsA<gtk::Widget>, x: f64, y: f64) {
        self.popover
            .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        self.show(parent);
    }

    /// Opens under a button, as dropdown menus do.
    pub fn popup_below(self: &Rc<Self>, button: &impl IsA<gtk::Widget>) {
        self.show(button);
    }
}

/// A button that opens a menu built fresh each time.
pub fn dropdown_button(
    content: &impl IsA<gtk::Widget>,
    tooltip: &str,
    build: impl Fn(&Rc<Menu>) + 'static,
) -> gtk::Button {
    let button = gtk::Button::builder()
        .child(content)
        .tooltip_text(tooltip)
        .build();
    button.add_css_class("flat");
    button.update_property(&[gtk::accessible::Property::HasPopup(true)]);
    button.connect_clicked(move |button| {
        let menu = Menu::new();
        build(&menu);
        if !menu.is_empty() {
            menu.popup_below(button);
        }
    });
    button
}

fn tristate(active: bool) -> gtk::AccessibleTristate {
    if active {
        gtk::AccessibleTristate::True
    } else {
        gtk::AccessibleTristate::False
    }
}
