// The corner that shows the desktop's files (the user, 2026-10-30: "pick
// (optionally) a corner that is not pre-occupied with any configuration that
// shows the files until they point the mouse to that same corner again").
// With the desktop-files setting at 'hidden' nothing shows on the desktop, and
// pointing at the desktop-files-corner slides the panel out; pointing at it
// again puts it away.
//
// Only a corner nothing else uses can be one. This works out what uses each
// corner of the primary screen (claims()), keeps that in
// desktop-corners-in-use, from which both Settings apps offer the rest, and
// follows it as it changes: a chosen corner that something else takes stands
// down and says so once, rather than fight over it, and works again if it's
// freed. The choice itself is never moved.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const CORNERS = ['top-left', 'top-right', 'bottom-left', 'bottom-right'];
const BUTTONS = ['close', 'minimize', 'maximize'];
// How big the corner is: two pixels, since not every pointer reaches the last
const SIZE = 2;
// How long the pointer stays before it counts (milliseconds), so brushing the
// corner on the way to something else does nothing
const DWELL = 150;
const USED_BY = {
    taskbar: 'the taskbar',
    'window-buttons': 'the title bar buttons of maximized windows',
    'hot-corner': 'GNOME\'s hot corner',
    screen: 'another screen beside it',
};

export class DesktopCorner {
    /**
     * @param {Gio.Settings} settings - the taskbar's settings
     * @param {Function} toggle - shows or hides the files, given the side
     *   they come from: -1 for the left, 1 for the right
     */
    constructor(settings, toggle) {
        this._settings = settings;
        this._toggle = toggle;
        this._armed = null;
        this._dwellId = 0;
        this._interface = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
        this._windowManager = new Gio.Settings({schema_id: 'org.gnome.desktop.wm.preferences'});

        this._actor = new St.Widget({reactive: true, track_hover: true});
        this._actor.connect('notify::hover', () => this._hovered());

        const sync = () => this._sync();
        this._settings.connectObject(
            'changed::desktop-files', sync,
            'changed::desktop-files-corner', sync,
            'changed::layout', sync, this);
        this._interface.connectObject('changed::enable-hot-corners', sync, this);
        this._windowManager.connectObject('changed::button-layout', sync, this);
        Main.layoutManager.connectObject('monitors-changed', sync, this);
        this._sync();
    }

    destroy() {
        this._stopDwell();
        this._settings.disconnectObject(this);
        this._interface.disconnectObject(this);
        this._windowManager.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._arm(null);
        this._actor.destroy();
    }

    /**
     * What uses the primary screen's corners.
     *
     * @returns {Map<string, string>} who uses each corner in use
     */
    claims() {
        const claims = new Map();
        const claim = (corner, who) => {
            if (!claims.has(corner))
                claims.set(corner, who);
        };
        // The taskbar, with the Menu button and Show Desktop at its ends
        const edge = this._settings.get_string('layout') === 'top' ? 'top' : 'bottom';
        claim(`${edge}-left`, 'taskbar');
        claim(`${edge}-right`, 'taskbar');
        // A maximized window's buttons: Close in a corner is what a corner
        // is for
        const [left = '', right = ''] = this._windowManager.get_string('button-layout').split(':');
        const hasButtons = side => side.split(',').some(button => BUTTONS.includes(button));
        if (hasButtons(left))
            claim('top-left', 'window-buttons');
        if (hasButtons(right))
            claim('top-right', 'window-buttons');
        // GNOME's hot corner, the overview's
        if (this._interface.get_boolean('enable-hot-corners')) {
            const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
            claim(rtl ? 'top-right' : 'top-left', 'hot-corner');
        }
        // A screen beside it: the pointer goes on instead of stopping there
        for (const corner of CORNERS) {
            if (this._screenBeside(corner))
                claim(corner, 'screen');
        }
        return claims;
    }

    _screenBeside(corner) {
        const {primaryMonitor, monitors} = Main.layoutManager;
        if (!primaryMonitor)
            return false;
        const {x, y, width, height} = primaryMonitor;
        const [inX, outX] = corner.endsWith('left') ? [x, x - 1] : [x + width - 1, x + width];
        const [inY, outY] = corner.startsWith('top') ? [y, y - 1] : [y + height - 1, y + height];
        const inside = (monitor, px, py) => px >= monitor.x && px < monitor.x + monitor.width &&
            py >= monitor.y && py < monitor.y + monitor.height;
        return monitors.some(monitor => monitor !== primaryMonitor &&
            (inside(monitor, outX, inY) || inside(monitor, inX, outY)));
    }

    _sync() {
        const claims = this.claims();
        const inUse = [...claims].map(([corner, who]) => `${corner}:${who}`).sort();
        if (inUse.join() !== this._settings.get_strv('desktop-corners-in-use').join())
            this._settings.set_strv('desktop-corners-in-use', inUse);

        const corner = this._settings.get_string('desktop-files-corner');
        const wanted = this._settings.get_string('desktop-files') === 'hidden' && corner !== 'none';
        const usedBy = wanted ? claims.get(corner) : null;
        if (usedBy && this._armed === corner) {
            Main.notify('Desktop files', `The ${corner.replace('-', ' ')} corner is used by ` +
                `${USED_BY[usedBy]} now, so it no longer shows your files. You can pick another ` +
                'in Settings, under Desktop.');
        }
        this._arm(wanted && !usedBy && Main.layoutManager.primaryMonitor ? corner : null);
    }

    _arm(corner) {
        if (corner && !this._armed) {
            // Hidden with the taskbar over a fullscreen window
            Main.layoutManager.addChrome(this._actor, {trackFullscreen: true});
        } else if (!corner && this._armed) {
            this._stopDwell();
            Main.layoutManager.removeChrome(this._actor);
        }
        this._armed = corner;
        if (!corner)
            return;
        const {x, y, width, height} = Main.layoutManager.primaryMonitor;
        const size = SIZE * St.ThemeContext.get_for_stage(global.stage).scale_factor;
        this._actor.set_size(size, size);
        this._actor.set_position(
            corner.endsWith('left') ? x : x + width - size,
            corner.startsWith('top') ? y : y + height - size);
    }

    // The pointer has to leave before it can point at the corner again, so
    // the files don't go as soon as they've come
    _hovered() {
        this._stopDwell();
        if (!this._actor.hover)
            return;
        this._dwellId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, DWELL, () => {
            this._dwellId = 0;
            this._pointedAt();
            return GLib.SOURCE_REMOVE;
        });
    }

    _stopDwell() {
        if (this._dwellId)
            GLib.source_remove(this._dwellId);
        this._dwellId = 0;
    }

    // Not while something is being dragged there (a window to tile it, say),
    // in the overview, or under a dialog of the shell's
    _pointedAt() {
        const [, , mods] = global.get_pointer();
        if (!this._armed || Main.overview.visible || Main.modalCount > 0 ||
            mods & Clutter.ModifierType.BUTTON1_MASK)
            return;
        this._toggle(this._armed.endsWith('right') ? 1 : -1);
    }
}
