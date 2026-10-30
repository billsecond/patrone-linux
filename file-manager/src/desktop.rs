//! The desktop, drawn by File Manager the way Explorer draws Windows'
//! desktop (the user, 2026-10-27: "The desktop and the roll out menu must
//! be an extension of the file explorer app, while supporting the desktop
//! and all it can do"), in place of Ubuntu's desktop icons.
//!
//! `patrone-file-manager --desktop` shows the Desktop folder in two
//! windows: one as big as the screen, with the icons on it, and, while the
//! taskbar's desktop files panel is on, the panel, which takes the icons and
//! leaves the desktop itself empty (it still takes right-clicks, drops and
//! keys). The panel's window covers the screen's free area, clear but for
//! the icons and a small bar saying what it is, with the wallpaper under it
//! (the taskbar's), and each icon where it would be on the desktop. Both are
//! File Manager's own view of the folder, with its cells, menus, commands
//! and undo.
//!
//! The taskbar starts it and hosts its windows: a Wayland app can't make
//! its window the desktop or keep it above the others, so GNOME Shell does
//! (`desktopHost.js` in the taskbar). It finds them by their titles, and
//! the two talk over this program's standard input and output, one line
//! at a time:
//!
//! - in: `area X Y W H` (the screen's free area, in the desktop window),
//!   `bin SHOWN X Y W H` (the Recycle Bin: whether it's on the desktop, and
//!   where it is, or would be, with its size), `panel on|clean|off` (the
//!   files in the panel, on a clean desktop as well, or on the desktop), `open` and
//!   `close` (the panel opening and closing);
//! - out: `count N` (how many items the desktop has), `open` and `close`
//!   (asks for the panel to show or go), `show-bin` (Show Recycle Bin, from
//!   the menu).
//!
//! It runs apart from File Manager's windows (its own process, not on
//! D-Bus), so the taskbar can start and stop it with the desktop, and a
//! problem with one never closes the other.

use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::path::PathBuf;
use std::rc::Rc;

use gtk::{gdk, gio, glib, prelude::*};

use crate::config;
use crate::desktopgrid::{DesktopGrid, ICON_SIZES, Layout, Rect};
use crate::places::{self, Special};
use crate::viewstate::{Column, View};
use crate::window::Window;

/// The windows' titles, by which the taskbar knows them.
const DESKTOP_TITLE: &str = "Patrone Desktop";
const PANEL_TITLE: &str = "Patrone Desktop Panel";

struct Desktop {
    app: gtk::Application,
    window: Window,
    folder: gio::File,
    /// The desktop's window while the files are in the panel.
    backdrop: gtk::Box,
    panel: RefCell<Option<gtk::Window>>,
    panel_on: Cell<bool>,
    /// Nothing on the desktop: the taskbar opens the panel from a corner,
    /// and has no pill.
    clean: Cell<bool>,
    panel_open: Cell<bool>,
    /// How many items the desktop has, on the panel's bar.
    count: gtk::Label,
    bin_shown: Cell<bool>,
    /// Where the Recycle Bin is, or would be, in the desktop window.
    bin: Cell<Option<Rect>>,
    /// The screen's free area, in the desktop window.
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
    let count = gtk::Label::new(None);
    count.add_css_class("count");
    let desktop = Rc::new(Desktop {
        app: app.clone(),
        window,
        folder,
        backdrop,
        panel: RefCell::default(),
        panel_on: Cell::new(false),
        clean: Cell::new(false),
        panel_open: Cell::new(false),
        count,
        bin_shown: Cell::new(true),
        bin: Cell::new(None),
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
            grid.connect_trash(|files| {
                glib::spawn_future_local(async move {
                    crate::ops::trash(None, files, true).await;
                });
            });
            let weak = Rc::downgrade(self);
            grid.connect_reveal(move |_| {
                if let Some(desktop) = weak.upgrade()
                    && desktop.panel_on.get()
                    && !desktop.panel_open.get()
                {
                    send("open");
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

    /// The empty desktop, while the files are in the panel: its menu, drops
    /// (into the Desktop folder, then shown in the panel) and keys.
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
        let items = self.window.tab().n_items();
        send(&format!("count {items}"));
        self.count.set_label(&match items {
            0 => "No items".to_owned(),
            1 => "1 item".to_owned(),
            n => format!("{n} items"),
        });
    }

    /// The free area, for the window the icons are in. In the panel's it's
    /// the whole window, but the same size, so they keep their places.
    fn apply_area(&self) {
        let (Some((x, y, width, height)), Some(grid)) = (self.area.get(), self.grid()) else {
            return;
        };
        if self.panel_on.get() {
            grid.set_area((0.0, 0.0, width, height));
        } else {
            grid.set_area((x, y, width, height));
        }
    }

    /// The Recycle Bin, for the window the icons are in: on the desktop only
    /// while it's shown there, on the panel always.
    fn apply_bin(&self) {
        let Some(grid) = self.grid() else { return };
        let place = self
            .bin
            .get()
            .filter(|_| self.panel_on.get() || self.bin_shown.get());
        match (place, self.area.get()) {
            (Some((x, y, width, height)), Some((ax, ay, _, _))) if self.panel_on.get() => {
                grid.set_bin(Some((x - ax, y - ay, width, height)));
            }
            _ => grid.set_bin(place),
        }
    }

    /// The files in the panel, or on the desktop.
    fn set_panel(self: &Rc<Self>, on: bool) {
        let has_panel = self.panel.borrow().is_some();
        self.panel_on.set(on);
        if on == has_panel {
            return;
        }
        let content = self.window.content();
        let Some(grid) = self.grid() else { return };
        if on {
            grid.set_layout(Layout::Panel);
            self.window.window.set_child(Some(&self.backdrop));
            let panel = self.make_panel(&content);
            panel.present();
            self.panel.replace(Some(panel));
        } else {
            self.panel_open.set(false);
            if let Some(panel) = self.panel.take() {
                if let Some(overlay) = panel.child().and_downcast::<gtk::Overlay>() {
                    overlay.set_child(None::<&gtk::Widget>);
                }
                panel.destroy();
            }
            grid.set_layout(Layout::Desktop);
            self.window.window.set_child(Some(&content));
        }
        self.apply_area();
        self.apply_bin();
    }

    /// The panel's window: the screen's free area, clear but for the icons
    /// and its bar. The taskbar puts the wallpaper under it, keeps it above
    /// the windows and shows it only while the panel is open.
    fn make_panel(self: &Rc<Self>, content: &gtk::Widget) -> gtk::Window {
        let panel = gtk::Window::builder()
            .application(&self.app)
            .title(PANEL_TITLE)
            .decorated(false)
            .build();
        panel.add_css_class("desktop");
        // Its menus and keys run the desktop window's commands.
        panel.insert_action_group("win", Some(&self.window.window));
        if let Some((_, _, width, height)) = self.area.get() {
            panel.set_default_size(width as i32, height as i32);
        }
        panel.connect_close_request(|_| glib::Propagation::Stop);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(|_, key, _, _| {
            if key == gdk::Key::Escape {
                send("close");
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        panel.add_controller(keys);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(content));
        overlay.add_overlay(&self.panel_bar());
        panel.set_child(Some(&overlay));
        panel
    }

    /// The panel's bar: it looks like the desktop, so this says what it is,
    /// and the ways out of it. "Open in File Manager" also reaches the icons
    /// past the screen's last column.
    fn panel_bar(&self) -> gtk::Box {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        bar.add_css_class("desktop-panel-bar");
        bar.set_halign(gtk::Align::End);
        bar.set_valign(gtk::Align::Start);
        let title = gtk::Label::new(Some("Desktop"));
        title.add_css_class("title");
        bar.append(&title);
        bar.append(&self.count);
        let open = gtk::Button::with_label("Open in File Manager");
        open.add_css_class("flat");
        open.connect_clicked(|_| open_desktop_folder());
        bar.append(&open);
        let close = crate::glyph::button("close", "Close (Esc)");
        close.connect_clicked(|_| send("close"));
        bar.append(&close);
        bar
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
                self.apply_area();
                self.apply_bin();
            }
            Some("bin") => {
                let shown = words.get(1) == Some(&"1");
                let (Some(x), Some(y), Some(width), Some(height)) =
                    (number(2), number(3), number(4), number(5))
                else {
                    return;
                };
                self.bin_shown.set(shown);
                self.bin.set(Some((x, y, width, height)));
                self.apply_bin();
            }
            Some("panel") => {
                let state = words.get(1).copied();
                self.clean.set(state == Some("clean"));
                self.set_panel(state == Some("on") || state == Some("clean"));
            }
            Some("open") => {
                self.panel_open.set(true);
                if let Some(grid) = self.grid() {
                    grid.grab_focus();
                }
            }
            Some("close") => self.panel_open.set(false),
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

// --- What the desktop's menus run ---------------------------------------------------

/// Whether the desktop's files are in the panel.
pub fn panel_on() -> bool {
    desktop().is_some_and(|desktop| desktop.panel_on.get())
}

/// Whether the panel is open.
pub fn panel_open() -> bool {
    desktop().is_some_and(|desktop| desktop.panel_open.get())
}

/// Whether the desktop is a clean one, with nothing on it.
pub fn clean() -> bool {
    desktop().is_some_and(|desktop| desktop.clean.get())
}

/// Show desktop files: asks the taskbar for the panel.
pub fn show_files() {
    send("open");
}

/// Open Recycle Bin, in a File Manager window.
pub fn open_recycle_bin() {
    open_in_file_manager(&[gio::File::for_uri("trash:///")]);
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
    let mut argv = vec![file_manager_program(std::env::current_exe().ok())];
    argv.extend(
        folders
            .iter()
            .map(|folder| OsString::from(folder.uri().as_str())),
    );
    spawn(&argv);
}

/// The program to open folders with: this one, so a copy run from a build
/// folder opens itself, unless it's gone. An update replaces the program under
/// the running desktop, and the path the system then gives for it ends in
/// " (deleted)", which nothing can start.
fn file_manager_program(running: Option<PathBuf>) -> OsString {
    running
        .filter(|path| path.exists())
        .map(PathBuf::into_os_string)
        .unwrap_or_else(|| "patrone-file-manager".into())
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

/// Personalize: Patrone Settings' Wallpaper page, or GNOME's Appearance.
pub fn personalize() {
    if glib::find_program_in_path("patrone-settings").is_some() {
        spawn(&["patrone-settings".into(), "--page".into(), "wallpaper".into()]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_an_update_replaced_is_started_by_name() {
        let replaced = PathBuf::from("/usr/bin/patrone-file-manager (deleted)");
        assert_eq!(
            file_manager_program(Some(replaced)),
            OsString::from("patrone-file-manager")
        );
    }

    #[test]
    fn a_program_still_there_is_started_from_its_path() {
        let running = std::env::current_exe().unwrap();
        assert_eq!(
            file_manager_program(Some(running.clone())),
            running.into_os_string()
        );
    }

    #[test]
    fn with_no_path_the_program_is_started_by_name() {
        assert_eq!(
            file_manager_program(None),
            OsString::from("patrone-file-manager")
        );
    }
}
