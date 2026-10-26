//! The command bar: New; Cut, Copy, Paste, Rename, Share and Delete; Sort,
//! View and Filter; buttons for what's selected (Empty Recycle Bin,
//! Extract all, Set as background...); "See more"; and Details on the right.

use std::rc::Rc;

use gtk::prelude::*;

use crate::glyph::{self, Glyph};
use crate::menu::{self, Menu};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuKind {
    New,
    Sort,
    View,
    Filter,
    More,
}

/// Fills a dropdown menu when it opens.
pub type MenuFiller = dyn Fn(MenuKind, &Rc<Menu>);

/// A button for the bar's middle section, shown for some selections.
pub struct Contextual {
    pub glyph: &'static str,
    pub label: &'static str,
    pub action: &'static str,
}

pub struct CommandBar {
    pub widget: gtk::Box,
    contextual: gtk::Box,
    contextual_separator: gtk::Separator,
    filter_button: gtk::Button,
}

fn labelled(glyph_name: &str, label: &str, dropdown: bool) -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&Glyph::new(glyph_name));
    content.append(&gtk::Label::new(Some(label)));
    if dropdown {
        let chevron = Glyph::sized("chevron-down", 12);
        chevron.add_css_class("dropdown-chevron");
        content.append(&chevron);
    }
    content
}

fn separator() -> gtk::Separator {
    let separator = gtk::Separator::new(gtk::Orientation::Vertical);
    separator.add_css_class("command-separator");
    separator
}

impl CommandBar {
    /// `menus` fills a dropdown when it opens.
    pub fn new(menus: Rc<MenuFiller>) -> CommandBar {
        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        widget.add_css_class("command-bar");

        let dropdown = |kind: MenuKind, glyph: &str, label: &str, tooltip: &str| {
            let menus = menus.clone();
            let button =
                menu::dropdown_button(&labelled(glyph, label, true), tooltip, move |menu| {
                    menus(kind, menu)
                });
            button.add_css_class("command");
            button
        };
        let new = dropdown(
            MenuKind::New,
            "new",
            "New",
            "Create a new item in the current location.",
        );
        new.add_css_class("command-new");
        widget.append(&new);
        widget.append(&separator());

        for (glyph, tooltip, action) in [
            ("cut", "Cut (Ctrl+X)", "win.cut"),
            ("copy", "Copy (Ctrl+C)", "win.copy"),
            ("paste", "Paste (Ctrl+V)", "win.paste"),
            ("rename", "Rename (F2)", "win.rename"),
            ("share", "Share", "win.share"),
            ("delete", "Delete (Delete)", "win.delete"),
        ] {
            let button = glyph::button(glyph, tooltip);
            button.add_css_class("command");
            button.add_css_class("command-icon");
            button.set_action_name(Some(action));
            widget.append(&button);
        }
        widget.append(&separator());
        widget.append(&dropdown(MenuKind::Sort, "sort", "Sort", "Sort the items."));
        widget.append(&dropdown(
            MenuKind::View,
            "view",
            "View",
            "Change how items are shown.",
        ));
        let filter_button = dropdown(
            MenuKind::Filter,
            "filter",
            "Filter",
            "Show only some kinds of items.",
        );
        widget.append(&filter_button);

        let contextual_separator = separator();
        contextual_separator.set_visible(false);
        widget.append(&contextual_separator);
        let contextual = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        widget.append(&contextual);
        widget.append(&separator());

        let more = {
            let menus = menus.clone();
            menu::dropdown_button(&Glyph::new("more"), "See more", move |menu| {
                menus(MenuKind::More, menu)
            })
        };
        more.add_css_class("command");
        more.add_css_class("command-icon");
        widget.append(&more);

        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        widget.append(&spacer);

        let details = gtk::ToggleButton::builder()
            .child(&labelled("details-pane", "Details", false))
            .tooltip_text("Show the details pane (Alt+Shift+P)")
            .action_name("win.details-pane")
            .build();
        details.add_css_class("flat");
        details.add_css_class("command");
        widget.append(&details);

        CommandBar {
            widget,
            contextual,
            contextual_separator,
            filter_button,
        }
    }

    /// Sets the buttons for the current selection.
    pub fn set_contextual(&self, buttons: &[Contextual]) {
        while let Some(child) = self.contextual.first_child() {
            self.contextual.remove(&child);
        }
        for button in buttons {
            let widget = gtk::Button::builder()
                .child(&labelled(button.glyph, button.label, false))
                .action_name(button.action)
                .build();
            widget.add_css_class("flat");
            widget.add_css_class("command");
            self.contextual.append(&widget);
        }
        self.contextual_separator.set_visible(!buttons.is_empty());
    }

    /// Filter is offered where it means something.
    pub fn set_filter_visible(&self, visible: bool) {
        self.filter_button.set_visible(visible);
    }
}
