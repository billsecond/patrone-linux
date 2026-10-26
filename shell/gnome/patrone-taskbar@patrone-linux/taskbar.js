// The taskbar: one compact bar along the bottom of the primary monitor, in
// place of GNOME's top bar. From the left: the Start button, search, pinned
// apps and role slots, then apps that are only running; on the right, the
// top bar's own status icons and menus (network, sound, power, the
// clock and notifications), and the "Show desktop" sliver.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import GnomeDesktop from 'gi://GnomeDesktop';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as CtrlAltTab from 'resource:///org/gnome/shell/ui/ctrlAltTab.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {JumpList} from './jumpList.js';
import {RecentFiles} from './recent.js';
import {ROLES, appForRole, roleOf} from './roles.js';
import {StartMenu} from './startMenu.js';
import {taskbarWindows} from './util.js';

// A third larger than the first 20 and 16 pixels, with the bar (common.css)
const ICON_SIZE = 26;
const SEARCH_ICON_SIZE = 21;
const TOOLTIP_DELAY = 500; // ms

const TASK_MANAGERS = [
    'net.nokyan.Resources.desktop',
    'org.gnome.SystemMonitor.desktop',
    'gnome-system-monitor.desktop',
];

/** The name shown above a taskbar button while the pointer rests on it. */
class Tooltip {
    constructor() {
        this.actor = new St.Label({style_class: 'patrone-taskbar-tooltip', visible: false});
        Main.uiGroup.add_child(this.actor);
        this._timeoutId = 0;
    }

    /**
     * @param {St.Widget} actor - a taskbar button with a tooltipText
     */
    update(actor) {
        if (actor.hover) {
            this._clearTimeout();
            this._timeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, TOOLTIP_DELAY, () => {
                this._timeoutId = 0;
                this._show(actor);
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this.hide();
        }
    }

    _show(actor) {
        const text = actor.tooltipText;
        if (!text || !actor.mapped)
            return;
        this.actor.text = text;
        this.actor.show();
        Main.uiGroup.set_child_above_sibling(this.actor, null);

        const [x, y] = actor.get_transformed_position();
        const [width] = actor.get_transformed_size();
        const [, labelWidth] = this.actor.get_preferred_width(-1);
        const [, labelHeight] = this.actor.get_preferred_height(-1);
        const monitor = Main.layoutManager.findMonitorForActor(actor);
        let labelX = Math.round(x + (width - labelWidth) / 2);
        if (monitor)
            labelX = Math.max(monitor.x + 4, Math.min(labelX, monitor.x + monitor.width - labelWidth - 4));
        this.actor.set_position(labelX, Math.round(y - labelHeight - 6));
        this.actor.opacity = 0;
        this.actor.ease({opacity: 255, duration: 100, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    _clearTimeout() {
        if (this._timeoutId) {
            GLib.source_remove(this._timeoutId);
            this._timeoutId = 0;
        }
    }

    hide() {
        this._clearTimeout();
        this.actor.hide();
    }

    destroy() {
        this._clearTimeout();
        this.actor.destroy();
    }
}

/** A pinned app, a role slot, or an app that's only running. */
const TaskbarItem = GObject.registerClass(
class TaskbarItem extends St.Button {
    /**
     * @param {Taskbar} taskbar
     * @param {string|null} entry - its entry in the pinned list; null if
     *   it's only running
     * @param {Shell.App|null} app - the app it opens (a role slot's may be
     *   null, when nothing does the job)
     */
    _init(taskbar, entry, app) {
        super._init({
            style_class: 'patrone-taskbar-button patrone-taskbar-item',
            button_mask: St.ButtonMask.ONE | St.ButtonMask.TWO,
            can_focus: true,
            track_hover: true,
            y_expand: true,
        });
        this.taskbar = taskbar;
        this.entry = entry;
        this.role = roleOf(entry);
        this.app = undefined;

        // A bin layout only follows a child's alignment where the child
        // expands. Expanding sideways would stretch the button along the
        // bar, so only up and down; sideways, the layout centers anyway.
        const layout = new St.Widget({
            layout_manager: new Clutter.BinLayout(),
            y_expand: true,
        });
        this._iconBin = new St.Bin({
            y_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._indicator = new St.Widget({
            style_class: 'patrone-taskbar-indicator',
            y_expand: true,
            y_align: Clutter.ActorAlign.END,
            visible: false,
        });
        layout.add_child(this._iconBin);
        layout.add_child(this._indicator);
        this.set_child(layout);

        const rightClick = new Clutter.ClickGesture({
            required_button: Clutter.BUTTON_SECONDARY,
            recognize_on_press: true,
        });
        rightClick.connect('recognize', () => this.popupMenu());
        this.add_action(rightClick);
        this.connect('popup-menu', () => this.popupMenu());
        this.connect('notify::hover', () => taskbar.tooltip.update(this));
        this.connect('destroy', () => this._menu?.destroy());

        this.setApp(app);
    }

    get tooltipText() {
        if (this.app)
            return this.app.get_name();
        return ROLES[this.role]?.name ?? '';
    }

    setApp(app) {
        if (this.app === app)
            return;
        this.app = app;

        let icon;
        if (this.role) {
            icon = new St.Icon({
                icon_name: ROLES[this.role].icon,
                icon_size: ICON_SIZE,
                fallback_gicon: app?.app_info?.get_icon() ?? null,
            });
        } else {
            icon = app.create_icon_texture(ICON_SIZE);
        }
        this._iconBin.child?.destroy();
        this._iconBin.child = icon;
        this.accessible_name = this.tooltipText;
    }

    /** Shows whether the app is running, and whether it has the focus. */
    sync() {
        const windows = this.app ? taskbarWindows(this.app) : [];
        const focused = windows.length > 0 &&
            Shell.WindowTracker.get_default().focus_app === this.app;
        this._indicator.visible = windows.length > 0;
        if (windows.length)
            this.add_style_class_name('running');
        else
            this.remove_style_class_name('running');
        if (focused)
            this.add_style_class_name('focused');
        else
            this.remove_style_class_name('focused');
    }

    vfunc_clicked(button) {
        this.taskbar.tooltip.hide();
        this.activate(button);
    }

    /**
     * As on Windows: starts the app, or shows its window, or hides it if
     * it's in front. With several windows, each click brings the next one.
     * Middle click, or Shift or Ctrl with a click, opens a new window.
     *
     * @param {number} button
     */
    activate(button) {
        const {app} = this;
        if (!app) {
            this.popupMenu();
            return;
        }

        const event = Clutter.get_current_event();
        const modifiers = event?.get_state() ?? 0;
        const newWindow = button === Clutter.BUTTON_MIDDLE ||
            (modifiers & (Clutter.ModifierType.SHIFT_MASK | Clutter.ModifierType.CONTROL_MASK)) !== 0;
        const windows = taskbarWindows(app);

        Main.overview.hide();
        if (windows.length === 0) {
            app.activate();
        } else if (newWindow) {
            if (app.can_open_new_window())
                app.open_new_window(-1);
            else
                app.activate();
        } else if (Shell.WindowTracker.get_default().focus_app === app) {
            if (windows.length === 1)
                windows[0].minimize();
            else
                Main.activateWindow(windows.at(-1));
        } else {
            Main.activateWindow(windows[0]);
        }
    }

    popupMenu() {
        this.taskbar.tooltip.hide();
        if (!this._menu) {
            this._menu = new JumpList(this);
            this._menu.connect('open-state-changed', (menu, open) => {
                if (open)
                    this.add_style_pseudo_class('active');
                else
                    this.remove_style_pseudo_class('active');
            });
        }
        this._menu.popup();
        return Clutter.EVENT_STOP;
    }
});

/** A plain taskbar button: Start, search. */
function makeButton(taskbar, styleClass, icon, tooltipText) {
    const button = new St.Button({
        style_class: `patrone-taskbar-button ${styleClass}`,
        child: icon,
        can_focus: true,
        track_hover: true,
        y_expand: true,
        accessible_name: tooltipText,
    });
    button.tooltipText = tooltipText;
    button.connect('notify::hover', () => taskbar.tooltip.update(button));
    return button;
}

/** The clock, Windows-style: the time over the date. */
class Clock {
    constructor() {
        this.actor = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'patrone-clock',
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._time = new St.Label({style_class: 'patrone-clock-time', x_align: Clutter.ActorAlign.END});
        this._date = new St.Label({style_class: 'patrone-clock-date', x_align: Clutter.ActorAlign.END});
        this.actor.add_child(this._time);
        this.actor.add_child(this._date);

        this._interfaceSettings = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
        this._interfaceSettings.connectObject('changed::clock-format', () => this._update(), this.actor);
        this._wallClock = new GnomeDesktop.WallClock();
        this._wallClock.connectObject('notify::clock', () => this._update(), this.actor);
        this._update();
    }

    _update() {
        const now = GLib.DateTime.new_now_local();
        const twelveHour = this._interfaceSettings.get_string('clock-format') === '12h';
        this._time.text = now.format(twelveHour ? '%-I:%M %p' : '%H:%M');
        this._date.text = now.format('%x');
    }

    destroy() {
        this.actor.destroy();
        this._wallClock.run_dispose();
        this._wallClock = null;
    }
}

export class Taskbar {
    constructor(extension) {
        this.settings = extension.getSettings();
        this.recent = new RecentFiles();
        this.tooltip = new Tooltip();

        this._appSystem = Shell.AppSystem.get_default();
        this._tracker = Shell.WindowTracker.get_default();
        this._items = new Map();
        this._runningOrder = [];
        this._rebuildLater = 0;

        this.actor = new St.BoxLayout({
            name: 'patroneTaskbar',
            style_class: 'patrone-taskbar',
            reactive: true,
        });

        const startIcon = new St.Icon({
            gicon: Gio.FileIcon.new(extension.dir.resolve_relative_path('icons/patrone-start.svg')),
            icon_size: ICON_SIZE,
        });
        this._startButton = makeButton(this, 'patrone-taskbar-start', startIcon, 'Start');
        this.actor.add_child(this._startButton);

        this._searchButton = makeButton(this, 'patrone-taskbar-search',
            new St.Icon({icon_name: 'system-search-symbolic', icon_size: SEARCH_ICON_SIZE}), 'Search');
        this.actor.add_child(this._searchButton);

        this._appsBox = new St.BoxLayout({style_class: 'patrone-taskbar-apps'});
        this.actor.add_child(this._appsBox);

        this._spacer = new St.Widget({x_expand: true, reactive: true});
        this.actor.add_child(this._spacer);

        this._tray = new St.BoxLayout({style_class: 'patrone-taskbar-tray'});
        this.actor.add_child(this._tray);

        this._showDesktopButton = makeButton(this, 'patrone-taskbar-show-desktop', null, 'Show desktop');
        this._showDesktopButton.connect('clicked', () => this.toggleDesktop());
        this.actor.add_child(this._showDesktopButton);

        Main.layoutManager.addChrome(this.actor, {affectsStruts: true, trackFullscreen: true});
        Main.ctrlAltTabManager.addGroup(this.actor, 'Taskbar', 'shell-focus-top-bar-symbolic',
            {sortGroup: CtrlAltTab.SortGroup.TOP});

        this.startMenu = new StartMenu(this, this._startButton);
        this._startButton.connect('clicked', () => this.startMenu.toggle('start'));
        this._searchButton.connect('clicked', () => this.startMenu.toggle('search'));
        this._buildBackgroundMenu();

        this._hideTopBar();
        this._takeTray();
        this._takeOverlayKey();

        this.settings.connectObject(
            'changed::pinned', () => this.queueRebuild(),
            'changed::terminal-app', () => this.queueRebuild(), this);
        this._appSystem.connectObject(
            'installed-changed', () => this.queueRebuild(),
            'app-state-changed', () => this.queueRebuild(), this);
        this._tracker.connectObject(
            'tracked-windows-changed', () => this.queueRebuild(),
            'notify::focus-app', () => this._syncItems(), this);
        global.display.connectObject('notify::focus-window', () => this._syncItems(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._relayout(), this);
        this.actor.connect('style-changed', () => this._relayout());

        this._relayout();
        this._rebuild();
    }

    destroy() {
        this._returnOverlayKey();
        this._returnTray();
        this._showTopBar();

        if (this._rebuildLater) {
            global.compositor.get_laters().remove(this._rebuildLater);
            this._rebuildLater = 0;
        }
        this.settings.disconnectObject(this);
        this._appSystem.disconnectObject(this);
        this._tracker.disconnectObject(this);
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);

        this.startMenu.destroy();
        this._backgroundMenu.destroy();
        Main.ctrlAltTabManager.removeGroup(this.actor);
        this.actor.destroy();
        this.tooltip.destroy();
        this.recent.destroy();
    }

    _relayout() {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor)
            return;
        const [, height] = this.actor.get_preferred_height(monitor.width);
        this.actor.set_size(monitor.width, height);
        this.actor.set_position(monitor.x, monitor.y + monitor.height - height);
    }

    // Pinned and running apps

    isPinned(appId) {
        return this.settings.get_strv('pinned').includes(appId);
    }

    pin(appId) {
        const pinned = this.settings.get_strv('pinned');
        if (!pinned.includes(appId))
            this.settings.set_strv('pinned', [...pinned, appId]);
    }

    unpin(entry) {
        this.settings.set_strv('pinned',
            this.settings.get_strv('pinned').filter(e => e !== entry));
    }

    queueRebuild() {
        if (this._rebuildLater)
            return;
        this._rebuildLater = global.compositor.get_laters().add(
            Meta.LaterType.BEFORE_REDRAW, () => {
                this._rebuildLater = 0;
                this._rebuild();
                return GLib.SOURCE_REMOVE;
            });
    }

    _rebuild() {
        const wanted = [];
        const shown = new Set();

        for (const entry of this.settings.get_strv('pinned')) {
            const role = roleOf(entry);
            const app = role
                ? appForRole(role, this.settings)
                : this._appSystem.lookup_app(entry);
            if (!role && !app)
                continue; // uninstalled
            if (app && shown.has(app))
                continue; // a slot already shows it
            if (app)
                shown.add(app);
            wanted.push({key: entry, entry, app});
        }

        // Apps that are only running, in the order they started
        const running = this._appSystem.get_running()
            .filter(app => !shown.has(app) && taskbarWindows(app).length > 0);
        const runningIds = running.map(app => app.get_id());
        this._runningOrder = this._runningOrder.filter(id => runningIds.includes(id));
        for (const id of runningIds) {
            if (!this._runningOrder.includes(id))
                this._runningOrder.push(id);
        }
        for (const id of this._runningOrder) {
            const app = running.find(a => a.get_id() === id);
            wanted.push({key: `running:${id}`, entry: null, app});
        }

        const old = this._items;
        this._items = new Map();
        wanted.forEach(({key, entry, app}, index) => {
            let item = old.get(key);
            if (item) {
                old.delete(key);
                item.setApp(app);
            } else {
                item = new TaskbarItem(this, entry, app);
                this._appsBox.add_child(item);
            }
            this._appsBox.set_child_at_index(item, index);
            this._items.set(key, item);
        });
        for (const item of old.values())
            item.destroy();
        this._syncItems();
    }

    _syncItems() {
        for (const item of this._items.values())
            item.sync();
    }

    // Show desktop: minimizes every window on the workspace, and a second
    // click brings them back

    toggleDesktop() {
        const workspace = global.workspace_manager.get_active_workspace();
        const windows = workspace.list_windows().filter(window =>
            window.window_type === Meta.WindowType.NORMAL &&
            !window.skip_taskbar &&
            window.showing_on_its_workspace());

        if (windows.length) {
            this._desktopWindows = windows;
            for (const window of windows)
                window.minimize();
        } else if (this._desktopWindows) {
            const existing = global.display.list_all_windows();
            for (const window of this._desktopWindows) {
                if (existing.includes(window))
                    window.unminimize();
            }
            this._desktopWindows = null;
        }
    }

    // Right-click on the bar itself

    _buildBackgroundMenu() {
        this._backgroundMenu = new PopupMenu.PopupMenu(Main.layoutManager.dummyCursor, 0, St.Side.BOTTOM);
        this._backgroundMenu.actor.add_style_class_name('patrone-menu');
        Main.uiGroup.add_child(this._backgroundMenu.actor);
        this._backgroundMenu.actor.hide();
        this._backgroundMenuManager = new PopupMenu.PopupMenuManager(this.actor);
        this._backgroundMenuManager.addMenu(this._backgroundMenu);

        this._backgroundMenu.addAction('Task Manager', () => {
            const app = TASK_MANAGERS.map(id => this._appSystem.lookup_app(id)).find(Boolean);
            app?.activate();
        }, 'utilities-system-monitor-symbolic');
        this._backgroundMenu.addAction('Show desktop', () => this.toggleDesktop(),
            'user-desktop-symbolic');

        const gesture = new Clutter.ClickGesture({
            required_button: Clutter.BUTTON_SECONDARY,
            recognize_on_press: true,
        });
        gesture.connect('recognize', () => {
            const coords = gesture.get_coords_abs();
            Main.layoutManager.setDummyCursorGeometry(coords.x, coords.y, 0, 0);
            this._backgroundMenu.open(BoxPointer.PopupAnimation.FULL);
        });
        this._spacer.add_action(gesture);
    }

    // GNOME's top bar: hidden, with its status icons and menus moved into
    // the taskbar's tray

    _hideTopBar() {
        const {panelBox} = Main.layoutManager;
        panelBox.hide();
        // A hidden actor still reserves its space, unless it's untracked
        Main.layoutManager.untrackChrome(panelBox);
        Main.layoutManager._destroyPanelBarrier?.();
    }

    _showTopBar() {
        const {panelBox} = Main.layoutManager;
        Main.layoutManager.trackChrome(panelBox, {affectsStruts: true, trackFullscreen: true});
        panelBox.show();
        Main.layoutManager._updatePanelBarrier?.();
    }

    _takeTray() {
        const {panel} = Main;
        this._movedBoxes = [];
        // Status icons first, then the clock, as on Windows
        for (const box of [panel._rightBox, panel._centerBox]) {
            if (!box)
                continue;
            this._movedBoxes.push({box, index: panel.get_children().indexOf(box)});
            panel.remove_child(box);
            // GNOME's top bar buttons ask to expand, which the top bar
            // ignores; here it would stretch the tray across the bar
            box.x_expand = false;
            this._tray.add_child(box);
            box.connectObject('child-added', () => this._updateTrayMenus(), this);
        }
        this._updateTrayMenus();

        // In a desktop with no screen of its own (Patrone's, which every
        // screen and Remote Desktop client shows through GNOME Remote
        // Desktop), the screen-sharing indicator is always on, for that very
        // connection, and clicking it (any button) cuts the connection
        this._hiddenIndicators = [];
        const sharing = panel.statusArea.screenSharing?.container;
        if (sharing && global.backend.is_headless()) {
            sharing.hide();
            this._hiddenIndicators.push(sharing);
        }

        const dateMenu = panel.statusArea.dateMenu;
        this._clockDisplay = dateMenu?._clockDisplay ?? null;
        if (this._clockDisplay) {
            this._clock = new Clock();
            this._clockDisplay.get_parent().insert_child_above(this._clock.actor, this._clockDisplay);
            this._clockDisplay.hide();
        }

        this._bannerAlignment = Main.messageTray.bannerAlignment;
        Main.messageTray.bannerAlignment = Clutter.ActorAlign.END;
    }

    _returnTray() {
        Main.messageTray.bannerAlignment = this._bannerAlignment;

        for (const actor of this._hiddenIndicators)
            actor.show();
        this._hiddenIndicators = [];

        this._clock?.destroy();
        this._clock = null;
        this._clockDisplay?.show();

        for (const indicator of Object.values(Main.panel.statusArea)) {
            const menu = indicator?.menu;
            if (!menu?._patroneTray)
                continue;
            menu._boxPointer.updateArrowSide(St.Side.TOP);
            menu.actor.remove_style_class_name('patrone-tray-menu');
            delete menu._patroneTray;
        }

        for (const {box, index} of [...this._movedBoxes].sort((a, b) => a.index - b.index)) {
            box.disconnectObject(this);
            this._tray.remove_child(box);
            Main.panel.insert_child_at_index(box, index);
        }
        this._movedBoxes = [];
    }

    /** Menus of the tray's icons open upward, from the bottom of the screen. */
    _updateTrayMenus() {
        for (const indicator of Object.values(Main.panel.statusArea)) {
            const menu = indicator?.menu;
            const container = indicator?.container ?? indicator;
            if (!menu?._boxPointer || menu._patroneTray || !this._tray.contains(container))
                continue;
            menu._boxPointer.updateArrowSide(St.Side.BOTTOM);
            menu.actor.add_style_class_name('patrone-tray-menu');
            menu._patroneTray = true;
        }
    }

    // The Super key opens the Start menu instead of the Activities overview

    _takeOverlayKey() {
        GObject.signal_handlers_block_matched(global.display, {signalId: 'overlay-key'});
        this._overlayKeyId = global.display.connect('overlay-key', () => {
            this.startMenu.toggle('start');
        });
    }

    _returnOverlayKey() {
        global.display.disconnect(this._overlayKeyId);
        GObject.signal_handlers_unblock_matched(global.display, {signalId: 'overlay-key'});
    }
}
