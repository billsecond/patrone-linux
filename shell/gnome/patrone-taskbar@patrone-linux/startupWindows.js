// Windows opened at sign-in, put in place. Patrone Terminal, which
// patrone-terminal's autostart entry opens at every sign-in, goes over the
// wallpaper's logo, which shows through its see-through background, the
// way the user arranged it (2026-10-26). On Wayland an app can't place its
// own window, so the shell does it. Only the first terminal window opened
// soon after signing in is placed, and it's left alone once the user moves,
// resizes, maximizes or tiles it. Until then it follows the logo when the
// screen's size changes (a Remote Desktop window resized).

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const TERMINAL_ID = 'org.patrone.Terminal';
// How long after the desktop has started a terminal window still counts as
// the one opened at sign-in (autostarted apps open a few seconds after it)
const STARTUP_PERIOD = 30; // seconds
// The least gap between the terminal and the free area's edges
const MARGIN = 16;
// GNOME moves windows out of a new taskbar's way this long, at most, before
// it says the free area changed (17 ms in the throwaway shell)
const GNOME_MOVE_TIME = 1000 * 1000; // microseconds

const BACKGROUND_SCHEMA = 'org.gnome.desktop.background';
const INTERFACE_SCHEMA = 'org.gnome.desktop.interface';

// Patrone's wallpapers (wallpaper 01's family, branding/README.md) are 16:9,
// with the logo in the middle, a ">_" prompt at the bottom left and a dashed
// line with crosses at the top right. The terminal goes over the logo and
// between those, where the user put it. In pixels of the 1672x941 original,
// the logo is at (569, 297)-(1104, 616), the prompt ends at x 188 and the
// dashed line starts at x 1425, and the terminal goes at (190, 81)-(1393, 777).
// GNOME scales the picture to cover the screen ('zoom'), cropping its sides
// or its top and bottom to fit, so the terminal is placed in the picture,
// not the screen, and keeps to the logo at any screen size.
const PATRONE_WALLPAPER = /^patrone-back-01/;
const PICTURE_ASPECT = 1672 / 941;
const FRAME = {x0: 190 / 1672, y0: 81 / 941, x1: 1393 / 1672, y1: 777 / 941};
// Patrone Badge has the logo, small, at the bottom right (from 0.76 down,
// branding/tools/make_wallpapers.py): the terminal stops above it
const FRAMES = {
    'patrone-back-01-badge': {...FRAME, y1: 0.74},
};

/**
 * Where the wallpaper's picture lies on a monitor, as GNOME draws it.
 *
 * @param {object} monitor - x, y, width and height
 * @param {string} option - the picture-options setting
 * @returns {object} x, y, width and height
 */
function pictureBox(monitor, option) {
    let {width, height} = monitor;
    const wider = monitor.width / monitor.height > PICTURE_ASPECT;
    if (option === 'zoom' || option === 'spanned') {
        if (wider)
            height = width / PICTURE_ASPECT;
        else
            width = height * PICTURE_ASPECT;
    } else if (option === 'scaled') {
        if (wider)
            width = height * PICTURE_ASPECT;
        else
            height = width / PICTURE_ASPECT;
    }
    return {
        x: monitor.x + (monitor.width - width) / 2,
        y: monitor.y + (monitor.height - height) / 2,
        width,
        height,
    };
}

export class StartupWindows {
    constructor(settings) {
        this._settings = settings;
        this._window = null;
        // Where it was put, and when it was last moved anywhere else
        this._placed = null;
        this._movedAt = 0;
        this._timeoutId = 0;
        // Windows made since sign-in that haven't shown yet
        this._pending = new Set();

        // Only at sign-in: GNOME Shell also turns extensions off on the lock
        // screen and on again after, long after it started
        if (!Main.layoutManager._startingUp)
            return;
        global.display.connectObject('window-created',
            (_, window) => this._onWindowCreated(window), this);
        Main.layoutManager.connectObject('startup-complete', () => {
            this._timeoutId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, STARTUP_PERIOD, () => {
                this._timeoutId = 0;
                this._stopWaiting();
                return GLib.SOURCE_REMOVE;
            });
        }, this);
    }

    destroy() {
        this._stopWaiting();
        this._leave();
    }

    _stopWaiting() {
        if (this._timeoutId) {
            GLib.source_remove(this._timeoutId);
            this._timeoutId = 0;
        }
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        for (const window of this._pending)
            window.disconnectObject(this);
        this._pending.clear();
    }

    _onWindowCreated(window) {
        if (window.get_window_type() !== Meta.WindowType.NORMAL)
            return;
        // Which app it is, a Wayland app says after its window is made, and
        // it's known by the time the window shows. That's also when GNOME
        // places a new window, and so when it's placed here.
        this._pending.add(window);
        window.connectObject(
            'shown', () => this._onWindowShown(window),
            'unmanaged', () => {
                window.disconnectObject(this);
                this._pending.delete(window);
            }, this);
    }

    _onWindowShown(window) {
        window.disconnectObject(this);
        this._pending.delete(window);
        if ((window.get_gtk_application_id() ?? window.get_wm_class()) !== TERMINAL_ID)
            return;
        this._stopWaiting();
        if (!this._settings.get_boolean('place-sign-in-terminal'))
            return;

        this._window = window;
        window.connectObject(
            'position-changed', () => this._onMoved(),
            'unmanaged', () => this._leave(), this);
        global.display.connectObject(
            // The user moving or resizing it, with the mouse or the keyboard
            'grab-op-begin', (_, grabbed) => {
                if (grabbed === window)
                    this._leave();
            },
            // A new screen size, or the taskbar's
            'workareas-changed', () => this._onWorkAreasChanged(), window);
        this._place();
    }

    /** Stops looking after the window: it's closed, or the user's to place */
    _leave() {
        if (!this._window)
            return;
        this._window.disconnectObject(this);
        global.display.disconnectObject(this._window);
        this._window = null;
        this._placed = null;
    }

    _onMoved() {
        // A Wayland app reaches its new place once it has redrawn at its new
        // size, so a while after it's placed
        const {x, y} = this._window.get_frame_rect();
        if (x !== this._placed.x || y !== this._placed.y)
            this._movedAt = GLib.get_monotonic_time();
    }

    _onWorkAreasChanged() {
        const window = this._window;
        // GNOME itself moves windows out of a new strip of the screen (the
        // taskbar moved to the top) just before it says the free area
        // changed. A move before that was the user's, Ubuntu's tiling or
        // the like, as are maximizing and full screen.
        if ((this._movedAt && GLib.get_monotonic_time() - this._movedAt > GNOME_MOVE_TIME) ||
            window.get_maximize_flags() || window.is_fullscreen() ||
            !this._settings.get_boolean('place-sign-in-terminal')) {
            this._leave();
            return;
        }
        this._place();
    }

    _place() {
        const window = this._window;
        const index = window.get_monitor();
        const monitor = Main.layoutManager.monitors[index];
        if (!monitor)
            return;
        const workArea = Main.layoutManager.getWorkAreaForMonitor(index);

        const background = new Gio.Settings({schema_id: BACKGROUND_SCHEMA});
        const dark = new Gio.Settings({schema_id: INTERFACE_SCHEMA})
            .get_string('color-scheme') === 'prefer-dark';
        const uri = (dark && background.get_string('picture-uri-dark')) ||
            background.get_string('picture-uri');
        const name = GLib.path_get_basename(uri).replace(/\.[^.]*$/, '');
        // Another picture: the same fractions of the screen
        const box = PATRONE_WALLPAPER.test(name)
            ? pictureBox(monitor, background.get_string('picture-options'))
            : monitor;
        const f = FRAMES[name] ?? FRAME;

        const x0 = Math.round(Math.max(box.x + box.width * f.x0, workArea.x + MARGIN));
        const y0 = Math.round(Math.max(box.y + box.height * f.y0, workArea.y + MARGIN));
        const x1 = Math.round(Math.min(box.x + box.width * f.x1, workArea.x + workArea.width - MARGIN));
        const y1 = Math.round(Math.min(box.y + box.height * f.y1, workArea.y + workArea.height - MARGIN));
        this._placed = {x: x0, y: y0};
        this._movedAt = 0;
        window.move_resize_frame(false, x0, y0, x1 - x0, y1 - y0);
    }
}
