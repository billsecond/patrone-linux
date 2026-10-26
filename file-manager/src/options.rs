//! Folder Options, as on Windows: General (where windows open, same or new
//! window, single- or double-click, privacy), View (apply a view to every
//! folder, and the advanced settings) and Search. Nothing changes until OK
//! or Apply; each tab has Restore Defaults.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::{gio, glib, prelude::*};

use crate::config;
use crate::dialogs::Dialog;
use crate::viewstate;
use crate::window::Window;

type Saver = Box<dyn Fn(&gio::Settings)>;
type Resetter = Box<dyn Fn(&gio::Settings)>;

thread_local! {
    static OPEN: RefCell<Option<glib::WeakRef<gtk::Window>>> = const { RefCell::new(None) };
}

#[derive(Default)]
struct Form {
    savers: RefCell<Vec<Saver>>,
    /// Restore Defaults, per tab.
    resetters: RefCell<Vec<Vec<Resetter>>>,
}

impl Form {
    fn tab(&self) -> usize {
        let mut resetters = self.resetters.borrow_mut();
        resetters.push(Vec::new());
        resetters.len() - 1
    }

    fn add(&self, tab: usize, save: Saver, reset: Resetter) {
        self.savers.borrow_mut().push(save);
        self.resetters.borrow_mut()[tab].push(reset);
    }

    /// A check box for a boolean setting (`invert` for "Hide..." wording).
    fn check(
        &self,
        tab: usize,
        parent: &gtk::Box,
        label: &str,
        key: &'static str,
        invert: bool,
        indent: i32,
    ) -> gtk::CheckButton {
        let check = gtk::CheckButton::with_label(label);
        check.set_margin_start(indent);
        check.set_active(config::flag(key) != invert);
        parent.append(&check);
        let saved = check.clone();
        let reset = check.clone();
        self.add(
            tab,
            Box::new(move |settings| {
                let value = saved.is_active() != invert;
                if settings.boolean(key) != value {
                    let _ = settings.set_boolean(key, value);
                }
            }),
            Box::new(move |settings| {
                let default = settings
                    .default_value(key)
                    .and_then(|v| v.get::<bool>())
                    .unwrap_or(false);
                reset.set_active(default != invert);
            }),
        );
        check
    }

    /// Radio buttons for a setting with a few values.
    fn radios(
        &self,
        tab: usize,
        parent: &gtk::Box,
        key: &'static str,
        choices: &[(&str, &'static str)],
        indent: i32,
    ) -> Vec<gtk::CheckButton> {
        let current = config::settings().value(key);
        let mut buttons: Vec<gtk::CheckButton> = Vec::new();
        for (label, value) in choices {
            let button = gtk::CheckButton::with_label(label);
            button.set_margin_start(indent);
            if let Some(first) = buttons.first() {
                button.set_group(Some(first));
            }
            button.set_active(variant_matches(&current, value));
            parent.append(&button);
            buttons.push(button);
        }
        let values: Vec<&'static str> = choices.iter().map(|(_, value)| *value).collect();
        {
            let buttons = buttons.clone();
            let reset_values = values.clone();
            let reset_buttons = buttons.clone();
            self.add(
                tab,
                Box::new(move |settings| {
                    if let Some(index) = buttons.iter().position(|b| b.is_active()) {
                        let value = variant_for(&settings.value(key), values[index]);
                        if settings.value(key) != value {
                            let _ = settings.set_value(key, &value);
                        }
                    }
                }),
                Box::new(move |settings| {
                    if let Some(default) = settings.default_value(key) {
                        for (button, value) in reset_buttons.iter().zip(&reset_values) {
                            if variant_matches(&default, value) {
                                button.set_active(true);
                            }
                        }
                    }
                }),
            );
        }
        buttons
    }
}

/// Settings here are strings or booleans ("true"/"false").
fn variant_matches(variant: &glib::Variant, value: &str) -> bool {
    match variant.get::<bool>() {
        Some(flag) => flag.to_string() == value,
        None => variant.get::<String>().as_deref() == Some(value),
    }
}

fn variant_for(current: &glib::Variant, value: &str) -> glib::Variant {
    if current.get::<bool>().is_some() {
        (value == "true").to_variant()
    } else {
        value.to_variant()
    }
}

fn group(title: &str) -> (gtk::Frame, gtk::Box) {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
    content.add_css_class("options-group");
    let frame = gtk::Frame::builder().label(title).child(&content).build();
    frame.add_css_class("options-frame");
    (frame, content)
}

fn heading(parent: &gtk::Box, text: &str) {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    label.add_css_class("options-heading");
    parent.append(&label);
}

fn restore_button(form: &Rc<Form>, tab: usize) -> gtk::Button {
    let button = gtk::Button::with_mnemonic("_Restore Defaults");
    button.add_css_class("dialog-button");
    button.set_halign(gtk::Align::End);
    let form = form.clone();
    button.connect_clicked(move |_| {
        let settings = config::settings();
        for reset in form.resetters.borrow()[tab].iter() {
            reset(&settings);
        }
    });
    button
}

pub fn show(window: &Window) {
    let existing = OPEN.with(|open| open.borrow().as_ref().and_then(|weak| weak.upgrade()));
    if let Some(existing) = existing {
        existing.present();
        return;
    }
    let dialog = Dialog::new(None, "Folder Options");
    dialog.window.set_transient_for(Some(&window.gtk_window()));
    dialog.window.set_default_size(440, 560);
    dialog.window.add_css_class("options-window");
    OPEN.with(|open| open.replace(Some(dialog.window.downgrade())));
    let form = Rc::new(Form::default());
    let notebook = gtk::Notebook::new();
    notebook.add_css_class("property-tabs");
    dialog.body.append(&notebook);

    // General.
    let tab = form.tab();
    let general = gtk::Box::new(gtk::Orientation::Vertical, 14);
    general.add_css_class("property-page");
    let open_to_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    open_to_row.append(&gtk::Label::new(Some("Open File Manager to:")));
    let open_to = gtk::DropDown::from_strings(&["Home", "This PC"]);
    open_to.set_selected(if config::string("open-to") == "this-pc" {
        1
    } else {
        0
    });
    open_to_row.append(&open_to);
    general.append(&open_to_row);
    {
        let open_to = open_to.clone();
        let reset = open_to.clone();
        form.add(
            tab,
            Box::new(move |settings| {
                let value = if open_to.selected() == 1 {
                    "this-pc"
                } else {
                    "home"
                };
                if settings.string("open-to") != value {
                    let _ = settings.set_string("open-to", value);
                }
            }),
            Box::new(move |_| reset.set_selected(0)),
        );
    }
    let (frame, content) = group("Browse folders");
    form.radios(
        tab,
        &content,
        "open-folders-in-new-window",
        &[
            ("Open each folder in the same window", "false"),
            ("Open each folder in its own window", "true"),
        ],
        0,
    );
    general.append(&frame);
    let (frame, content) = group("Click items as follows");
    let single = gtk::CheckButton::with_label("Single-click to open an item (point to select)");
    let double =
        gtk::CheckButton::with_label("Double-click to open an item (single-click to select)");
    double.set_group(Some(&single));
    content.append(&single);
    let underline = form.radios(
        tab,
        &content,
        "underline-titles",
        &[
            ("Underline icon titles always", "always"),
            ("Underline icon titles only when I point at them", "hover"),
        ],
        24,
    );
    content.append(&double);
    let single_now = config::flag("single-click");
    single.set_active(single_now);
    double.set_active(!single_now);
    for button in &underline {
        button.set_sensitive(single_now);
    }
    {
        let underline = underline.clone();
        single.connect_toggled(move |single| {
            for button in &underline {
                button.set_sensitive(single.is_active());
            }
        });
    }
    {
        let single = single.clone();
        let reset_double = double.clone();
        form.add(
            tab,
            Box::new(move |settings| {
                if settings.boolean("single-click") != single.is_active() {
                    let _ = settings.set_boolean("single-click", single.is_active());
                }
            }),
            Box::new(move |_| reset_double.set_active(true)),
        );
    }
    general.append(&frame);
    let (frame, content) = group("Privacy");
    form.check(
        tab,
        &content,
        "Show recently used files",
        "show-recent-files",
        false,
        0,
    );
    form.check(
        tab,
        &content,
        "Show frequently used folders",
        "show-frequent-folders",
        false,
        0,
    );
    let clear_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let clear_label = gtk::Label::builder()
        .label("Clear File Manager history")
        .xalign(0.0)
        .hexpand(true)
        .build();
    let clear = gtk::Button::with_mnemonic("_Clear");
    clear.add_css_class("dialog-button");
    clear.connect_clicked(|_| {
        viewstate::clear_history();
        let recent = gtk::RecentManager::default();
        let _ = recent.purge_items();
        let _ = config::settings().set_strv("search-history", &[] as &[&str]);
    });
    clear_row.append(&clear_label);
    clear_row.append(&clear);
    content.append(&clear_row);
    general.append(&frame);
    general.append(&restore_button(&form, tab));
    notebook.append_page(&general, Some(&gtk::Label::new(Some("General"))));

    // View.
    let tab = form.tab();
    let view = gtk::Box::new(gtk::Orientation::Vertical, 14);
    view.add_css_class("property-page");
    let (frame, content) = group("Folder views");
    let explain = gtk::Label::builder()
        .label("You can apply this view (such as Details or Icons) to all folders.")
        .xalign(0.0)
        .wrap(true)
        .width_chars(40)
        .build();
    content.append(&explain);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let apply_all = gtk::Button::with_mnemonic("Apply to _Folders");
    let reset_all = gtk::Button::with_mnemonic("R_eset Folders");
    for button in [&apply_all, &reset_all] {
        button.add_css_class("dialog-button");
        buttons.append(button);
    }
    content.append(&buttons);
    {
        let state = window.tab().view_state();
        apply_all.set_sensitive(state.is_some());
        apply_all.connect_clicked(move |_| {
            if let Some(state) = state {
                viewstate::apply_to_all(&state);
            }
        });
    }
    reset_all.connect_clicked(|_| viewstate::forget_all());
    view.append(&frame);
    heading(&view, "Advanced settings:");
    let advanced = gtk::Box::new(gtk::Orientation::Vertical, 6);
    advanced.add_css_class("advanced-settings");
    heading(&advanced, "Files and Folders");
    form.check(
        tab,
        &advanced,
        "Always show icons, never thumbnails",
        "show-thumbnails",
        true,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Display file size information in folder tips",
        "show-size-in-tips",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Display the full path in the title bar",
        "show-full-path-in-title",
        false,
        12,
    );
    let hidden_label = gtk::Label::builder()
        .label("Hidden files and folders")
        .xalign(0.0)
        .margin_start(12)
        .build();
    advanced.append(&hidden_label);
    form.radios(
        tab,
        &advanced,
        "show-hidden",
        &[
            ("Don't show hidden files, folders, or drives", "false"),
            ("Show hidden files, folders, and drives", "true"),
        ],
        28,
    );
    form.check(
        tab,
        &advanced,
        "Hide empty drives",
        "hide-empty-drives",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Hide extensions for known file types",
        "hide-extensions",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Restore previous folder windows at sign-in",
        "restore-windows",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Remember each folder's view settings",
        "remember-folder-views",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Show pop-up description for folder and file items",
        "show-tooltips",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Show preview handlers in preview pane",
        "show-preview-handlers",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Show status bar",
        "show-status-bar",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Use check boxes to select items",
        "item-checkboxes",
        false,
        12,
    );
    form.check(
        tab,
        &advanced,
        "Display delete confirmation dialog",
        "confirm-trash",
        false,
        12,
    );
    let typing_label = gtk::Label::builder()
        .label("When typing into list view")
        .xalign(0.0)
        .margin_start(12)
        .build();
    advanced.append(&typing_label);
    form.radios(
        tab,
        &advanced,
        "typing-in-view",
        &[
            ("Automatically type into the Search Box", "search"),
            ("Select the typed item in the view", "select"),
        ],
        28,
    );
    heading(&advanced, "Navigation pane");
    form.check(
        tab,
        &advanced,
        "Expand to open folder",
        "nav-expand-to-folder",
        false,
        12,
    );
    let scroller = gtk::ScrolledWindow::builder()
        .child(&advanced)
        .vexpand(true)
        .min_content_height(240)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    scroller.add_css_class("advanced-scroller");
    view.append(&scroller);
    view.append(&restore_button(&form, tab));
    notebook.append_page(&view, Some(&gtk::Label::new(Some("View"))));

    // Search.
    let tab = form.tab();
    let search = gtk::Box::new(gtk::Orientation::Vertical, 14);
    search.add_css_class("property-page");
    let (frame, content) = group("How to search");
    form.check(
        tab,
        &content,
        "Search all subfolders of the folder being searched",
        "search-subfolders",
        false,
        0,
    );
    search.append(&frame);
    let (frame, content) = group("When searching");
    form.check(
        tab,
        &content,
        "Include system directories",
        "search-system-folders",
        false,
        0,
    );
    form.check(
        tab,
        &content,
        "Include compressed files (ZIP)",
        "search-archives",
        false,
        0,
    );
    form.check(
        tab,
        &content,
        "Always search file names and contents (this might take several minutes)",
        "search-contents",
        false,
        0,
    );
    search.append(&frame);
    search.append(&restore_button(&form, tab));
    notebook.append_page(&search, Some(&gtk::Label::new(Some("Search"))));

    let save = {
        let form = form.clone();
        move || {
            // Written only now, on OK or Apply.
            let settings = config::settings();
            for saver in form.savers.borrow().iter() {
                saver(&settings);
            }
        }
    };
    dialog.add_button("OK", 0, true);
    dialog.add_button("Cancel", 1, false);
    let apply = gtk::Button::with_mnemonic("_Apply");
    apply.add_css_class("dialog-button");
    {
        let save = save.clone();
        apply.connect_clicked(move |_| save());
    }
    dialog.footer.append(&apply);
    let dialog_ref = dialog.clone();
    glib::spawn_future_local(async move {
        if dialog_ref.run().await == 0 {
            save();
        }
    });
}
