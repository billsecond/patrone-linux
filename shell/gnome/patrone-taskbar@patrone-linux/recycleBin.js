// The Recycle Bin on the desktop. Patrone draws it, in place of the desktop
// icons extension's trash, so that it can stay docked to a corner of the
// screen: bottom right unless moved (the user asked for it on 2026-10-26).
// Double-click opens it in the file manager. Right-click empties it, picks
// the corner or takes it off the desktop. Dragged while docked, it docks to
// the nearest corner; not docked, it stays where it's dropped. Files and
// folders dropped on it go to the trash, through a clear window under it
// (dropTarget.js), and the desktop icons keep out of its column. Taken off
// the desktop, GNOME's desktop menu offers it back.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as BackgroundMenu from 'resource:///org/gnome/shell/ui/backgroundMenu.js';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Dialog from 'resource:///org/gnome/shell/ui/dialog.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const TRASH = 'trash:///';
const NAME = 'Recycle Bin';
const ICON_SIZE = 48;
// From the edges of the screen, inside the taskbar
const MARGIN = 12;

// The desktop icons extension, and the ID its area API answers to (Ubuntu's
// dock uses the same, desktopIconsIntegration.js)
const DING_UUID = 'ding@rastersoft.com';
const DING_AREA_ID = '130cbc66-235c-4bd6-8571-98d2d8bba5e2';
const OWN_UUID = 'patrone-taskbar@patrone-linux';
// Before the drop target starts again, if it ends on its own
const RESTART_DELAY = 3; // seconds
// How often it makes sure it's still right above the desktop icons
const STACK_CHECK = 2; // seconds

// The corners it can stay docked to (its recycle-bin-dock setting), and
// "none" for wherever it's put
const DOCKS = [
    ['bottom-right', 'Bottom right'],
    ['bottom-left', 'Bottom left'],
    ['top-right', 'Top right'],
    ['top-left', 'Top left'],
    ['none', 'Not docked'],
];

export class RecycleBin {
    /**
     * @param {Gio.Settings} settings - the taskbar's settings
     * @param {Gio.File} iconsDir - the taskbar's icons folder
     */
    constructor(settings, iconsDir) {
        this._settings = settings;
        this._iconsDir = iconsDir;
        this._dropTargetPath = iconsDir.get_parent().get_child('dropTarget.js').get_path();
        this._items = 0;
        this._restackLater = 0;
        this._restartId = 0;

        this._icon = new St.Icon({icon_size: ICON_SIZE});
        const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL});
        box.add_child(this._icon);
        box.add_child(new St.Label({
            text: NAME,
            style_class: 'patrone-desktop-icon-label',
            x_align: Clutter.ActorAlign.CENTER,
        }));
        this.actor = new St.Bin({
            style_class: 'patrone-desktop-icon',
            reactive: true,
            track_hover: true,
            child: box,
            accessible_name: NAME,
        });
        // On the desktop: above the wallpaper and the desktop icons' window,
        // below every other window
        global.window_group.add_child(this.actor);

        const open = new Clutter.ClickGesture({n_clicks_required: 2});
        open.connect('recognize', () => this.open());
        this.actor.add_action(open);

        const rightClick = new Clutter.ClickGesture({
            required_button: Clutter.BUTTON_SECONDARY,
            recognize_on_press: true,
        });
        rightClick.connect('recognize', () => this._openMenu());
        this.actor.add_action(rightClick);

        // Dragging holds the pointer, or the windows it passes over (the
        // desktop icons' covers the whole screen) take the rest of the drag
        this._pan = new Clutter.PanGesture();
        this._pan.connect('recognize', () => {
            this._dragFrom = [this.actor.x, this.actor.y];
            this._grab = global.stage.grab(this.actor);
        });
        this._pan.connect('pan-update', () => {
            const delta = this._pan.get_accumulated_delta_abs();
            this.actor.set_position(
                Math.round(this._dragFrom[0] + delta.get_x()),
                Math.round(this._dragFrom[1] + delta.get_y()));
        });
        this._pan.connect('end', () => {
            this._endGrab();
            this._dropped();
        });
        this._pan.connect('cancel', () => {
            this._endGrab();
            this._place();
        });
        this.actor.add_action(this._pan);

        // While an app drags something (a file from the desktop or a file
        // manager), the drop goes through to the drop target under it, and
        // the Recycle Bin lights up when it's over it
        global.backend.get_dnd().connectObject(
            'dnd-enter', () => {
                this.actor.reactive = false;
            },
            'dnd-position-change', (_, x, y) => this._dragOver(x, y),
            'dnd-leave', () => {
                this.actor.reactive = true;
                this._dragOver(-1, -1);
            }, this);

        // The desktop icons keep out of its column, through their area API,
        // whenever that extension is (re)started
        Main.extensionManager.connectObject('extension-state-changed', (_, extension) => {
            if (extension?.uuid === DING_UUID)
                this._reserveArea();
        }, this);

        this._buildMenu();

        // Taken off the desktop, it's offered back in GNOME's desktop menu:
        // the one a right-click on the empty desktop opens while the desktop
        // bubble keeps the desktop icons out of sight (the user, 2026-10-27).
        // GNOME makes one for each screen and wallpaper, so the item goes in
        // as each opens.
        this._desktopMenuItems = new Set();
        this._injections = new InjectionManager();
        const bin = this;
        this._injections.overrideMethod(BackgroundMenu.BackgroundMenu.prototype, 'open',
            open => function (...args) {
                bin._offerInDesktopMenu(this);
                return open.call(this, ...args);
            });

        this._trash = Gio.File.new_for_uri(TRASH);
        try {
            this._monitor = this._trash.monitor_directory(Gio.FileMonitorFlags.NONE, null);
            this._monitor.connect('changed', () => this._countItems());
        } catch (e) {
            console.warn(`Patrone taskbar: can't watch the Recycle Bin: ${e.message}`);
        }
        this._countItems();

        this._settings.connectObject(
            'changed::show-recycle-bin', () => this._place(),
            'changed::recycle-bin-dock', () => this._place(),
            'changed::recycle-bin-position', () => this._place(), this);
        global.display.connectObject(
            'workareas-changed', () => this._place(),
            'restacked', () => this._queueRestack(),
            'window-created', (_, window) => this._watchWindow(window), this);
        for (const window of global.display.list_all_windows())
            this._watchWindow(window);
        Main.layoutManager.connectObject('monitors-changed', () => this._place(), this);
        // Its size is only known once it's styled, after it's on the stage
        this.actor.connect('notify::width', () => this._place());
        this.actor.connect('notify::height', () => this._place());

        this._place();
        this._restack();
        // And a look every couple of seconds, in case a restack was missed
        // (the user's desktop showed the desktop icons over it once more,
        // after they'd restarted, and it couldn't be checked from here)
        this._stackCheckId = GLib.timeout_add_seconds(GLib.PRIORITY_LOW, STACK_CHECK, () => {
            if (!this._stackedRight())
                this._queueRestack();
            return GLib.SOURCE_CONTINUE;
        });
    }

    destroy() {
        this._destroyed = true;
        GLib.source_remove(this._stackCheckId);
        this._endGrab();
        this._stopDropTarget();
        this._areaApi()?.setMarginsForExtension(OWN_UUID, null);
        this._injections.clear();
        for (const item of this._desktopMenuItems)
            item.destroy();
        this._monitor?.cancel();
        if (this._restackLater)
            global.compositor.get_laters().remove(this._restackLater);
        this._settings.disconnectObject(this);
        global.display.disconnectObject(this);
        global.backend.get_dnd().disconnectObject(this);
        Main.extensionManager.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._menu.destroy();
        this.actor.destroy();
    }

    /** Opens it in the file manager. */
    open() {
        try {
            Gio.AppInfo.launch_default_for_uri(TRASH, global.create_app_launch_context(0, -1));
        } catch (e) {
            Main.notifyError(`Can't open the ${NAME}`, e.message);
        }
    }

    /**
     * Holds it away from its own place: the desktop bubble (desktopBubble.js)
     * lifts it above the windows while it's open, onto the bubble's edge.
     * Called again to move it; release() puts it back. Dropped while held,
     * it still docks where it was dropped, and comes back here.
     *
     * @param {Clutter.Actor} parent - where it goes, above the windows
     * @param {number} x - across, in the parent
     * @param {number} y - down, in the parent
     */
    hold(parent, x, y) {
        if (this.actor.get_parent() !== parent) {
            this.actor.get_parent()?.remove_child(this.actor);
            parent.add_child(this.actor);
            this.actor.add_style_class_name('on-bubble');
            // Left uncovered in its corner, the drop target's all but clear
            // window showed as a faint square
            this._showDropTarget(false);
        }
        this._held = [x, y];
        this.actor.visible = true;
        this.actor.set_position(x, y);
    }

    /** Back on the desktop, in its own place. */
    release() {
        if (!this._held)
            return;
        this._held = null;
        this.actor.remove_style_class_name('on-bubble');
        this.actor.get_parent()?.remove_child(this.actor);
        global.window_group.add_child(this.actor);
        this._showDropTarget(true);
        this._place();
        this._restack();
    }

    // "Show Recycle Bin" in one of GNOME's desktop menus, under Change
    // Background, added the first time it opens and shown while the bin's
    // off the desktop
    _offerInDesktopMenu(menu) {
        let item = menu._patroneShowBin;
        if (!item) {
            item = new PopupMenu.PopupMenuItem(`Show ${NAME}`);
            item.connect('activate', () => this._settings.set_boolean('show-recycle-bin', true));
            item.connect('destroy', () => {
                this._desktopMenuItems.delete(item);
                delete menu._patroneShowBin;
            });
            menu.addMenuItem(item, 1);
            menu._patroneShowBin = item;
            this._desktopMenuItems.add(item);
        }
        item.visible = !this._settings.get_boolean('show-recycle-bin');
    }

    // Shows or hides the drop target's window (a hidden window actor neither
    // draws nor takes the pointer; a desktop window can't be minimized)
    _showDropTarget(shown) {
        const actor = this._dropWindow?.get_compositor_private();
        if (actor)
            actor.visible = shown;
    }

    _countItems() {
        this._trash.query_info_async('trash::item-count', Gio.FileQueryInfoFlags.NONE,
            GLib.PRIORITY_LOW, null, (file, result) => {
                try {
                    this._items = file.query_info_finish(result).get_attribute_uint32('trash::item-count');
                } catch {
                    this._items = 0;
                }
                const name = this._items > 0 ? 'recycle-bin-full.svg' : 'recycle-bin.svg';
                this._icon.gicon = new Gio.FileIcon({file: this._iconsDir.get_child(name)});
                this._emptyItem.setSensitive(this._items > 0);
            });
    }

    // Where it goes: the docked corner of the primary monitor's work area
    // (inside the taskbar), or where it was dropped, kept as a fraction of
    // the work area so it stays put when the screen size changes
    _place() {
        // Held on the desktop bubble's edge, it shows there even while it's
        // off the desktop (the user, 2026-10-27)
        if (this._held) {
            this.actor.visible = true;
            this.actor.set_position(...this._held);
            return;
        }
        this.actor.visible = this._settings.get_boolean('show-recycle-bin');
        if (!this.actor.visible) {
            this._stopDropTarget();
            this._reserveArea();
            return;
        }
        // Over Remote Desktop the desktop starts with no screen, until the
        // client's arrives (monitors-changed places it then). Read as
        // nothing, the screen's size took the whole taskbar down at the
        // user's sign-in (2026-10-27).
        if (!Main.layoutManager.primaryMonitor)
            return;
        const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        if (!area)
            return;
        const [, width] = this.actor.get_preferred_width(-1);
        const [, height] = this.actor.get_preferred_height(width);
        const dock = this._settings.get_string('recycle-bin-dock');
        let x, y;
        if (dock === 'none') {
            const [fx, fy] = this._settings.get_value('recycle-bin-position').deep_unpack();
            x = area.x + fx * (area.width - width);
            y = area.y + fy * (area.height - height);
        } else {
            x = dock.endsWith('left') ? area.x + MARGIN : area.x + area.width - width - MARGIN;
            y = dock.startsWith('top') ? area.y + MARGIN : area.y + area.height - height - MARGIN;
        }
        this.actor.set_position(Math.round(x), Math.round(y));
        this._startDropTarget();
        this._placeDropTarget();
        this._reserveArea();
    }

    // The desktop icons' area API, when that extension is running
    _areaApi() {
        const extension = Main.extensionManager.lookup(DING_UUID);
        const api = extension?.stateObj?.DesktopIconsUsableArea;
        return api?.uuid === DING_AREA_ID ? api : null;
    }

    // Keeps the desktop icons out of the strip of the screen the Recycle Bin
    // is in, from the side it's nearest to, so none sits under it where it
    // can't be clicked (the user moved a folder there, 2026-10-26). The API
    // takes whole margins, so that's its column, the width of one icon.
    _reserveArea() {
        const api = this._areaApi();
        if (!api)
            return;
        if (!this.actor.visible) {
            api.setMarginsForExtension(OWN_UUID, null);
            return;
        }
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor)
            return;
        const [x, width] = [this.actor.x, this.actor.width];
        const left = x + width / 2 < monitor.x + monitor.width / 2;
        api.setMarginsForExtension(OWN_UUID, {
            [Main.layoutManager.primaryIndex]: {
                top: 0,
                bottom: 0,
                left: left ? x - monitor.x + width + MARGIN : 0,
                right: left ? 0 : monitor.x + monitor.width - x + MARGIN,
            },
        });
    }

    // Lights up while something an app drags is over it
    _dragOver(x, y) {
        const [ax, ay] = this.actor.get_transformed_position();
        const [width, height] = this.actor.get_transformed_size();
        const over = this.actor.visible && x >= ax && x < ax + width && y >= ay && y < ay + height;
        if (over)
            this.actor.add_style_pseudo_class('drop');
        else
            this.actor.remove_style_pseudo_class('drop');
    }

    _startDropTarget() {
        if (this._dropClient || this._restartId || this._destroyed)
            return;
        const [width, height] = this.actor.get_size();
        if (width <= 0 || height <= 0)
            return; // not styled yet
        try {
            const launcher = new Gio.SubprocessLauncher({flags: Gio.SubprocessFlags.NONE});
            const client = Meta.WaylandClient.new_subprocess(global.context, launcher,
                ['gjs', '-m', this._dropTargetPath, `${Math.round(width)}`, `${Math.round(height)}`]);
            this._dropClient = client;
            client.get_subprocess().wait_async(null, () => {
                if (this._dropClient !== client)
                    return;
                // Ended on its own: again in a moment, while it's wanted
                this._dropClient = null;
                this._dropWindow = null;
                this._restartId = GLib.timeout_add_seconds(GLib.PRIORITY_LOW, RESTART_DELAY, () => {
                    this._restartId = 0;
                    if (this.actor.visible)
                        this._startDropTarget();
                    return GLib.SOURCE_REMOVE;
                });
            });
        } catch (e) {
            console.warn(`Patrone taskbar: can't start the Recycle Bin's drop target: ${e.message}`);
        }
    }

    _stopDropTarget() {
        if (this._restartId) {
            GLib.source_remove(this._restartId);
            this._restartId = 0;
        }
        const client = this._dropClient;
        this._dropClient = null;
        this._dropWindow = null;
        client?.get_subprocess()?.force_exit();
    }

    // The drop target's window: a desktop window like the desktop icons'
    // (no taskbar button, no Alt+Tab, no focus), on every workspace, right
    // under the Recycle Bin
    _adoptDropTarget(window) {
        this._dropWindow = window;
        const focused = global.display.focus_window;
        // Only once it's shown: made a desktop window, or moved, as soon as
        // it was created, it never showed
        window.connectObject('shown', () => {
            window.set_type?.(Meta.WindowType.DESKTOP);
            window.stick();
            this._placeDropTarget();
            this._queueRestack();
            if (this._held)
                this._showDropTarget(false);
            // Showing gave it the focus: back to the window that had it, or
            // to none when none did (it kept it at sign-in)
            if (global.display.focus_window === window) {
                if (focused && focused !== window)
                    focused.activate(global.get_current_time());
                else
                    global.display.unset_input_focus(global.get_current_time());
            }
        }, this.actor);
    }

    _placeDropTarget() {
        const [width, height] = this.actor.get_size();
        this._dropWindow?.move_resize_frame(false, this.actor.x, this.actor.y,
            Math.round(width), Math.round(height));
    }

    _endGrab() {
        this._grab?.dismiss();
        this._grab = null;
    }

    _dropped() {
        const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        const [width, height] = this.actor.get_size();
        if (this._settings.get_string('recycle-bin-dock') === 'none') {
            const fraction = (offset, room) => Math.min(Math.max(offset / Math.max(room, 1), 0), 1);
            this._settings.set_value('recycle-bin-position', new GLib.Variant('(dd)', [
                fraction(this.actor.x - area.x, area.width - width),
                fraction(this.actor.y - area.y, area.height - height),
            ]));
        } else {
            // Docked: to the corner nearest where it was dropped
            const right = this.actor.x + width / 2 > area.x + area.width / 2;
            const bottom = this.actor.y + height / 2 > area.y + area.height / 2;
            this._settings.set_string('recycle-bin-dock',
                `${bottom ? 'bottom' : 'top'}-${right ? 'right' : 'left'}`);
        }
        this._place();
    }

    // Just above the topmost desktop window (the desktop icons'), or the
    // wallpaper. GNOME Shell reorders the window actors whenever the windows
    // restack, so this follows each time. The desktop icons' window starts
    // as an ordinary one and only then becomes a desktop window and lowers
    // itself, which isn't a restack when it's already the lowest: without
    // following the type, the Recycle Bin stayed under that window, which
    // covers the screen and took its clicks (the user found it couldn't be
    // right-clicked or moved, 2026-10-26).
    _watchWindow(window) {
        if (window && this._dropClient?.owns_window(window))
            this._adoptDropTarget(window);
        window?.connectObject('notify::window-type', () => this._queueRestack(), this.actor);
        this._queueRestack();
    }

    _isDesktop(window) {
        return window?.get_window_type() === Meta.WindowType.DESKTOP ||
            !!window?.customJS_ding?.keepAtBottom;
    }

    _queueRestack() {
        if (this._restackLater)
            return;
        this._restackLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._restackLater = 0;
            this._restack();
            return GLib.SOURCE_REMOVE;
        });
    }

    // The window group's own children, not global.get_window_actors(): the
    // desktop icons extension takes its window out of that list on Wayland
    // (gnomeShellOverride.js), so 0.3.3 and 0.3.4 never found it and stayed
    // under it
    _restack() {
        if (!this._held)
            global.window_group.set_child_above_sibling(this.actor, this._below());
    }

    // What it goes right above: the topmost desktop window, else the
    // wallpaper
    _below() {
        const desktops = global.window_group.get_children().filter(actor =>
            actor instanceof Meta.WindowActor && this._isDesktop(actor.meta_window));
        return desktops.at(-1) ?? Main.layoutManager._backgroundGroup;
    }

    _stackedRight() {
        if (this._held)
            return true;
        const children = global.window_group.get_children();
        return children.indexOf(this.actor) === children.indexOf(this._below()) + 1;
    }

    _buildMenu() {
        this._menu = new PopupMenu.PopupMenu(this.actor, 0.5, St.Side.BOTTOM);
        this._menu.actor.add_style_class_name('patrone-menu');
        Main.uiGroup.add_child(this._menu.actor);
        this._menu.actor.hide();
        this._menuManager = new PopupMenu.PopupMenuManager(this.actor);
        this._menuManager.addMenu(this._menu);

        this._menu.addAction('Open', () => this.open(), 'folder-open-symbolic');
        this._emptyItem = this._menu.addAction(`Empty ${NAME}`, () => this._confirmEmpty(),
            'user-trash-symbolic');
        this._menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem('Keep docked to'));
        this._dockItems = DOCKS.map(([value, label]) => {
            const item = this._menu.addAction(label, () =>
                this._settings.set_string('recycle-bin-dock', value));
            item.dockValue = value;
            return item;
        });
        // Back through Patrone Settings (its Desktop page), the
        // show-recycle-bin setting, or this item while it's on the desktop
        // bubble, where it shows anyway
        this._menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._desktopItem = this._menu.addAction('Remove from desktop', () =>
            this._settings.set_boolean('show-recycle-bin',
                !this._settings.get_boolean('show-recycle-bin')), 'list-remove-symbolic');
    }

    _openMenu() {
        const dock = this._settings.get_string('recycle-bin-dock');
        for (const item of this._dockItems) {
            item.setOrnament(item.dockValue === dock
                ? PopupMenu.Ornament.DOT
                : PopupMenu.Ornament.NO_DOT ?? PopupMenu.Ornament.NONE);
        }
        const shown = this._settings.get_boolean('show-recycle-bin');
        this._desktopItem.label.text = shown ? 'Remove from desktop' : 'Show on desktop';
        this._desktopItem.setIcon(shown ? 'list-remove-symbolic' : 'list-add-symbolic');
        // Above it in the lower half of the screen, below it in the upper
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor)
            return;
        const lower = this.actor.y + this.actor.height / 2 > monitor.y + monitor.height / 2;
        this._menu._boxPointer.updateArrowSide(lower ? St.Side.BOTTOM : St.Side.TOP);
        this._menu.open(BoxPointer.PopupAnimation.FULL);
    }

    _confirmEmpty() {
        const dialog = new ModalDialog.ModalDialog({styleClass: 'patrone-dialog'});
        const count = this._items === 1 ? 'The item' : `All ${this._items} items`;
        dialog.contentLayout.add_child(new Dialog.MessageDialogContent({
            title: `Empty the ${NAME}?`,
            description: `${count} in it will be deleted for good. This can't be undone.`,
        }));
        dialog.setButtons([
            {label: 'Cancel', action: () => dialog.close(), key: Clutter.KEY_Escape},
            {
                label: `Empty ${NAME}`,
                action: () => {
                    dialog.close();
                    this._empty();
                },
            },
        ]);
        dialog.open();
    }

    _empty() {
        try {
            const process = Gio.Subprocess.new(['gio', 'trash', '--empty'], Gio.SubprocessFlags.NONE);
            process.wait_async(null, () => this._countItems());
        } catch (e) {
            Main.notifyError(`Can't empty the ${NAME}`, e.message);
        }
    }
}
