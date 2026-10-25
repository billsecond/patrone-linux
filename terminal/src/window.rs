//! Terminal windows: a header bar over a notebook of terminal tabs.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::{gdk, gio, glib, pango, prelude::*};
use vte::prelude::*;

use crate::theme;

const SCROLLBACK_LINES: i64 = 100_000;
const COLUMNS: i64 = 100;
const ROWS: i64 = 30;
const FONT_SCALE_STEP: f64 = 0.1;
const FONT_SCALE_MIN: f64 = 0.5;
const FONT_SCALE_MAX: f64 = 4.0;
/// Dragging a selection changes it on every pointer movement, so the clipboard
/// is only updated once the selection has stopped changing for this long.
const COPY_DELAY: Duration = Duration::from_millis(150);
/// The title a program sets with the xterm escape sequence.
const TITLE: &str = "xterm.title";
const DEFAULT_TITLE: &str = "Terminal";

/// What to run in a new tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Launch {
    pub working_directory: Option<PathBuf>,
    /// The program and its arguments. `None` runs the user's shell.
    pub command: Option<Vec<String>>,
    pub title: Option<String>,
}

pub fn open(app: &gtk::Application, launch: Launch) {
    let notebook = gtk::Notebook::builder()
        .scrollable(true)
        .show_border(false)
        .show_tabs(false)
        .build();
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title(DEFAULT_TITLE)
        .child(&notebook)
        .build();
    window.add_css_class("patrone-terminal");

    let header = gtk::HeaderBar::new();
    header.pack_start(
        &gtk::Button::builder()
            .icon_name("tab-new-symbolic")
            .tooltip_text("New Tab")
            .action_name("win.new-tab")
            .build(),
    );
    window.set_titlebar(Some(&header));

    // The tab bar only shows with two or more tabs, and the last tab to close
    // takes the window with it.
    notebook.connect_page_added(|notebook, _, _| notebook.set_show_tabs(notebook.n_pages() > 1));
    notebook.connect_page_removed(glib::clone!(
        #[weak]
        window,
        move |notebook, _, _| {
            notebook.set_show_tabs(notebook.n_pages() > 1);
            if notebook.n_pages() == 0 {
                window.close();
            }
        }
    ));
    notebook.connect_switch_page(glib::clone!(
        #[weak]
        window,
        move |notebook, page, _| {
            window.set_title(tab_title(notebook, page).as_deref());
            if let Some(terminal) = terminal_of(page) {
                // The page isn't visible yet, so focus it once the switch is done.
                glib::idle_add_local_once(move || {
                    terminal.grab_focus();
                });
            }
        }
    ));

    add_actions(&window, &notebook);
    add_tab(&notebook, launch);
    window.present();
}

fn add_tab(notebook: &gtk::Notebook, launch: Launch) {
    let terminal = vte::Terminal::new();
    configure(&terminal);
    terminal.set_size(COLUMNS, ROWS);

    let page = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_width(true)
        .propagate_natural_height(true)
        .child(&terminal)
        .build();

    let title = gtk::Label::builder()
        .label(launch.title.as_deref().unwrap_or(DEFAULT_TITLE))
        .ellipsize(pango::EllipsizeMode::End)
        .width_chars(12)
        .max_width_chars(28)
        .hexpand(true)
        .build();
    let close = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Close Tab")
        .has_frame(false)
        .build();
    let tab = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    tab.append(&title);
    tab.append(&close);

    let index = notebook.append_page(&page, Some(&tab));
    notebook.set_tab_reorderable(&page, true);
    notebook.set_current_page(Some(index));

    terminal.connect_termprop_changed(
        Some(TITLE),
        glib::clone!(
            #[weak]
            notebook,
            #[weak]
            page,
            #[weak]
            title,
            move |terminal, _| {
                let text = terminal.termprop_string(TITLE).0;
                title.set_text(
                    text.as_deref()
                        .filter(|t| !t.is_empty())
                        .unwrap_or(DEFAULT_TITLE),
                );
                if notebook.current_page() == notebook.page_num(&page)
                    && let Some(window) = notebook.root().and_downcast::<gtk::Window>()
                {
                    window.set_title(Some(&title.text()));
                }
            }
        ),
    );
    close.connect_clicked(glib::clone!(
        #[weak]
        notebook,
        #[weak]
        page,
        move |_| close_page(&notebook, &page)
    ));
    terminal.connect_child_exited(glib::clone!(
        #[weak]
        notebook,
        #[weak]
        page,
        move |_, _| close_page(&notebook, &page)
    ));

    spawn(&terminal, &launch);
}

fn configure(terminal: &vte::Terminal) {
    theme::apply(terminal);
    terminal.set_scrollback_lines(SCROLLBACK_LINES);
    terminal.set_cursor_shape(vte::CursorShape::Block);
    terminal.set_cursor_blink_mode(vte::CursorBlinkMode::System);
    terminal.set_audible_bell(false);
    terminal.set_allow_hyperlink(true);
    terminal.set_mouse_autohide(true);
    terminal.set_scroll_on_keystroke(true);
    terminal.set_scroll_on_output(false);
    terminal.set_hexpand(true);
    terminal.set_vexpand(true);
    copy_on_select(terminal);
    paste_on_right_click(terminal);
}

fn spawn(terminal: &vte::Terminal, launch: &Launch) {
    let argv = launch.command.clone().unwrap_or_else(|| vec![user_shell()]);
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    let program = argv[0].to_owned();
    let working_directory = launch.working_directory.as_deref().and_then(Path::to_str);
    terminal.spawn_async(
        vte::PtyFlags::DEFAULT,
        working_directory,
        &argv,
        &[],
        glib::SpawnFlags::SEARCH_PATH,
        || {},
        -1,
        gio::Cancellable::NONE,
        glib::clone!(
            #[weak]
            terminal,
            move |result| {
                if let Err(error) = result {
                    terminal.feed(format!("Could not run {program}: {error}\r\n").as_bytes());
                }
            }
        ),
    );
}

/// The login shell from the password database, which stays current after
/// `chsh`, unlike `$SHELL`.
fn user_shell() -> String {
    // SAFETY: returns a newly allocated string, or NULL.
    let shell: Option<glib::GString> =
        unsafe { glib::translate::from_glib_full(vte::ffi::vte_get_user_shell()) };
    shell
        .map(String::from)
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "/bin/sh".to_owned())
}

fn copy_on_select(terminal: &vte::Terminal) {
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
    terminal.connect_selection_changed(move |terminal| {
        if let Some(source) = pending.take() {
            source.remove();
        }
        if !terminal.has_selection() {
            return;
        }
        let source = glib::timeout_add_local_once(
            COPY_DELAY,
            glib::clone!(
                #[weak]
                terminal,
                #[strong]
                pending,
                move || {
                    pending.take();
                    if terminal.has_selection() {
                        terminal.copy_clipboard_format(vte::Format::Text);
                    }
                }
            ),
        );
        pending.replace(Some(source));
    });
}

fn paste_on_right_click(terminal: &vte::Terminal) {
    let click = gtk::GestureClick::builder()
        .button(gdk::BUTTON_SECONDARY)
        .propagation_phase(gtk::PropagationPhase::Capture)
        .build();
    click.connect_pressed(|gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        if let Some(terminal) = gesture.widget().and_downcast::<vte::Terminal>() {
            terminal.paste_clipboard();
        }
    });
    terminal.add_controller(click);
}

fn add_actions(window: &gtk::ApplicationWindow, notebook: &gtk::Notebook) {
    window.add_action_entries([
        gio::ActionEntry::builder("new-tab")
            .activate(glib::clone!(
                #[weak]
                notebook,
                move |_: &gtk::ApplicationWindow, _, _| {
                    let working_directory =
                        current_terminal(&notebook).and_then(|t| working_directory(&t));
                    add_tab(
                        &notebook,
                        Launch {
                            working_directory,
                            ..Launch::default()
                        },
                    );
                }
            ))
            .build(),
        gio::ActionEntry::builder("new-window")
            .activate(glib::clone!(
                #[weak]
                notebook,
                move |window: &gtk::ApplicationWindow, _, _| {
                    let working_directory =
                        current_terminal(&notebook).and_then(|t| working_directory(&t));
                    if let Some(app) = window.application() {
                        open(
                            &app,
                            Launch {
                                working_directory,
                                ..Launch::default()
                            },
                        );
                    }
                }
            ))
            .build(),
        notebook_action("close-tab", notebook, |notebook| {
            if let Some(page) = notebook.nth_page(notebook.current_page()) {
                close_page(notebook, &page);
            }
        }),
        notebook_action("next-tab", notebook, |notebook| cycle_tabs(notebook, 1)),
        notebook_action("previous-tab", notebook, |notebook| {
            cycle_tabs(notebook, -1)
        }),
        terminal_action("copy", notebook, |terminal| {
            if terminal.has_selection() {
                terminal.copy_clipboard_format(vte::Format::Text);
            }
        }),
        terminal_action("paste", notebook, |terminal| terminal.paste_clipboard()),
        terminal_action("zoom-in", notebook, |terminal| {
            zoom(terminal, FONT_SCALE_STEP)
        }),
        terminal_action("zoom-out", notebook, |terminal| {
            zoom(terminal, -FONT_SCALE_STEP)
        }),
        terminal_action("zoom-reset", notebook, |terminal| {
            terminal.set_font_scale(1.0)
        }),
    ]);
}

fn notebook_action(
    name: &str,
    notebook: &gtk::Notebook,
    activate: fn(&gtk::Notebook),
) -> gio::ActionEntry<gtk::ApplicationWindow> {
    let notebook = notebook.downgrade();
    gio::ActionEntry::builder(name)
        .activate(move |_, _, _| {
            if let Some(notebook) = notebook.upgrade() {
                activate(&notebook);
            }
        })
        .build()
}

fn terminal_action(
    name: &str,
    notebook: &gtk::Notebook,
    activate: fn(&vte::Terminal),
) -> gio::ActionEntry<gtk::ApplicationWindow> {
    let notebook = notebook.downgrade();
    gio::ActionEntry::builder(name)
        .activate(move |_, _, _| {
            if let Some(terminal) = notebook.upgrade().as_ref().and_then(current_terminal) {
                activate(&terminal);
            }
        })
        .build()
}

fn close_page(notebook: &gtk::Notebook, page: &impl IsA<gtk::Widget>) {
    if let Some(index) = notebook.page_num(page) {
        notebook.remove_page(Some(index));
    }
}

fn cycle_tabs(notebook: &gtk::Notebook, step: i32) {
    let count = notebook.n_pages() as i32;
    if let Some(current) = notebook.current_page() {
        notebook.set_current_page(Some((current as i32 + step).rem_euclid(count) as u32));
    }
}

fn zoom(terminal: &vte::Terminal, step: f64) {
    terminal.set_font_scale((terminal.font_scale() + step).clamp(FONT_SCALE_MIN, FONT_SCALE_MAX));
}

fn current_terminal(notebook: &gtk::Notebook) -> Option<vte::Terminal> {
    terminal_of(&notebook.nth_page(notebook.current_page())?)
}

fn terminal_of(page: &gtk::Widget) -> Option<vte::Terminal> {
    page.downcast_ref::<gtk::ScrolledWindow>()?
        .child()?
        .downcast()
        .ok()
}

fn tab_title(notebook: &gtk::Notebook, page: &gtk::Widget) -> Option<glib::GString> {
    let label = notebook
        .tab_label(page)?
        .first_child()?
        .downcast::<gtk::Label>()
        .ok()?;
    Some(label.text())
}

/// The shell's directory, which it reports with the OSC 7 escape sequence
/// (Patrone's zshrc sends it after every command).
#[allow(deprecated)] // its termprop replacement isn't in the Rust bindings
fn working_directory(terminal: &vte::Terminal) -> Option<PathBuf> {
    let uri = terminal.current_directory_uri()?;
    glib::filename_from_uri(&uri).ok().map(|(path, _host)| path)
}
