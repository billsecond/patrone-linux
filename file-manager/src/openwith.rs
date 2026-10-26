//! Choosing an app for a file, as Windows 11 asks: "Select an app to open
//! this .txt file", with "Just once" and "Always".

use std::rc::Rc;

use gtk::{gio, glib, prelude::*};

use crate::commands;
use crate::dialogs::Dialog;
use crate::util;
use crate::window::Window;

const ALWAYS: i32 = 0;
const JUST_ONCE: i32 = 1;

fn app_row(app: &gio::AppInfo) -> gtk::ListBoxRow {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("app-row");
    let image = match app.icon() {
        Some(icon) => gtk::Image::from_gicon(&icon),
        None => gtk::Image::from_icon_name("application-x-executable"),
    };
    image.set_pixel_size(32);
    row.append(&image);
    let name = gtk::Label::builder()
        .label(app.display_name().as_str())
        .xalign(0.0)
        .hexpand(true)
        .build();
    row.append(&name);
    gtk::ListBoxRow::builder().child(&row).build()
}

/// The apps to offer: the default and recommended ones first.
fn apps_for(mime: &str) -> (Vec<gio::AppInfo>, Vec<gio::AppInfo>) {
    let mut suggested = gio::AppInfo::recommended_for_type(mime);
    if let Some(default) = gio::AppInfo::default_for_type(mime, false) {
        suggested.retain(|app| !app.equal(&default));
        suggested.insert(0, default);
    }
    let mut more: Vec<gio::AppInfo> = gio::AppInfo::all()
        .into_iter()
        .filter(|app| app.should_show() && !suggested.iter().any(|known| known.equal(app)))
        .collect();
    more.sort_by(|a, b| util::natural_cmp(&a.display_name(), &b.display_name()));
    (suggested, more)
}

/// Opens `files` with an app the user picks.
pub fn show(window: &Window, files: Vec<gio::File>, mime: String) {
    run(window, files, mime, false);
}

/// From Properties' "Change...": picks the app that always opens this type.
pub fn show_choose_default(window: &Window, files: Vec<gio::File>, mime: String) {
    run(window, files, mime, true);
}

fn run(window: &Window, files: Vec<gio::File>, mime: String, default_only: bool) {
    let name = files
        .first()
        .and_then(|f| f.basename())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (_, extension) = util::split_extension(&name);
    let what = if extension.is_empty() {
        "file".to_owned()
    } else {
        format!("{extension} file")
    };
    let dialog = Dialog::new(Some(&window.gtk_window()), "Open with");
    dialog.window.set_default_size(420, 520);
    dialog.window.set_resizable(true);
    let heading = gtk::Label::builder()
        .label(format!("Select an app to open this {what}"))
        .xalign(0.0)
        .wrap(true)
        .width_chars(30)
        .build();
    heading.add_css_class("dialog-heading");
    dialog.body.append(&heading);
    let (suggested, more) = apps_for(&mime);
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .build();
    list.add_css_class("app-list");
    let mut rows: Vec<(gtk::ListBoxRow, gio::AppInfo)> = Vec::new();
    if !suggested.is_empty() {
        let label = gtk::Label::builder()
            .label("Suggested apps")
            .xalign(0.0)
            .build();
        label.add_css_class("options-heading");
        let row = gtk::ListBoxRow::builder()
            .child(&label)
            .selectable(false)
            .activatable(false)
            .build();
        list.append(&row);
    }
    for app in &suggested {
        let row = app_row(app);
        list.append(&row);
        rows.push((row, app.clone()));
    }
    let more_label = gtk::Label::builder().label("More apps").xalign(0.0).build();
    more_label.add_css_class("options-heading");
    list.append(
        &gtk::ListBoxRow::builder()
            .child(&more_label)
            .selectable(false)
            .activatable(false)
            .build(),
    );
    for app in &more {
        let row = app_row(app);
        list.append(&row);
        rows.push((row, app.clone()));
    }
    if let Some((first, _)) = rows.first() {
        list.select_row(Some(first));
    }
    let scroller = gtk::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .min_content_height(300)
        .build();
    dialog.body.append(&scroller);
    if default_only {
        dialog.add_button("OK", ALWAYS, true);
        dialog.add_button("Cancel", -1, false);
    } else {
        dialog.add_button("_Always", ALWAYS, true);
        dialog.add_button("_Just once", JUST_ONCE, false);
    }
    {
        let dialog = Rc::downgrade(&dialog);
        list.connect_row_activated(move |_, _| {
            if let Some(dialog) = dialog.upgrade() {
                dialog.respond(if default_only { ALWAYS } else { JUST_ONCE });
            }
        });
    }
    let window = window.clone();
    glib::spawn_future_local(async move {
        let response = dialog.run().await;
        if response != ALWAYS && response != JUST_ONCE {
            return;
        }
        let Some(selected) = list.selected_row() else {
            return;
        };
        let Some(app) = rows
            .iter()
            .find(|(row, _)| *row == selected)
            .map(|(_, app)| app.clone())
        else {
            return;
        };
        if response == ALWAYS
            && let Err(error) = app.set_as_default_for_type(&mime)
        {
            crate::dialogs::error(
                Some(&window.gtk_window()),
                "Open with",
                "The default app couldn't be changed.",
                &error.to_string(),
            );
        }
        if !default_only {
            commands::open_with(&window, &app, &files);
        }
    });
}
