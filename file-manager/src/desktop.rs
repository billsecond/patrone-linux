//! The desktop, drawn by File Manager the way Explorer draws Windows'
//! desktop (the user, 2026-10-27: "The desktop and the roll out menu must
//! be an extension of the file explorer app, while supporting the desktop
//! and all it can do"), in place of Ubuntu's desktop icons.
//!
//! `patrone-file-manager --desktop` shows the Desktop folder in two
//! windows: one as big as the screen, with the icons on it, and, while the
//! taskbar's desktop bubble is on, the bubble, which takes the icons and
//! leaves the desktop itself empty (it still takes right-clicks, drops and
//! keys). Both are File Manager's own view of the folder, with its cells,
//! menus, commands and undo.
//!
//! The taskbar starts it and hosts its windows: a Wayland app can't make
//! its window the desktop or keep it above the others, so GNOME Shell does
//! (`desktopHost.js` in the taskbar). It finds them by their titles, and
//! the two talk over this program's standard input and output, one line
//! at a time:
//!
//! - in: `area X Y W H` (the screen's free area, in the desktop window),
//!   `bin SHOWN X Y W H` (the Recycle Bin: whether it's on the desktop,
//!   where, and its size), `bubble on|off`, `open` and `close` (the
//!   bubble opening and closing);
//! - out: `count N` (how many items the desktop has), `shape A B [X Y]`
//!   (the bubble's reach and half height, and where the Recycle Bin rides
//!   on its edge, from the bubble's top left corner; its window is the
//!   bubble's size, with room for its shadow, `bubble::SHADOW`, above,
//!   below and to the right), `open` and `close` (asks for the bubble to
//!   show or go), `show-bin` (Show Recycle Bin, from the menu).
//!
//! It runs apart from File Manager's windows (its own process, not on
//! D-Bus), so the taskbar can start and stop it with the desktop, and a
//! problem with one never closes the other.

use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::rc::Rc;

use gtk::{gdk, gio, glib, prelude::*};

use crate::bubble;
use crate::config;
use crate::desktopgrid::{DesktopGrid, ICON_SIZES, Layout, Rect};
use crate::places::{self, Special};
use crate::viewstate::{Column, View};
use crate::window::Window;

/// The windows' titles, by which the taskbar knows them.
const DESKTOP_TITLE: &str = "Patrone Desktop";
const BUBBLE_TITLE: &str = "Patrone Desktop Bubble";

struct Desktop {
    app: gtk::Application,
    window: Window,
    folder: gio::File,
    /// The desktop's window while the files are in the bubble.
    backdrop: gtk::Box,
    bubble: RefCell<Option<gtk::Window>>,
    bubble_on: Cell<bool>,
    bubble_open: Cell<bool>,
    bin_shown: Cell<bool>,
    area: Cell<Option<Rect>>,
}

thread_local! {
    static RUNNING: Cell<bool> = const { Cell::new(false) };
    static DESKTOP: RefCell<Option<Rc<Desktop>>> = const { RefCell::new(None) };
    /// Where lines to the taskbar go.
    static OUTPUT: RefCell<Option<std::fs::File>> = const { RefCell::new(None) };
    /// The taskbar's lines from before the desktop's windows were made.
    static PENDING: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Whether this process is the desktop.
pub fn is_running() -> bool {
    RUNNING.with(Cell::get)
}

fn desktop() -> Option<Rc<Desktop>> {
    DESKTOP.with(|desktop| desktop.borrow().clone())
}

/// Runs the desktop, until the taskbar closes its end.
pub fn run() -> glib::ExitCode {
    RUNNING.with(|running| running.set(true));
    // Its dialogs belong to no window (dialog_parent), and GTK names a
    // window with no app after the program: this way they're File
    // Manager's on the taskbar, not an unknown app's.
    glib::set_prgname(Some(crate::APP_ID));
    let (input, from_taskbar) = take_standard_streams();
    let app = gtk::Application::builder()
        .application_id(crate::APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_startup(|app| {
        crate::load_style();
        crate::window::set_accels(app);
        crate::thumbs::set_enabled(config::flag("show-thumbnails"));
    });
    app.connect_activate(start);
    if let Some(input) = input {
        read_commands(input, from_taskbar);
    }
    app.run_with_args(&["patrone-file-manager"])
}

/// The taskbar's pipes, kept for the desktop alone. The apps it opens
/// inherit standard input and output, so those become /dev/null and the
/// error stream: an app's output must never reach the taskbar as a line of
/// its own, nor keep the pipes open after the desktop has gone.
fn take_standard_streams() -> (Option<OwnedFd>, bool) {
    use std::os::unix::fs::FileTypeExt;
    let from_taskbar = std::fs::metadata("/proc/self/fd/0")
        .map(|metadata| metadata.file_type().is_fifo())
        .unwrap_or(false);
    let input = std::io::stdin().as_fd().try_clone_to_owned().ok();
    let output = std::io::stdout().as_fd().try_clone_to_owned().ok();
    OUTPUT.with(|out| out.replace(output.map(std::fs::File::from)));
    // SAFETY: plain descriptor calls on this process's own standard
    // streams, before any thread starts.
    unsafe {
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC);
        if null >= 0 {
            libc::dup2(null, 0);
            libc::close(null);
        }
        libc::dup2(2, 1);
    }
    (input, from_taskbar)
}

/// Reads the taskbar's lines in a thread of their own, each handled on the
/// main loop. When the taskbar's end closes, the desktop ends.
fn read_commands(input: OwnedFd, from_taskbar: bool) {
    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(std::fs::File::from(input));
        for line in reader.lines() {
            let Ok(line) = line else { break };
            glib::MainContext::default().invoke(move || handle(&line));
        }
        if from_taskbar {
            glib::MainContext::default().invoke(|| {
                if let Some(app) = gio::Application::default() {
                    app.quit();
                }
            });
        }
    });
}

/// A line to the taskbar.
fn send(line: &str) {
    OUTPUT.with(|out| {
        if let Some(out) = out.borrow_mut().as_mut() {
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
        }
    });
}

fn start(app: &gtk::Application) {
    if desktop().is_some() {
        return;
    }
    let folder = desktop_folder();
    let window = Window::new_desktop(app, folder.clone(), DESKTOP_TITLE);
    let backdrop = gtk::Box::new(gtk::Orientation::Vertical, 0);
    backdrop.add_css_class("desktop-backdrop");
    backdrop.set_hexpand(true);
    backdrop.set_vexpand(true);
    backdrop.set_focusable(true);
    let desktop = Rc::new(Desktop {
        app: app.clone(),
        window,
        folder,
        backdrop,
        bubble: RefCell::default(),
        bubble_on: Cell::new(false),
        bubble_open: Cell::new(false),
        bin_shown: Cell::new(true),
        area: Cell::new(None),
    });
    DESKTOP.with(|slot| slot.replace(Some(desktop.clone())));
    desktop.connect();
    desktop.apply_sort();
    desktop.window.present();
    desktop.send_count();
    for line in PENDING.with(|pending| pending.take()) {
        handle(&line);
    }
}

/// The Desktop folder, made if it's missing.
fn desktop_folder() -> gio::File {
    let folder =
        places::user_folder(Special::Desktop).unwrap_or_else(|| places::home().child("Desktop"));
    if let Some(path) = folder.path() {
        let _ = std::fs::create_dir_all(path);
    }
    folder
}

impl Desktop {
    fn grid(&self) -> Option<DesktopGrid> {
        self.window.tab().folder.desktop_grid()
    }

    fn connect(self: &Rc<Self>) {
        if let Some(grid) = self.grid() {
            grid.connect_outside(|_| send("close"));
            grid.connect_more(|_| open_desktop_folder());
            let weak = Rc::downgrade(self);
            grid.connect_reveal(move |_| {
                if let Some(desktop) = weak.upgrade()
                    && desktop.bubble_on.get()
                    && !desktop.bubble_open.get()
                {
                    send("open");
                }
            });
            let weak = Rc::downgrade(self);
            grid.connect_shape(move |_| {
                if let Some(desktop) = weak.upgrade() {
                    desktop.send_shape();
                }
            });
        }
        let weak = Rc::downgrade(self);
        self.window
            .tab()
            .folder
            .selection
            .connect_items_changed(move |_, _, _, _| {
                if let Some(desktop) = weak.upgrade() {
                    desktop.send_count();
                }
            });
        let weak = Rc::downgrade(self);
        config::settings().connect_changed(None, move |_, key| {
            let Some(desktop) = weak.upgrade() else {
                return;
            };
            match key {
                "desktop-icon-size" | "desktop-auto-arrange" | "desktop-show-icons" => {
                    if let Some(grid) = desktop.grid() {
                        grid.apply_settings();
                    }
                }
                "desktop-sort" => desktop.apply_sort(),
                _ => {}
            }
        });
        self.connect_backdrop();
    }

    /// The empty desktop, while the files are in the bubble: its menu,
    /// drops (into the Desktop folder, then shown in the bubble) and keys.
    fn connect_backdrop(self: &Rc<Self>) {
        let backdrop = &self.backdrop;
        let menu = gtk::GestureClick::new();
        menu.set_button(gdk::BUTTON_SECONDARY);
        let weak = Rc::downgrade(self);
        menu.connect_pressed(move |gesture, _, x, y| {
            let Some(desktop) = weak.upgrade() else {
                return;
            };
            gesture.set_state(gtk::EventSequenceState::Claimed);
            desktop.close_bubble();
            desktop.window.tab().select_none();
            if let Some(widget) = gesture.widget() {
                crate::menus::desktop_menu(&desktop.window, &widget, x, y);
            }
        });
        backdrop.add_controller(menu);
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, _, _, _| {
            let Some(desktop) = weak.upgrade() else {
                return;
            };
            desktop.close_bubble();
            desktop.window.tab().select_none();
            if let Some(widget) = gesture.widget() {
                widget.grab_focus();
            }
        });
        backdrop.add_controller(click);
        let target = gtk::DropTarget::new(
            gdk::FileList::static_type(),
            gdk::DragAction::COPY | gdk::DragAction::MOVE | gdk::DragAction::LINK,
        );
        target.set_preload(true);
        let folder = self.folder.clone();
        target.connect_motion(move |target, _, _| crate::dnd::preferred_action(target, &folder));
        let weak = Rc::downgrade(self);
        target.connect_drop(move |target, value, _, _| {
            let Some(desktop) = weak.upgrade() else {
                return false;
            };
            let Ok(list) = value.get::<gdk::FileList>() else {
                return false;
            };
            let files = list.files();
            let folder = desktop.folder.clone();
            if files
                .iter()
                .all(|file| file.parent().is_some_and(|parent| parent.equal(&folder)))
            {
                return false;
            }
            let action = crate::dnd::drop_action(target, &folder);
            crate::commands::drop_files(&desktop.window, files, folder, action);
            true
        });
        backdrop.add_controller(target);
        backdrop.add_controller(crate::window::file_shortcuts());
    }

    /// A click on the empty desktop, outside the open bubble, closes it.
    fn close_bubble(&self) {
        if self.bubble_open.get() {
            send("close");
        }
    }

    /// The desktop's Sort by, from the settings.
    fn apply_sort(&self) {
        let sort = Column::from_id(&config::string("desktop-sort")).unwrap_or(Column::Name);
        let folder = self.window.tab().folder.clone();
        let mut state = folder.state();
        state.view = View::Medium;
        state.sort = sort;
        state.descending = false;
        state.group = None;
        folder.set_state(state, false);
    }

    fn send_count(&self) {
        send(&format!("count {}", self.window.tab().n_items()));
    }

    fn send_shape(&self) {
        if !self.bubble_on.get() {
            return;
        }
        let Some((shape, _)) = self.grid().and_then(|grid| grid.shape()) else {
            return;
        };
        // The window is the bubble's size; the taskbar moves and sizes it
        // to match.
        if let Some(bubble) = self.bubble.borrow().as_ref() {
            let (width, height) = bubble_size(&shape);
            bubble.set_default_size(width, height);
        }
        match shape.bin {
            Some((x, y)) => send(&format!("shape {} {} {} {}", shape.a, shape.b, x, y)),
            None => send(&format!("shape {} {}", shape.a, shape.b)),
        }
    }

    /// The files in the bubble, or on the desktop.
    fn set_bubble(self: &Rc<Self>, on: bool) {
        let has_bubble = self.bubble.borrow().is_some();
        self.bubble_on.set(on);
        if on == has_bubble {
            return;
        }
        let content = self.window.content();
        let Some(grid) = self.grid() else { return };
        if on {
            grid.set_layout(Layout::Bubble);
            self.window.window.set_child(Some(&self.backdrop));
            let bubble = self.make_bubble();
            bubble.set_child(Some(&content));
            bubble.present();
            self.bubble.replace(Some(bubble));
            self.send_shape();
        } else {
            self.bubble_open.set(false);
            if let Some(bubble) = self.bubble.take() {
                bubble.set_child(None::<&gtk::Widget>);
                bubble.destroy();
            }
            grid.set_layout(Layout::Desktop);
            self.window.window.set_child(Some(&content));
        }
    }

    /// The bubble's window: the bubble's size, clear around its curve. The
    /// taskbar keeps it above the windows and shows it only while the
    /// bubble is open. (As big as the bubble can grow, its clear part took
    /// the drops meant for the windows under it.)
    fn make_bubble(self: &Rc<Self>) -> gtk::Window {
        let bubble = gtk::Window::builder()
            .application(&self.app)
            .title(BUBBLE_TITLE)
            .decorated(false)
            .build();
        bubble.add_css_class("desktop-bubble");
        // Its menus and keys run the desktop window's commands.
        bubble.insert_action_group("win", Some(&self.window.window));
        let (width, height) = self
            .grid()
            .and_then(|grid| grid.shape())
            .map_or((400, 600), |(shape, _)| bubble_size(&shape));
        bubble.set_default_size(width, height);
        bubble.connect_close_request(|_| glib::Propagation::Stop);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(|_, key, _, _| {
            if key == gdk::Key::Escape {
                send("close");
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        bubble.add_controller(keys);
        bubble
    }

    fn handle(self: &Rc<Self>, words: &[&str]) {
        let number = |index: usize| words.get(index).and_then(|w| w.parse::<f64>().ok());
        match words.first().copied() {
            Some("area") => {
                let (Some(x), Some(y), Some(width), Some(height)) =
                    (number(1), number(2), number(3), number(4))
                else {
                    return;
                };
                self.area.set(Some((x, y, width, height)));
                if let Some(grid) = self.grid() {
                    grid.set_area((x, y, width, height));
                }
                self.send_shape();
            }
            Some("bin") => {
                let shown = words.get(1) == Some(&"1");
                let (Some(x), Some(y), Some(width), Some(height)) =
                    (number(2), number(3), number(4), number(5))
                else {
                    return;
                };
                self.bin_shown.set(shown);
                if let Some(grid) = self.grid() {
                    grid.set_bin(
                        shown.then_some((x, y, width, height)),
                        Some((width, height)),
                    );
                }
            }
            Some("bubble") => self.set_bubble(words.get(1) == Some(&"on")),
            Some("open") => {
                self.bubble_open.set(true);
                if let Some(grid) = self.grid() {
                    grid.grab_focus();
                }
            }
            Some("close") => self.bubble_open.set(false),
            _ => {}
        }
    }
}

fn handle(line: &str) {
    let Some(desktop) = desktop() else {
        PENDING.with(|pending| pending.borrow_mut().push(line.to_owned()));
        return;
    };
    let words: Vec<&str> = line.split_whitespace().collect();
    desktop.handle(&words);
}

/// The bubble's window: the bubble, with room for its shadow.
fn bubble_size(shape: &bubble::Shape) -> (i32, i32) {
    (
        (shape.a + bubble::SHADOW) as i32,
        (2.0 * shape.b + 2.0 * bubble::SHADOW) as i32,
    )
}

// --- What the desktop's menus run ---------------------------------------------------

/// Whether the desktop's files are in the bubble.
pub fn bubble_on() -> bool {
    desktop().is_some_and(|desktop| desktop.bubble_on.get())
}

/// Whether the Recycle Bin is on the desktop.
pub fn bin_shown() -> bool {
    desktop().is_none_or(|desktop| desktop.bin_shown.get())
}

/// Show Recycle Bin: the taskbar's own setting.
pub fn show_bin() {
    send("show-bin");
}

pub fn open_desktop_folder() {
    if let Some(desktop) = desktop() {
        send("close");
        open_in_file_manager(std::slice::from_ref(&desktop.folder));
    }
}

/// Opens folders in File Manager's windows (a process of their own).
pub fn open_in_file_manager(folders: &[gio::File]) {
    if folders.is_empty() {
        return;
    }
    let program: OsString = std::env::current_exe()
        .map(|path| path.into_os_string())
        .unwrap_or_else(|_| "patrone-file-manager".into());
    let mut argv = vec![program];
    argv.extend(
        folders
            .iter()
            .map(|folder| OsString::from(folder.uri().as_str())),
    );
    spawn(&argv);
}

/// Starts a program and lets it run, with nothing of the desktop's.
fn spawn(argv: &[OsString]) {
    let argv: Vec<&std::ffi::OsStr> = argv.iter().map(OsString::as_os_str).collect();
    match gio::Subprocess::newv(
        &argv,
        gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_SILENCE,
    ) {
        Ok(process) => process.wait_async(None::<&gio::Cancellable>, |_| {}),
        Err(error) => eprintln!(
            "patrone-file-manager: could not start {}: {error}",
            argv[0].to_string_lossy()
        ),
    }
}

/// Display settings: GNOME Settings' Displays.
pub fn display_settings() {
    spawn(&["gnome-control-center".into(), "display".into()]);
}

/// Personalize: Patrone Settings' Theme page, or GNOME's Appearance.
pub fn personalize() {
    if glib::find_program_in_path("patrone-settings").is_some() {
        spawn(&["patrone-settings".into(), "--page".into(), "theme".into()]);
    } else {
        spawn(&["gnome-control-center".into(), "background".into()]);
    }
}

/// Sort by, from the desktop's menu. With Auto arrange off, Windows puts
/// the icons in that order once; they stay where they're moved after.
pub fn sort_by(_window: &Window, column: Column) {
    let _ = config::settings().set_string("desktop-sort", column.id());
    if let Some(desktop) = desktop() {
        desktop.apply_sort();
        if !config::flag("desktop-auto-arrange")
            && let Some(grid) = desktop.grid()
        {
            grid.arrange_in_order();
        }
    }
}

/// Ctrl+wheel on the desktop: bigger or smaller icons, as on Windows.
pub fn step_icon_size(bigger: bool) {
    let current = config::string("desktop-icon-size");
    let index = ICON_SIZES
        .iter()
        .position(|(id, _, _)| *id == current)
        .unwrap_or(1);
    let next = if bigger {
        index.saturating_sub(1)
    } else {
        (index + 1).min(ICON_SIZES.len() - 1)
    };
    let _ = config::settings().set_string("desktop-icon-size", ICON_SIZES[next].0);
}
