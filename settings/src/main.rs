//! Patrone Settings: the settings Patrone adds to the desktop's own, in a
//! window of their own, since GNOME Settings takes no pages from other
//! programs. Pages: Taskbar (its size), Title bar buttons (where minimize,
//! maximize and close go) and Theme.
//!
//!   patrone-settings [--page taskbar|buttons|theme]

mod preview;
mod theme;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use preview::Layout;

const APP_ID: &str = "org.patrone.Settings";
const TASKBAR_SCHEMA: &str = "org.gnome.shell.extensions.patrone-taskbar";
const DESKTOP_SCHEMA: &str = "org.patrone.desktop";
const WINDOW_MANAGER_SCHEMA: &str = "org.gnome.desktop.wm.preferences";
const PAGES: [(&str, &str, &str); 3] = [
    ("taskbar", "Taskbar", "user-desktop-symbolic"),
    ("buttons", "Title bar buttons", "window-maximize-symbolic"),
    ("theme", "Theme", "applications-graphics-symbolic"),
];

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

/// The page asked for with --page NAME or --page=NAME.
fn requested_page(arguments: &[String]) -> Option<String> {
    let mut iter = arguments.iter().skip(1);
    while let Some(argument) = iter.next() {
        if let Some(page) = argument.strip_prefix("--page=") {
            return Some(page.to_string());
        }
        if argument == "--page" {
            return iter.next().cloned();
        }
    }
    None
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
    // Three choices in a row; four in two rows of two
    let per_line = if items.len() == 3 { 3 } else { 2 };
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

fn taskbar_page() -> gtk::ScrolledWindow {
    let (scrolled, content) = page(
        "Taskbar",
        "The bar along the bottom of the screen, with Start, your apps and the clock.",
    );
    let Some(taskbar) = settings(TASKBAR_SCHEMA) else {
        note(&content, "Patrone's taskbar isn't installed (the patrone-gnome-taskbar package).");
        return scrolled;
    };

    section_title(&content, "Size");
    let size = |value: &'static str, title: &'static str, detail: &'static str, bar: f64, icon: f64| Choice {
        value,
        title,
        detail,
        draw: drawing(move |cr, w, h| preview::taskbar_size(cr, w, h, bar, icon)),
    };
    let current = taskbar.string("size");
    let target = taskbar.clone();
    let refresh = choices(
        &content,
        vec![
            size("small", "Small", "More room for windows", 12.0, 8.0),
            size("medium", "Medium", "The standard size", 16.0, 11.0),
            size("large", "Large", "Bigger buttons, easier to hit", 21.0, 14.0),
        ],
        &current,
        move |value| {
            if target.string("size") != value {
                let _ = target.set_string("size", value);
            }
        },
    );
    // (The choices hold the settings, so they last as long as the page)
    taskbar.connect_changed(Some("size"), move |settings, key| refresh(&settings.string(key)));
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
        "Which corner of windows' title bars minimize, maximize and close go in. \
         It changes at once, and stays as you set it whatever the theme.",
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
    scrolled
}

fn theme_page() -> gtk::ScrolledWindow {
    let (scrolled, content) = page(
        "Theme",
        "The look of the taskbar, title bars and window buttons. Which corner the \
         window buttons go in is up to you, on the Title bar buttons page. Apps \
         that are open pick up the change when they're next opened.",
    );
    let Some(desktop) = settings(DESKTOP_SCHEMA) else {
        note(&content, "Patrone Settings isn't installed properly: its settings schema is missing.");
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
    scrolled
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
        assert_eq!(super::requested_page(&args(&["patrone-settings"])), None);
    }
}