//! Patrone Settings: the settings Patrone adds to the desktop's own, in a
//! window of their own. Pages: Taskbar (its size, layout and Start icon),
//! Desktop (the desktop's files in a bubble or across the desktop, and the
//! Recycle Bin), Title bar buttons (where minimize, maximize and close go,
//! and their style) and Theme (the theme, and the desktop's font: Patrone
//! Sans or Noto Sans). Patrone's build of GNOME Settings has the same
//! settings on its Patrone page, and uses this program to apply a theme or
//! button style and for its pictures.
//!
//!   patrone-settings [--page taskbar|desktop|buttons|theme]
//!   patrone-settings --set-theme THEME          apply a theme, and quit
//!   patrone-settings --set-title-buttons STYLE  apply a button style, and quit
//!   patrone-settings --apply                    write the account's GTK styles
//!                                               for the settings as they are
//!   patrone-settings --export-previews DIR      save the choices' pictures as PNGs

mod font;
mod preview;
mod theme;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};

use preview::Layout;

const APP_ID: &str = "org.patrone.Settings";
const TASKBAR_SCHEMA: &str = "org.gnome.shell.extensions.patrone-taskbar";
const DESKTOP_SCHEMA: &str = "org.patrone.desktop";
const WINDOW_MANAGER_SCHEMA: &str = "org.gnome.desktop.wm.preferences";
const PAGES: [(&str, &str, &str); 4] = [
    ("taskbar", "Taskbar", "user-desktop-symbolic"),
    ("desktop", "Desktop", "view-grid-symbolic"),
    ("buttons", "Title bar buttons", "window-maximize-symbolic"),
    ("theme", "Theme", "applications-graphics-symbolic"),
];

/// The taskbar's sizes: setting value, name, description, and the bar's
/// and icons' heights in the picture.
const SIZES: [(&str, &str, &str, f64, f64); 3] = [
    ("small", "Small", "More room for windows", 12.0, 8.0),
    ("medium", "Medium", "The standard size", 16.0, 11.0),
    ("large", "Large", "Bigger buttons, easier to hit", 21.0, 14.0),
];

/// The taskbar's layouts (its layout setting).
const LAYOUTS: [(&str, &str, &str); 4] = [
    ("left", "Left", "Start and your apps on the left"),
    ("center", "Centered", "Start and your apps in the middle"),
    ("split", "Split", "Start on the left, your apps in the middle"),
    ("top", "Top", "Along the top of the screen, menus opening downward"),
];

/// The Start button's icons (its start-icon setting) and their files in the
/// taskbar's icons folder; the -symbolic ones take the text color.
const START_ICONS: [(&str, &str, &str); 8] = [
    ("patrone", "Patrone", "patrone-start.svg"),
    ("mono", "One color", "start-mono-symbolic.svg"),
    ("orb", "Orb", "start-orb.svg"),
    ("grid", "Grid", "start-grid-symbolic.svg"),
    ("menu", "Menu", "start-menu-symbolic.svg"),
    ("circle", "Circle", "start-circle-symbolic.svg"),
    ("spark", "Spark", "start-spark-symbolic.svg"),
    ("terminal", "Prompt", "start-terminal-symbolic.svg"),
];

/// Where the taskbar's color comes from (its color setting).
const COLORS: [(&str, &str, &str); 2] = [
    ("wallpaper", "From the wallpaper", "The wallpaper's own color, dark or light with the style"),
    ("theme", "Theme's own", "The theme's grey, whatever the wallpaper"),
];

/// What the taskbar's web, terminal, files and mail pins show (its
/// pin-icons setting).
const PIN_ICONS: [(&str, &str, &str); 2] = [
    ("patrone", "Patrone's", "A globe, a prompt, a folder and an envelope"),
    ("app", "Each app's own", "The icon of the app each one opens, in its own style"),
];

/// Where the desktop's files show (the taskbar's desktop-bubble setting,
/// on for the first).
const DESKTOP_FILES: [(&str, &str, &str); 2] = [
    ("bubble", "In a bubble", "A pill at the screen's left edge opens them, over any window"),
    ("desktop", "On the desktop", "Across the desktop, under your windows"),
];

/// Whether the Recycle Bin is on the desktop (its show-recycle-bin setting,
/// on for the first).
const RECYCLE_BIN: [(&str, &str, &str); 2] = [
    ("shown", "On the desktop", "Docked to a corner; drop files on it to delete them"),
    ("hidden", "Hidden", "Still in File Manager's navigation pane"),
];

/// The taskbar's icons folder: in the account's extensions, or the system's.
fn taskbar_icons() -> Option<PathBuf> {
    let relative = Path::new("gnome-shell/extensions/patrone-taskbar@patrone-linux/icons");
    std::iter::once(glib::user_data_dir())
        .chain(glib::system_data_dirs())
        .map(|dir| dir.join(relative))
        .find(|dir| dir.is_dir())
}

thread_local! {
    static SIDEBAR: RefCell<Option<gtk::ListBox>> = const { RefCell::new(None) };
}

/// A settings schema's settings, if it's installed (the taskbar's isn't,
/// without the taskbar).
fn settings(id: &str) -> Option<gio::Settings> {
    let schema = gio::SettingsSchemaSource::default()?.lookup(id, true)?;
    Some(gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None))
}

fn main() -> glib::ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    if let Some(name) = option_value(&arguments, "--set-theme") {
        return set_theme(&name);
    }
    if let Some(style) = option_value(&arguments, "--set-title-buttons") {
        return set_title_buttons(&style);
    }
    if arguments.iter().skip(1).any(|argument| argument == "--apply") {
        return apply_current();
    }
    if let Some(dir) = option_value(&arguments, "--export-previews") {
        return export_previews(Path::new(&dir));
    }

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_startup(|_| load_style());
    app.connect_command_line(|app, command_line| {
        let arguments: Vec<String> = command_line
            .arguments()
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        present(app, requested_page(&arguments).as_deref());
        glib::ExitCode::SUCCESS
    });
    app.run()
}

/// The value of an option given as `NAME VALUE` or `NAME=VALUE`.
fn option_value(arguments: &[String], name: &str) -> Option<String> {
    let mut iter = arguments.iter().skip(1);
    while let Some(argument) = iter.next() {
        if let Some(value) = argument.strip_prefix(name).and_then(|rest| rest.strip_prefix('=')) {
            return Some(value.to_string());
        }
        if argument == name {
            return iter.next().cloned();
        }
    }
    None
}

/// The page asked for with --page NAME or --page=NAME.
fn requested_page(arguments: &[String]) -> Option<String> {
    option_value(arguments, "--page")
}

/// --set-theme: applies a theme without a window (GNOME Settings' Patrone
/// page runs this), and waits for the setting to be written.
fn set_theme(name: &str) -> glib::ExitCode {
    if !theme::THEMES.iter().any(|&(value, _, _)| value == name) {
        eprintln!("patrone-settings: no theme called {name}");
        return glib::ExitCode::FAILURE;
    }
    let Some(desktop) = settings(DESKTOP_SCHEMA) else {
        eprintln!("patrone-settings: the {DESKTOP_SCHEMA} settings schema is missing");
        return glib::ExitCode::FAILURE;
    };
    theme::apply(name, &desktop);
    gio::Settings::sync();
    glib::ExitCode::SUCCESS
}

/// --set-title-buttons: applies a title bar button style without a window.
fn set_title_buttons(style: &str) -> glib::ExitCode {
    if !theme::BUTTON_STYLES.iter().any(|&(value, _, _)| value == style) {
        eprintln!("patrone-settings: no title bar button style called {style}");
        return glib::ExitCode::FAILURE;
    }
    let Some(desktop) = settings(DESKTOP_SCHEMA) else {
        eprintln!("patrone-settings: the {DESKTOP_SCHEMA} settings schema is missing");
        return glib::ExitCode::FAILURE;
    };
    theme::apply_buttons(style, &desktop);
    gio::Settings::sync();
    glib::ExitCode::SUCCESS
}

/// --apply: writes the account's GTK styles for the theme and button style
/// as they're set. It runs at every sign-in (an autostart entry), so every
/// account gets the same title bar buttons in every app, and so style
/// sheets a new version of Patrone Settings brings take over from the old.
fn apply_current() -> glib::ExitCode {
    let Some(desktop) = settings(DESKTOP_SCHEMA) else {
        eprintln!("patrone-settings: the {DESKTOP_SCHEMA} settings schema is missing");
        return glib::ExitCode::FAILURE;
    };
    theme::write_styles(&desktop);
    glib::ExitCode::SUCCESS
}

/// --export-previews: every choice's picture as a PNG in `dir`, at twice
/// the size the cards draw them (GNOME Settings' Patrone page shows these).
fn export_previews(dir: &Path) -> glib::ExitCode {
    let mut pictures: Vec<(String, Draw)> = Vec::new();
    for &(value, _, _, bar, icon) in &SIZES {
        pictures.push((format!("size-{value}"), drawing(move |cr, w, h| preview::taskbar_size(cr, w, h, bar, icon))));
    }
    for &(value, _, _) in &LAYOUTS {
        pictures.push((format!("layout-{value}"), drawing(move |cr, w, h| preview::taskbar_layout(cr, w, h, value))));
    }
    for &(value, _, _) in &COLORS {
        pictures.push((format!("color-{value}"), drawing(move |cr, w, h| preview::taskbar_color(cr, w, h, value))));
    }
    for &(value, _, _) in &PIN_ICONS {
        pictures.push((format!("pins-{value}"), drawing(move |cr, w, h| preview::pin_icons(cr, w, h, value))));
    }
    for &(value, _, _) in &DESKTOP_FILES {
        pictures.push((format!("files-{value}"), drawing(move |cr, w, h| preview::desktop_files(cr, w, h, value))));
    }
    for &(value, _, _) in &RECYCLE_BIN {
        pictures.push((format!("bin-{value}"), drawing(move |cr, w, h| preview::recycle_bin(cr, w, h, value))));
    }
    for (value, layout) in [("right", Layout::Right), ("left", Layout::Left)] {
        pictures.push((format!("buttons-{value}"), drawing(move |cr, w, h| preview::buttons(cr, w, h, layout))));
    }
    for &(value, _, _) in &theme::BUTTON_STYLES {
        pictures.push((format!("titlebuttons-{value}"), drawing(move |cr, w, h| preview::button_style(cr, w, h, value))));
    }
    for &(value, _, _) in &theme::THEMES {
        pictures.push((format!("theme-{value}"), drawing(move |cr, w, h| preview::theme(cr, w, h, value))));
    }

    if let Err(error) = std::fs::create_dir_all(dir) {
        eprintln!("patrone-settings: can't make {}: {error}", dir.display());
        return glib::ExitCode::FAILURE;
    }
    for (name, draw) in pictures {
        let path = dir.join(format!("{name}.png"));
        if let Err(error) = save_picture(&draw, &path) {
            eprintln!("patrone-settings: can't save {}: {error}", path.display());
            return glib::ExitCode::FAILURE;
        }
    }
    glib::ExitCode::SUCCESS
}

/// Draws a picture at twice the cards' size (188 by 118) and saves it as a
/// PNG, through a GDK texture (cairo's own PNG writer isn't built in here).
fn save_picture(draw: &Draw, path: &Path) -> Result<(), String> {
    const SCALE: i32 = 2;
    let (width, height) = (188 * SCALE, 118 * SCALE);
    let mut surface = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, width, height)
        .map_err(|error| error.to_string())?;
    {
        let cr = gtk::cairo::Context::new(&surface).map_err(|error| error.to_string())?;
        cr.scale(f64::from(SCALE), f64::from(SCALE));
        draw(&cr, 188.0, 118.0);
    }
    surface.flush();
    let stride = surface.stride() as usize;
    let data = surface.data().map_err(|error| error.to_string())?.to_vec();
    // Cairo's ARGB32 is premultiplied BGRA in memory on little-endian machines
    let texture = gdk::MemoryTexture::new(
        width,
        height,
        gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &glib::Bytes::from_owned(data),
        stride,
    );
    texture.save_to_png(path).map_err(|error| error.to_string())
}

fn load_style() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../data/style.css"));
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn present(app: &gtk::Application, page: Option<&str>) {
    let window = match app.active_window() {
        Some(window) => window,
        None => build_window(app).upcast(),
    };
    select_page(page.unwrap_or("taskbar"));
    window.present();
}

fn select_page(name: &str) {
    SIDEBAR.with(|sidebar| {
        let sidebar = sidebar.borrow();
        let Some(sidebar) = sidebar.as_ref() else { return };
        let mut index = 0;
        while let Some(row) = sidebar.row_at_index(index) {
            if row.widget_name() == name {
                sidebar.select_row(Some(&row));
                return;
            }
            index += 1;
        }
    });
}

fn build_window(app: &gtk::Application) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Patrone Settings")
        .default_width(960)
        .default_height(660)
        .build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .hexpand(true)
        .vexpand(true)
        .build();
    stack.add_named(&taskbar_page(), Some("taskbar"));
    stack.add_named(&desktop_page(), Some("desktop"));
    stack.add_named(&buttons_page(), Some("buttons"));
    stack.add_named(&theme_page(), Some("theme"));

    let sidebar = gtk::ListBox::new();
    sidebar.add_css_class("patrone-nav");
    for (name, title, icon) in PAGES {
        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row_box.append(&gtk::Image::from_icon_name(icon));
        row_box.append(&gtk::Label::new(Some(title)));
        let row = gtk::ListBoxRow::builder().child(&row_box).build();
        row.set_widget_name(name);
        sidebar.append(&row);
    }
    let pages = stack.clone();
    sidebar.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            pages.set_visible_child_name(&row.widget_name());
        }
    });
    SIDEBAR.with(|cell| *cell.borrow_mut() = Some(sidebar.clone()));

    let side = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .width_request(240)
        .css_classes(["patrone-side"])
        .build();
    side.append(
        &gtk::Label::builder()
            .label("Patrone Settings")
            .xalign(0.0)
            .css_classes(["patrone-side-title"])
            .build(),
    );
    side.append(&sidebar);

    let layout = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    layout.append(&side);
    layout.append(&stack);
    window.set_child(Some(&layout));
    window
}

/// A page: a title, a line about it, and room for sections.
fn page(title: &str, subtitle: &str) -> (gtk::ScrolledWindow, gtk::Box) {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .css_classes(["patrone-page"])
        .build();
    content.append(&gtk::Label::builder().label(title).xalign(0.0).css_classes(["patrone-page-title"]).build());
    content.append(
        &gtk::Label::builder()
            .label(subtitle)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["patrone-page-subtitle"])
            .build(),
    );
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();
    (scrolled, content)
}

fn section_title(content: &gtk::Box, title: &str) {
    content.append(&gtk::Label::builder().label(title).xalign(0.0).css_classes(["patrone-section-title"]).build());
}

fn note(content: &gtk::Box, text: &str) {
    content.append(
        &gtk::Label::builder()
            .label(text)
            .xalign(0.0)
            .wrap(true)
            .css_classes(["patrone-note"])
            .build(),
    );
}

type Draw = Rc<dyn Fn(&gtk::cairo::Context, f64, f64)>;

/// One choice: its setting value, name, description and picture.
struct Choice {
    value: &'static str,
    title: &'static str,
    detail: &'static str,
    draw: Draw,
}

fn drawing(draw: impl Fn(&gtk::cairo::Context, f64, f64) + 'static) -> Draw {
    Rc::new(draw)
}

/// A row of choices, one of them picked. `pick` runs when one is picked;
/// `refresh`, given the setting's value, shows it picked.
fn choices(content: &gtk::Box, items: Vec<Choice>, current: &str, pick: impl Fn(&str) + 'static) -> Box<dyn Fn(&str)> {
    // Three choices in a row, and five in rows of three; four in two rows
    // of two
    let per_line = if items.len() == 3 || items.len() == 5 { 3 } else { 2 };
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .column_spacing(12)
        .row_spacing(12)
        .max_children_per_line(per_line)
        .min_children_per_line(per_line)
        .homogeneous(true)
        .build();
    let pick = Rc::new(pick);
    let mut buttons: Vec<(gtk::ToggleButton, &'static str)> = Vec::new();
    for item in items {
        let picture = gtk::DrawingArea::builder().content_width(188).content_height(118).build();
        let draw = item.draw.clone();
        picture.set_draw_func(move |_, cr, width, height| draw(cr, f64::from(width), f64::from(height)));

        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.append(&picture);
        card.append(&gtk::Label::builder().label(item.title).xalign(0.0).css_classes(["patrone-choice-title"]).build());
        card.append(
            &gtk::Label::builder()
                .label(item.detail)
                .xalign(0.0)
                .wrap(true)
                .max_width_chars(24)
                .css_classes(["patrone-choice-detail"])
                .build(),
        );
        let button = gtk::ToggleButton::builder()
            .child(&card)
            .css_classes(["patrone-choice"])
            .active(item.value == current)
            .build();
        if let Some((first, _)) = buttons.first() {
            button.set_group(Some(first));
        }
        let pick = pick.clone();
        let value = item.value;
        button.connect_toggled(move |button| {
            if button.is_active() {
                pick(value);
            }
        });
        flow.append(&button);
        buttons.push((button, item.value));
    }
    content.append(&flow);

    Box::new(move |value: &str| {
        for (button, choice) in &buttons {
            if *choice == value && !button.is_active() {
                button.set_active(true);
            }
        }
    })
}

/// Keeps the choices that `refresh` updates showing `key`'s value whenever
/// it changes, from here or elsewhere.
fn bind(settings: &gio::Settings, key: &'static str, refresh: Box<dyn Fn(&str)>) {
    // (The handler holds the settings, so they last as long as the page)
    settings.connect_changed(Some(key), move |settings, key| refresh(&settings.string(key)));
}

fn setter(settings: &gio::Settings, key: &'static str) -> impl Fn(&str) + 'static {
    let target = settings.clone();
    move |value| {
        if target.string(key) != value {
            let _ = target.set_string(key, value);
        }
    }
}

/// A row of icons to pick from, one of them picked.
fn icon_choices(content: &gtk::Box, current: &str, pick: impl Fn(&str) + 'static) -> Box<dyn Fn(&str)> {
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .column_spacing(12)
        .row_spacing(12)
        .max_children_per_line(4)
        .min_children_per_line(4)
        .homogeneous(true)
        .build();
    let icons = taskbar_icons();
    let pick = Rc::new(pick);
    let mut buttons: Vec<(gtk::ToggleButton, &'static str)> = Vec::new();
    for &(value, title, file) in &START_ICONS {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        if let Some(dir) = &icons {
            // Through a GIcon, so GTK draws the -symbolic ones in the text color
            let icon = gio::FileIcon::new(&gio::File::for_path(dir.join(file)));
            card.append(&gtk::Image::builder().gicon(&icon).pixel_size(40).build());
        }
        card.append(&gtk::Label::builder().label(title).css_classes(["patrone-choice-title"]).build());
        let button = gtk::ToggleButton::builder()
            .child(&card)
            .css_classes(["patrone-choice", "patrone-icon-choice"])
            .active(value == current)
            .build();
        if let Some((first, _)) = buttons.first() {
            button.set_group(Some(first));
        }
        let pick = pick.clone();
        button.connect_toggled(move |button| {
            if button.is_active() {
                pick(value);
            }
        });
        flow.append(&button);
        buttons.push((button, value));
    }
    content.append(&flow);

    Box::new(move |value: &str| {
        for (button, choice) in &buttons {
            if *choice == value && !button.is_active() {
                button.set_active(true);
            }
        }
    })
}

fn taskbar_page() -> gtk::ScrolledWindow {
    let (scrolled, content) = page(
        "Taskbar",
        "The bar with Start, your apps and the clock: its size, where it and its \
         buttons go, the Start button's icon, its color and its pins' icons. \
         Themes don't change these.",
    );
    let Some(taskbar) = settings(TASKBAR_SCHEMA) else {
        note(&content, "Patrone's taskbar isn't installed (the patrone-gnome-taskbar package).");
        return scrolled;
    };

    section_title(&content, "Size");
    let items = SIZES
        .iter()
        .map(|&(value, title, detail, bar, icon)| Choice {
            value,
            title,
            detail,
            draw: drawing(move |cr, w, h| preview::taskbar_size(cr, w, h, bar, icon)),
        })
        .collect();
    let refresh = choices(&content, items, &taskbar.string("size"), setter(&taskbar, "size"));
    bind(&taskbar, "size", refresh);

    section_title(&content, "Layout");
    let items = LAYOUTS
        .iter()
        .map(|&(value, title, detail)| Choice {
            value,
            title,
            detail,
            draw: drawing(move |cr, w, h| preview::taskbar_layout(cr, w, h, value)),
        })
        .collect();
    let refresh = choices(&content, items, &taskbar.string("layout"), setter(&taskbar, "layout"));
    bind(&taskbar, "layout", refresh);

    section_title(&content, "Start button");
    let refresh = icon_choices(&content, &taskbar.string("start-icon"), setter(&taskbar, "start-icon"));
    bind(&taskbar, "start-icon", refresh);

    // An older taskbar's settings have neither (reading a missing key would
    // end the app)
    let has_key = |key: &str| taskbar.settings_schema().is_some_and(|schema| schema.has_key(key));
    if has_key("color") {
        section_title(&content, "Color");
        let items = COLORS
            .iter()
            .map(|&(value, title, detail)| Choice {
                value,
                title,
                detail,
                draw: drawing(move |cr, w, h| preview::taskbar_color(cr, w, h, value)),
            })
            .collect();
        let refresh = choices(&content, items, &taskbar.string("color"), setter(&taskbar, "color"));
        bind(&taskbar, "color", refresh);
    }
    if has_key("pin-icons") {
        section_title(&content, "Pin icons");
        let items = PIN_ICONS
            .iter()
            .map(|&(value, title, detail)| Choice {
                value,
                title,
                detail,
                draw: drawing(move |cr, w, h| preview::pin_icons(cr, w, h, value)),
            })
            .collect();
        let refresh = choices(&content, items, &taskbar.string("pin-icons"), setter(&taskbar, "pin-icons"));
        bind(&taskbar, "pin-icons", refresh);
    }
    scrolled
}

/// Choices for a yes-or-no setting: `yes` is the choice that turns it on,
/// `no` the one that turns it off.
fn yes_no_choices(content: &gtk::Box, settings: &gio::Settings, key: &'static str, items: Vec<Choice>, yes: &'static str, no: &'static str) {
    let value = move |on: bool| if on { yes } else { no };
    let target = settings.clone();
    let refresh = choices(content, items, value(settings.boolean(key)), move |picked| {
        let on = picked == yes;
        if target.boolean(key) != on {
            let _ = target.set_boolean(key, on);
        }
    });
    settings.connect_changed(Some(key), move |settings, key| refresh(value(settings.boolean(key))));
}

fn desktop_page() -> gtk::ScrolledWindow {
    let (scrolled, content) = page(
        "Desktop",
        "Where the files on your desktop show, and whether the Recycle Bin is on it.",
    );
    let Some(taskbar) = settings(TASKBAR_SCHEMA) else {
        note(&content, "Patrone's taskbar isn't installed (the patrone-gnome-taskbar package).");
        return scrolled;
    };
    // An older taskbar's settings may have neither (reading a missing key
    // would end the app)
    let has_key = |key: &str| taskbar.settings_schema().is_some_and(|schema| schema.has_key(key));
    if has_key("desktop-bubble") {
        section_title(&content, "Desktop files");
        note(
            &content,
            "In a bubble, they're kept off the desktop: a small pill at the screen's left \
             edge, shown whenever no window covers it, opens them. Click it, or drag it out.",
        );
        let items = DESKTOP_FILES
            .iter()
            .map(|&(value, title, detail)| Choice {
                value,
                title,
                detail,
                draw: drawing(move |cr, w, h| preview::desktop_files(cr, w, h, value)),
            })
            .collect();
        yes_no_choices(&content, &taskbar, "desktop-bubble", items, "bubble", "desktop");
    }
    if has_key("show-recycle-bin") {
        section_title(&content, "Recycle Bin");
        let items = RECYCLE_BIN
            .iter()
            .map(|&(value, title, detail)| Choice {
                value,
                title,
                detail,
                draw: drawing(move |cr, w, h| preview::recycle_bin(cr, w, h, value)),
            })
            .collect();
        yes_no_choices(&content, &taskbar, "show-recycle-bin", items, "shown", "hidden");
    }
    if !has_key("desktop-bubble") && !has_key("show-recycle-bin") {
        note(&content, "The taskbar installed is older than these settings: update patrone-gnome-taskbar.");
    }
    scrolled
}

/// Which corner a button-layout puts the buttons in: the left if close is
/// before the colon (GNOME's layouts list the left side's buttons first).
fn side_of(layout: &str) -> &'static str {
    let left = layout.split(':').next().unwrap_or("");
    if left.split(',').any(|button| button == "close") { "left" } else { "right" }
}

fn buttons_page() -> gtk::ScrolledWindow {
    let (scrolled, content) = page(
        "Title bar buttons",
        "Which corner of windows' title bars minimize, maximize and close go in, \
         and how they look. The corner changes at once, and stays as you set it \
         whatever the theme.",
    );
    let Some(window_manager) = settings(WINDOW_MANAGER_SCHEMA) else {
        note(&content, "GNOME's window settings aren't installed.");
        return scrolled;
    };

    section_title(&content, "Position");
    let position = |value: &'static str, title: &'static str, detail: &'static str, layout: Layout| Choice {
        value,
        title,
        detail,
        draw: drawing(move |cr, w, h| preview::buttons(cr, w, h, layout)),
    };
    let current = side_of(&window_manager.string("button-layout"));
    let target = window_manager.clone();
    let refresh = choices(
        &content,
        vec![
            position("right", "Top right", "Minimize, maximize, then close in the corner", Layout::Right),
            position("left", "Top left", "Close in the corner, then minimize, maximize", Layout::Left),
        ],
        current,
        move |side| {
            let layout = if side == "left" { theme::LAYOUT_LEFT } else { theme::LAYOUT_RIGHT };
            if target.string("button-layout") != layout {
                let _ = target.set_string("button-layout", layout);
            }
        },
    );
    window_manager.connect_changed(Some("button-layout"), move |settings, key| {
        refresh(side_of(&settings.string(key)));
    });

    section_title(&content, "Style");
    let Some(desktop) = settings(DESKTOP_SCHEMA) else {
        note(&content, "Patrone Settings isn't installed properly: its settings schema is missing.");
        return scrolled;
    };
    note(
        &content,
        "The same in every app. Apps that are open pick up a new style when they're next opened.",
    );
    let items = theme::BUTTON_STYLES
        .iter()
        .map(|&(value, title, detail)| Choice {
            value,
            title,
            detail,
            draw: drawing(move |cr, w, h| preview::button_style(cr, w, h, value)),
        })
        .collect();
    let target = desktop.clone();
    let refresh = choices(&content, items, &desktop.string("title-buttons"), move |value| {
        if target.string("title-buttons") != value {
            theme::apply_buttons(value, &target);
        }
    });
    bind(&desktop, "title-buttons", refresh);
    scrolled
}

fn theme_page() -> gtk::ScrolledWindow {
    let (scrolled, content) = page(
        "Theme",
        "The look of the taskbar, title bars and window buttons, and the \
         desktop's font. Where the taskbar's buttons go (the Taskbar page) and \
         which corner the window buttons go in (Title bar buttons) are up to \
         you: themes don't change them. Apps that are open pick up a new theme \
         when they're next opened.",
    );
    let Some(desktop) = settings(DESKTOP_SCHEMA) else {
        note(&content, "Patrone Settings isn't installed properly: its settings schema is missing.");
        font_section(&content);
        return scrolled;
    };

    section_title(&content, "Themes");
    let items = theme::THEMES
        .iter()
        .map(|&(value, title, detail)| Choice {
            value,
            title,
            detail,
            draw: drawing(move |cr, w, h| preview::theme(cr, w, h, value)),
        })
        .collect();
    let current = desktop.string("theme");
    let target = desktop.clone();
    let refresh = choices(&content, items, &current, move |value| {
        if target.string("theme") != value {
            theme::apply(value, &target);
        }
    });
    desktop.connect_changed(Some("theme"), move |settings, key| refresh(&settings.string(key)));

    font_section(&content);
    scrolled
}

fn font_section(content: &gtk::Box) {
    section_title(content, "Font");
    let Some(interface) = settings(font::INTERFACE_SCHEMA) else {
        note(content, "GNOME's interface settings aren't installed.");
        return;
    };
    note(
        content,
        "For the desktop's and apps' text, at the size it is now. Monospace text \
         keeps its own font.",
    );
    let window_manager = settings(WINDOW_MANAGER_SCHEMA);
    let installed: Vec<String> = content
        .pango_context()
        .list_families()
        .iter()
        .map(|family| family.name().to_string())
        .collect();
    let target = interface.clone();
    let current = font::family_of(&interface.string("font-name"));
    let refresh = font_choices(content, &current, &installed, move |family| {
        font::apply(family, &target, window_manager.as_ref());
    });
    interface.connect_changed(Some("font-name"), move |settings, key| {
        refresh(&font::family_of(&settings.string(key)));
    });
}

/// Text in a font, at a size in points, for a font's card (in its regular
/// weight, whatever weight the theme gives buttons' text).
fn sample_label(text: &str, family: &str, points: i32) -> gtk::Label {
    let mut description = pango::FontDescription::new();
    description.set_family(family);
    description.set_weight(pango::Weight::Normal);
    description.set_size(points * pango::SCALE);
    let attributes = pango::AttrList::new();
    attributes.insert(pango::AttrFontDesc::new(&description));
    gtk::Label::builder().label(text).attributes(&attributes).xalign(0.0).build()
}

/// The fonts to pick from, each card showing text in its font; one that
/// isn't installed can't be picked.
fn font_choices(content: &gtk::Box, current: &str, installed: &[String], pick: impl Fn(&str) + 'static) -> Box<dyn Fn(&str)> {
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .column_spacing(12)
        .row_spacing(12)
        .max_children_per_line(2)
        .min_children_per_line(2)
        .homogeneous(true)
        .build();
    let pick = Rc::new(pick);
    let mut buttons: Vec<(gtk::ToggleButton, &'static str)> = Vec::new();
    for &(family, title, detail, package) in &font::FONTS {
        let is_installed = installed.iter().any(|name| name == family);
        let sample = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .height_request(118)
            .css_classes(["patrone-font-sample"])
            .build();
        sample.append(&sample_label(font::SAMPLE_BIG, family, 30));
        sample.append(&sample_label(font::SAMPLE, family, 15));

        let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
        card.append(&sample);
        card.append(&gtk::Label::builder().label(title).xalign(0.0).css_classes(["patrone-choice-title"]).build());
        let missing = format!("Not installed: the {package} package");
        card.append(
            &gtk::Label::builder()
                .label(if is_installed { detail } else { missing.as_str() })
                .xalign(0.0)
                .wrap(true)
                .max_width_chars(24)
                .css_classes(["patrone-choice-detail"])
                .build(),
        );
        let button = gtk::ToggleButton::builder()
            .child(&card)
            .css_classes(["patrone-choice"])
            .active(family == current)
            .sensitive(is_installed)
            .build();
        if let Some((first, _)) = buttons.first() {
            button.set_group(Some(first));
        }
        let pick = pick.clone();
        button.connect_toggled(move |button| {
            if button.is_active() {
                pick(family);
            }
        });
        flow.append(&button);
        buttons.push((button, family));
    }
    content.append(&flow);

    Box::new(move |value: &str| {
        for (button, choice) in &buttons {
            if *choice == value && !button.is_active() {
                button.set_active(true);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::side_of;

    #[test]
    fn reads_which_corner_a_layout_uses() {
        assert_eq!(side_of("appmenu:minimize,maximize,close"), "right");
        assert_eq!(side_of("close,minimize,maximize:appmenu"), "left");
        assert_eq!(side_of("appmenu:close"), "right"); // GNOME's own default
        assert_eq!(side_of("close:"), "left");
        assert_eq!(side_of(""), "right");
    }

    #[test]
    fn reads_the_page_asked_for() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(super::requested_page(&args(&["patrone-settings", "--page", "theme"])).as_deref(), Some("theme"));
        assert_eq!(super::requested_page(&args(&["patrone-settings", "--page=buttons"])).as_deref(), Some("buttons"));
        assert_eq!(super::requested_page(&args(&["patrone-settings", "--page", "desktop"])).as_deref(), Some("desktop"));
        assert_eq!(super::requested_page(&args(&["patrone-settings"])), None);
    }

    #[test]
    fn reads_options_with_or_without_an_equals_sign() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let set = args(&["patrone-settings", "--set-theme", "royal"]);
        assert_eq!(super::option_value(&set, "--set-theme").as_deref(), Some("royal"));
        let export = args(&["patrone-settings", "--export-previews=/tmp/p"]);
        assert_eq!(super::option_value(&export, "--export-previews").as_deref(), Some("/tmp/p"));
        // A longer option that starts the same way isn't it
        let other = args(&["patrone-settings", "--set-themes=x"]);
        assert_eq!(super::option_value(&other, "--set-theme"), None);
    }

    #[test]
    fn every_theme_is_one_the_setting_takes() {
        let desktop = include_str!("../data/org.patrone.desktop.gschema.xml");
        for (value, _, _) in crate::theme::THEMES {
            assert!(desktop.contains(&format!("<choice value=\"{value}\"/>")), "{value}");
        }
    }
}