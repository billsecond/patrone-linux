//! The Preferences window. Changes apply to every open terminal at once.

use gtk::{pango, prelude::*};

use crate::{settings, window};

pub fn show(parent: &gtk::ApplicationWindow) {
    let current = settings::get();
    let Some(app) = parent.application() else {
        return;
    };

    let font = gtk::FontDialogButton::builder()
        .dialog(&gtk::FontDialog::builder().title("Terminal Font").build())
        .font_desc(&pango::FontDescription::from_string(&current.font))
        .level(gtk::FontLevel::Font)
        .use_font(true)
        .valign(gtk::Align::Center)
        .build();
    let scrollback = gtk::SpinButton::with_range(1_000.0, 1_000_000.0, 1_000.0);
    scrollback.set_value(current.scrollback_lines as f64);
    scrollback.set_valign(gtk::Align::Center);
    let copy_on_select = gtk::Switch::builder()
        .active(current.copy_on_select)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    let paste_on_right_click = gtk::Switch::builder()
        .active(current.paste_on_right_click)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();

    let grid = gtk::Grid::builder()
        .row_spacing(14)
        .column_spacing(32)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    let rows: [(&str, &gtk::Widget); 4] = [
        ("Font", font.upcast_ref()),
        ("Lines of scrollback", scrollback.upcast_ref()),
        ("Copy selected text", copy_on_select.upcast_ref()),
        ("Paste with right-click", paste_on_right_click.upcast_ref()),
    ];
    for (row, (label, widget)) in rows.into_iter().enumerate() {
        let label = gtk::Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .build();
        grid.attach(&label, 0, row as i32, 1, 1);
        grid.attach(widget, 1, row as i32, 1, 1);
    }

    font.connect_font_desc_notify({
        let app = app.clone();
        move |button| {
            if let Some(desc) = button.font_desc() {
                settings::update(|s| s.font = desc.to_string());
                window::apply_settings_everywhere(&app);
            }
        }
    });
    scrollback.connect_value_changed({
        let app = app.clone();
        move |spin| {
            settings::update(|s| s.scrollback_lines = spin.value() as i64);
            window::apply_settings_everywhere(&app);
        }
    });
    copy_on_select.connect_active_notify(|switch| {
        settings::update(|s| s.copy_on_select = switch.is_active())
    });
    paste_on_right_click.connect_active_notify(|switch| {
        settings::update(|s| s.paste_on_right_click = switch.is_active())
    });

    gtk::Window::builder()
        .title("Preferences")
        .transient_for(parent)
        .modal(true)
        .resizable(false)
        .child(&grid)
        .build()
        .present();
}
