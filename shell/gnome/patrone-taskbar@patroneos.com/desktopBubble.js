// The desktop bubble (the user's design, 2026-10-26). With the
// desktop-bubble setting on, the desktop's files show only in a bubble that
// comes out of the left edge of the primary screen, not across the desktop.
// A small pill at that edge, shown while no window covers its spot, opens
// it when clicked or dragged out. The bubble is half an ellipse, at most
// half the screen wide and smaller with fewer files; it opens over any
// window, like a menu, and the Recycle Bin rides on its lower right edge
// while it's open. Until the pill is first opened, it bounces now and then
// with a tip saying what it does.
//
// Patrone File Manager draws the bubble and its files, in a window of its
// own (desktopHost.js, file-manager/src/desktop.rs), since 2026-10-27; this
// keeps the pill, grows that window's actor out of the edge, and carries
// the Recycle Bin on its edge. Ubuntu's desktop icons extension is off
// while File Manager draws the desktop; if it runs anyway, its windows are
// kept out of sight.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const PILL_WIDTH = 12;
const PILL_HEIGHT = 72;
// A window this close to the pill counts as covering it
const PILL_MARGIN = 6;

const OPEN_TIME = 320;
const CLOSE_TIME = 200;
// Until it's first opened, the pill bounces this often (seconds), its tip
// showing for a moment (milliseconds)
const BOUNCE_EVERY = 6;
const TIP_TIME = 3500;
const HOP = 10;
const TIP_FIRST = 'Your desktop files are in here: click, or drag it out';
// Files an app drags, held over the pill this long (milliseconds), open
// the bubble, so they can be dropped in it
const DRAG_OPEN_TIME = 500;

export class DesktopBubble {
    /**
     * @param {Gio.Settings} settings - the taskbar's settings
     * @param {object} recycleBin - the taskbar's Recycle Bin (recycleBin.js)
     * @param {object} host - File Manager's desktop (desktopHost.js)
     */
    constructor(settings, recycleBin, host) {
        this._settings = settings;
        this._bin = recycleBin;
        this._host = host;
        this._open = false;
        this._showing = false;
        this._coverLater = 0;
        this._desktopsLater = 0;
        this._bounceId = 0;
        this._firstBounceId = 0;
        this._tipId = 0;

        // The pill, its tip and the Recycle Bin while it rides on the
        // bubble: above the windows, below the taskbar and the shell's
        // menus. Among the windows' own actors, so that while the bubble is
        // open they go right above its window and under its menus and
        // tooltips (which are windows above it); Mutter puts what isn't a
        // window above the windows at every restack, and _restackLayer
        // moves it back down.
        this._layer = new St.Widget({reactive: false, visible: false});
        this._layer.add_constraint(new Clutter.BindConstraint({
            source: global.stage,
            coordinate: Clutter.BindCoordinate.SIZE,
        }));
        global.window_group.add_child(this._layer);

        this._pillIcon = new St.Icon({icon_name: 'pan-end-symbolic', style_class: 'patrone-desktop-pill-icon'});
        this._pill = new St.Bin({
            style_class: 'patrone-desktop-pill',
            reactive: true,
            track_hover: true,
            child: this._pillIcon,
            accessible_name: 'Desktop',
            opacity: 0,
        });
        this._pill.connect('notify::hover', () => {
            if (this._pill.hover)
                this._showTip(0);
            else if (!this._tipId)
                this._hideTip();
            this._shieldPointer();
        });
        this._layer.add_child(this._pill);

        const click = new Clutter.ClickGesture();
        click.connect('recognize', () => (this._open ? this.close() : this.open()));
        this._pill.add_action(click);
        this._pan = new Clutter.PanGesture();
        this._pan.connect('recognize', () => this._dragStarted());
        this._pan.connect('pan-update', () => this._dragged());
        this._pan.connect('end', () => this._dragEnded(false));
        this._pan.connect('cancel', () => this._dragEnded(true));
        this._pill.add_action(this._pan);

        this._tip = new St.Label({style_class: 'patrone-taskbar-tooltip', visible: false});
        this._layer.add_child(this._tip);

        // Where Ubuntu's desktop icons wait, unseen, if they run
        // (_hideDesktopIcons)
        this._parking = new Clutter.Actor({visible: false});
        Main.uiGroup.add_child(this._parking);

        // Something an app drags (files from File Manager), held over the
        // pill, opens the bubble to drop it in, as Windows opens a window
        // when a drag waits over its taskbar button
        this._dragOpenId = 0;
        this._appDragging = false;
        global.backend.get_dnd().connectObject(
            'dnd-enter', () => {
                this._appDragging = true;
                this._shieldPointer();
            },
            'dnd-position-change', (_, x, y) => this._appDragAt(x, y),
            'dnd-leave', () => {
                this._appDragging = false;
                this._shieldPointer();
                this._appDragAt(-1, -1);
            }, this);

        this._host.on('open', () => this.open());
        this._host.on('close', () => this.close());
        this._host.on('shape', () => this._follow());
        this._host.on('bubble-window', () => {
            if (!this._host.bubbleWindow)
                this.close(false);
            this._queueChecks();
        });

        this._settings.connectObject(
            'changed::desktop-bubble', () => this._sync(),
            'changed::show-recycle-bin', () => this.close(false),
            'changed::desktop-bubble-introduced', () => this._introduce(), this);
        global.display.connectObject(
            'workareas-changed', () => this._sync(),
            'restacked', () => this._queueChecks(),
            'window-created', (_, window) => this._watchWindow(window),
            'notify::focus-window', () => this._focusChanged(), this);
        for (const window of global.display.list_all_windows())
            this._watchWindow(window);
        global.workspace_manager.connectObject('active-workspace-changed', () => {
            this.close(false);
            this._queueChecks();
        }, this);
        Main.layoutManager.connectObject('monitors-changed', () => this._sync(), this);
        Main.overview.connectObject(
            'showing', () => this._sync(),
            'hidden', () => this._sync(), this);

        this._sync();
        this._introduce();
    }

    destroy() {
        // Nothing more is queued from here on, and File Manager's desktop
        // (ended right after this, when the taskbar is turned off, as GNOME
        // does at every screen lock) no longer calls in: its last calls
        // queued checks that ran on the destroyed pill and layer.
        this._destroyed = true;
        for (const name of ['open', 'close', 'shape', 'bubble-window'])
            this._host.on(name, null);
        this.close(false);
        this._stopIntroduction();
        for (const id of [this._tipId, this._dragOpenId]) {
            if (id)
                GLib.source_remove(id);
        }
        global.backend.get_dnd().disconnectObject(this);
        for (const later of [this._coverLater, this._desktopsLater]) {
            if (later)
                global.compositor.get_laters().remove(later);
        }
        this._settings.disconnectObject(this);
        global.display.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        Main.overview.disconnectObject(this);
        // The windows go back before their hiding place goes, or they'd go
        // with it
        this._hideDesktopIcons(false);
        this._parking.destroy();
        this._layer.destroy();
    }

    get _on() {
        return this._settings.get_boolean('desktop-bubble');
    }

    get _scale() {
        return St.ThemeContext.get_for_stage(global.stage).scale_factor;
    }

    // None until there's a screen: over Remote Desktop, a moment after
    // sign-in (monitors-changed syncs it then)
    _area() {
        if (!Main.layoutManager.primaryMonitor)
            return null;
        return Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
    }

    // How far the bubble has grown, from 0 to 1 (and a little over)
    get _grown() {
        const actor = this._host.bubbleActor;
        return this._showing && actor ? actor.scale_x : 0;
    }

    // Shows or hides it all for the setting, the overview and the screen
    _sync() {
        const on = this._on;
        this._hideDesktopIcons(on || this._host.running);
        if (!on || Main.overview.visible)
            this.close(false);
        this._layer.visible = on && !Main.overview.visible;
        this._follow();
        this._queueChecks();
    }

    /** Opens the bubble, growing out of the left edge. */
    open() {
        if (this._prepare())
            this._settle(true);
    }

    /**
     * Closes the bubble.
     *
     * @param {boolean} animate - shrink back into the edge, or go at once
     */
    close(animate = true) {
        if (!this._showing)
            return;
        const actor = this._host.bubbleActor;
        if (animate && actor) {
            this._settle(false);
            return;
        }
        actor?.remove_all_transitions();
        this._closed();
    }

    // The bubble's window, shown at nothing, ready to grow, with the
    // keyboard
    _prepare() {
        if (this._showing)
            return true;
        const actor = this._host.bubbleActor;
        if (!this._layer.visible || !actor || !this._host.shape || !this._area())
            return false;
        this._showing = true;
        this._returnFocus = global.display.focus_window;
        actor.remove_all_transitions();
        actor.set_pivot_point(0, 0.5);
        actor.set_scale(0, 0);
        actor.show();
        actor.connectObject('notify::scale-x', () => this._follow(), this);
        this._host.tell('open');
        this._host.focusBubble();
        this._pill.add_style_class_name('open');
        this._pillIcon.icon_name = 'pan-start-symbolic';
        this._hideTip();
        this._holdBin();
        this._restackLayer();
        // A click on the taskbar or a menu of the shell's closes it too
        global.stage.connectObject('captured-event', (_, event) => this._stageEvent(event), this);
        if (!this._settings.get_boolean('desktop-bubble-introduced'))
            this._settings.set_boolean('desktop-bubble-introduced', true);
        return true;
    }

    // Grows it to full size, or shrinks it away
    _settle(open) {
        const actor = this._host.bubbleActor;
        if (!actor) {
            this._closed();
            return;
        }
        this._open = open;
        actor.remove_all_transitions();
        actor.ease({
            scale_x: open ? 1 : 0,
            scale_y: open ? 1 : 0,
            duration: open ? OPEN_TIME : CLOSE_TIME,
            mode: open ? Clutter.AnimationMode.EASE_OUT_BACK : Clutter.AnimationMode.EASE_IN_QUAD,
            onComplete: () => {
                if (!open && !this._open)
                    this._closed();
            },
        });
    }

    _closed() {
        const actor = this._host.bubbleActor;
        const bubbleWindow = this._host.bubbleWindow;
        this._open = false;
        this._showing = false;
        global.stage.disconnectObject(this);
        if (actor) {
            actor.disconnectObject(this);
            actor.hide();
            actor.set_scale(1, 1);
        }
        this._host.tell('close');
        this._pill.remove_style_class_name('open');
        this._pillIcon.icon_name = 'pan-end-symbolic';
        this._releaseBin();
        // The keyboard back where it was, or nowhere: the bubble's unseen
        // window mustn't keep it
        const back = this._returnFocus;
        this._returnFocus = null;
        if (bubbleWindow && global.display.focus_window === bubbleWindow) {
            if (back && back !== bubbleWindow && !back.minimized &&
                global.display.list_all_windows().includes(back))
                back.activate(global.get_current_time());
            else
                global.display.unset_input_focus(global.get_current_time());
        }
        this._follow();
        this._queueChecks();
    }

    // Like a menu: anything else taking the keyboard closes it. The
    // bubble's own menus are popups of its window, which keeps the keyboard.
    _focusChanged() {
        const focus = global.display.focus_window;
        const bubbleWindow = this._host.bubbleWindow;
        if (!this._showing) {
            // Unseen, its window mustn't take the keyboard (GNOME gives it
            // to it when the last window closes)
            if (focus && focus === bubbleWindow)
                global.display.unset_input_focus(global.get_current_time());
            this._keepFocusOffDesktop();
            return;
        }
        if (focus && focus !== bubbleWindow && focus.get_transient_for() !== bubbleWindow)
            this.close();
    }

    // A press on the shell itself (the taskbar, its menus) closes it; one
    // on a window is the windows' business (it takes the keyboard), and
    // the pill and the Recycle Bin have their own
    _stageEvent(event) {
        const type = event.type();
        if (type !== Clutter.EventType.BUTTON_PRESS && type !== Clutter.EventType.TOUCH_BEGIN)
            return Clutter.EVENT_PROPAGATE;
        const target = global.stage.get_event_actor(event);
        const inside = actor => {
            for (let current = target; current; current = current.get_parent()) {
                if (current === actor)
                    return true;
            }
            return false;
        };
        if (!inside(global.window_group) && !inside(this._pill) &&
            !(this._bin?.actor && inside(this._bin.actor)) && !this._isMenu(target))
            this.close();
        return Clutter.EVENT_PROPAGATE;
    }

    // The Recycle Bin's menu, opened while it rides on the bubble
    _isMenu(actor) {
        for (let current = actor; current; current = current.get_parent()) {
            if (current.has_style_class_name?.('patrone-menu'))
                return true;
        }
        return false;
    }

    // Dragging the pill pulls the bubble out (or pushes it back) with the
    // pointer; let go, it opens if it was pulled out far enough
    _dragStarted() {
        if (!this._prepare()) {
            this._dragFrom = null;
            return;
        }
        const actor = this._host.bubbleActor;
        actor.remove_all_transitions();
        this._dragFrom = actor.scale_x;
        this._dragGrab = global.stage.grab(this._pill);
        this._shieldPointer();
    }

    // An app's drag at (x, y), or gone (-1, -1): waiting over the pill,
    // it opens the bubble
    _appDragAt(x, y) {
        const [px, py] = this._pill.get_transformed_position();
        const [width, height] = this._pill.get_transformed_size();
        const over = this._pill.visible && !this._showing &&
            x >= px && x < px + width && y >= py && y < py + height;
        if (!over) {
            if (this._dragOpenId)
                GLib.source_remove(this._dragOpenId);
            this._dragOpenId = 0;
            return;
        }
        if (this._dragOpenId)
            return;
        this._dragOpenId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, DRAG_OPEN_TIME, () => {
            this._dragOpenId = 0;
            this.open();
            return GLib.SOURCE_REMOVE;
        });
    }

    // While the pointer's on the pill, or the pill is dragged, File
    // Manager's windows under it don't take the pointer (desktopHost.js)
    // (Not while an app drags something: the bubble's window must take its
    // drop.)
    _shieldPointer() {
        this._host.shieldPointer('pill',
            (this._pill.hover || !!this._dragGrab) && !this._appDragging);
    }

    _dragged() {
        const actor = this._host.bubbleActor;
        if (this._dragFrom === null || this._dragFrom === undefined || !actor || !this._host.shape)
            return;
        const delta = this._pan.get_accumulated_delta_abs().get_x();
        const scale = Math.min(Math.max(this._dragFrom + delta / this._host.shape.a, 0), 1.08);
        actor.set_scale(scale, scale);
    }

    _dragEnded(cancelled) {
        this._dragGrab?.dismiss();
        this._dragGrab = null;
        this._shieldPointer();
        if (this._dragFrom === null || this._dragFrom === undefined)
            return;
        const wasOpen = this._dragFrom >= 0.5;
        const reached = this._grown;
        this._dragFrom = null;
        this._settle(cancelled ? wasOpen : reached > (wasOpen ? 0.75 : 0.25));
    }

    // The pill, and the Recycle Bin while it's held, follow the bubble's edge
    _follow() {
        const area = this._area();
        if (!area)
            return;
        const scale = this._scale;
        const shape = this._host.shape;
        const [pillWidth, pillHeight] = [PILL_WIDTH * scale, PILL_HEIGHT * scale];
        const middle = area.y + area.height / 2;
        const grown = this._grown;
        const x = grown > 0 && shape ? area.x + shape.a * grown - pillWidth / 2 : area.x;
        this._pill.set_size(pillWidth, pillHeight);
        this._pill.set_position(Math.round(x), Math.round(middle - pillHeight / 2));

        if (this._binFrom && shape?.bin) {
            // From where it sat towards its spot on the edge, which moves out
            // as the bubble grows
            const [width, height] = this._bin.actor.get_size();
            const spotX = area.x + (shape.bin[0] + width / 2) * grown - width / 2;
            const spotY = middle + (shape.bin[1] + height / 2 - shape.b) * grown - height / 2;
            const along = this._binOnEdge ? 1 : Math.min(Math.max(grown, 0), 1);
            const x = this._binFrom[0] + (spotX - this._binFrom[0]) * along;
            const y = this._binFrom[1] + (spotY - this._binFrom[1]) * along;
            if (Number.isFinite(x) && Number.isFinite(y))
                this._bin.hold(this._layer, Math.round(x), Math.round(y));
        }
    }

    // The Recycle Bin rides on its edge even while it's off the desktop
    // (the user, 2026-10-27)
    _holdBin() {
        if (!this._bin?.hold || !this._host.shape?.bin)
            return;
        // Off the desktop, it has no corner to come from: it rides the edge
        // from the start. (It has never been drawn then, and its place reads
        // as NaN: taken as where it came from, that put it nowhere, and the
        // bubble opened without it, taskbar 0.4.2.)
        this._binOnEdge = !this._bin.actor.visible;
        const from = this._bin.actor.get_transformed_position();
        this._binFrom = from.every(Number.isFinite) ? from : [0, 0];
        if (this._binOnEdge)
            this._follow();
        else
            this._bin.hold(this._layer, ...this._binFrom.map(Math.round));
    }

    _releaseBin() {
        if (!this._binFrom)
            return;
        this._binFrom = null;
        this._bin.release();
    }

    // Ubuntu's desktop icons extension's windows, which cover the screen,
    // by that extension's own mark: the Recycle Bin's drop target and File
    // Manager's desktop are desktop windows too, and must stay
    _isDesktopIcons(window) {
        return !!window?.customJS_ding;
    }

    _watchWindow(window) {
        window?.connectObject(
            'position-changed', () => this._queueCoverCheck(),
            'size-changed', () => this._queueCoverCheck(),
            'notify::minimized', () => this._queueCoverCheck(),
            'notify::window-type', () => this._queueChecks(), this._layer);
        this._queueChecks();
    }

    _queueChecks() {
        this._queueCoverCheck();
        if (this._desktopsLater || this._destroyed)
            return;
        this._desktopsLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._desktopsLater = 0;
            this._hideDesktopIcons(this._on || this._host.running);
            this._restackLayer();
            return GLib.SOURCE_REMOVE;
        });
    }

    // Right above the bubble's window while it's open, else above them all
    _restackLayer() {
        const actor = this._host.bubbleActor;
        if (this._showing && actor?.get_parent() === global.window_group)
            global.window_group.set_child_above_sibling(this._layer, actor);
        else
            global.window_group.set_child_above_sibling(this._layer, null);
    }

    // Ubuntu's desktop icons' windows wait in a hidden group of their own,
    // where they neither draw nor take clicks. (Clipped to nothing, they
    // still drew.) Mutter keeps restacking them there, as it does windows
    // GNOME Shell's own effects move. They're the window group's own
    // children, since that extension takes its windows out of
    // global.get_window_actors() on Wayland.
    _hideDesktopIcons(hide) {
        if (hide) {
            for (const actor of global.window_group.get_children()) {
                if (actor instanceof Meta.WindowActor && this._isDesktopIcons(actor.meta_window)) {
                    global.window_group.remove_child(actor);
                    this._parking.add_child(actor);
                }
            }
            this._keepFocusOffDesktop();
            return;
        }
        // Back right above the wallpaper, under every other window
        for (const actor of this._parking.get_children().reverse()) {
            this._parking.remove_child(actor);
            global.window_group.add_child(actor);
            global.window_group.set_child_above_sibling(actor, Main.layoutManager._backgroundGroup);
        }
    }

    // Unseen, Ubuntu's desktop icons' window mustn't take the keyboard (it
    // gets it when the last window closes), or keys would act on hidden
    // files
    _keepFocusOffDesktop() {
        const window = global.display.focus_window;
        if (this._isDesktopIcons(window) && this._parking.get_n_children() > 0)
            global.display.unset_input_focus(global.get_current_time());
    }

    _queueCoverCheck() {
        if (this._coverLater || this._destroyed)
            return;
        this._coverLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._coverLater = 0;
            this._checkCover();
            return GLib.SOURCE_REMOVE;
        });
    }

    // The pill shows while no window covers its spot, and while there's a
    // bubble to open
    _checkCover() {
        const area = this._area();
        if (!area || this._showing)
            return;
        const scale = this._scale;
        const margin = PILL_MARGIN * scale;
        const top = area.y + area.height / 2 - PILL_HEIGHT * scale / 2 - margin;
        const bottom = top + PILL_HEIGHT * scale + 2 * margin;
        const right = area.x + (PILL_WIDTH + HOP) * scale + margin;
        const workspace = global.workspace_manager.get_active_workspace();
        const bubbleWindow = this._host.bubbleWindow;
        const covered = global.display.list_all_windows().some(window => {
            if (window === bubbleWindow || this._isDesktopIcons(window) ||
                window.get_window_type() === Meta.WindowType.DESKTOP ||
                window.minimized || !window.located_on_workspace(workspace) ||
                !window.showing_on_its_workspace() || !window.get_compositor_private()?.visible)
                return false;
            const frame = window.get_frame_rect();
            return frame.x < right && frame.x + frame.width > area.x &&
                frame.y < bottom && frame.y + frame.height > top;
        });
        const show = this._layer.visible && !covered && !!bubbleWindow;
        if (show === this._pill.visible && this._pill.opacity === (show ? 255 : 0))
            return;
        this._pill.remove_transition('opacity');
        if (show) {
            this._pill.show();
            this._pill.ease({opacity: 255, duration: 180, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        } else {
            this._hideTip();
            this._pill.ease({
                opacity: 0,
                duration: 120,
                mode: Clutter.AnimationMode.EASE_OUT_QUAD,
                onComplete: () => this._pill.hide(),
            });
        }
    }

    // Until it's first opened, the pill bounces now and then with its tip
    _introduce() {
        if (this._settings.get_boolean('desktop-bubble-introduced')) {
            this._stopIntroduction();
            return;
        }
        if (this._bounceId)
            return;
        this._firstBounceId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1500, () => {
            this._firstBounceId = 0;
            this._bounce();
            return GLib.SOURCE_REMOVE;
        });
        this._bounceId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, BOUNCE_EVERY, () => {
            this._bounce();
            return GLib.SOURCE_CONTINUE;
        });
    }

    _stopIntroduction() {
        for (const id of [this._bounceId, this._firstBounceId]) {
            if (id)
                GLib.source_remove(id);
        }
        this._bounceId = this._firstBounceId = 0;
        this._pill.remove_transition('translation-x');
        this._pill.translation_x = 0;
    }

    _bounce() {
        if (!this._layer.visible || !this._pill.visible || this._pill.opacity < 255 || this._showing)
            return;
        this._pill.ease({
            translation_x: HOP * this._scale,
            duration: 160,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onComplete: () => this._pill.ease({
                translation_x: 0,
                duration: 500,
                mode: Clutter.AnimationMode.EASE_OUT_BOUNCE,
            }),
        });
        this._showTip(TIP_TIME);
    }

    /**
     * Shows the tip beside the pill.
     *
     * @param {number} time - how long, in milliseconds; 0 while it's hovered
     */
    _showTip(time) {
        if (this._showing || !this._pill.visible)
            return;
        const count = this._host.count;
        this._tip.text = this._settings.get_boolean('desktop-bubble-introduced')
            ? `Desktop: ${count === 1 ? '1 item' : `${count} items`}`
            : TIP_FIRST;
        if (!this._tip.visible) {
            this._tip.opacity = 0;
            this._tip.show();
            this._tip.ease({opacity: 255, duration: 120, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        }
        // Measured once shown, so with its style
        const [, height] = this._tip.get_preferred_height(-1);
        this._tip.set_position(
            Math.round(this._pill.x + this._pill.width + 8 * this._scale),
            Math.round(this._pill.y + (this._pill.height - height) / 2));
        this._layer.set_child_above_sibling(this._tip, null);
        if (this._tipId)
            GLib.source_remove(this._tipId);
        this._tipId = 0;
        if (time > 0) {
            this._tipId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, time, () => {
                this._tipId = 0;
                if (!this._pill.hover)
                    this._hideTip();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    _hideTip() {
        if (this._tipId)
            GLib.source_remove(this._tipId);
        this._tipId = 0;
        this._tip.hide();
    }
}
