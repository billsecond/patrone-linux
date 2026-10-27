// The Recycle Bin on the desktop. Patrone draws it, in place of the desktop
// icons extension's trash, so that it can stay docked to a corner of the
// screen: bottom right unless moved (the user asked for it on 2026-10-26).
// Double-click opens it in the file manager. Right-click empties it or picks
// the corner. Dragged while docked, it docks to the nearest corner; not
// docked, it stays where it's dropped.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

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
        this._items = 0;
        this._restackLater = 0;

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

        this._buildMenu();

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
    }

    destroy() {
        this._endGrab();
        this._monitor?.cancel();
        if (this._restackLater)
            global.compositor.get_laters().remove(this._restackLater);
        this._settings.disconnectObject(this);
        global.display.disconnectObject(this);
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
        this.actor.visible = this._settings.get_boolean('show-recycle-bin');
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
        const desktops = global.window_group.get_children().filter(actor =>
            actor instanceof Meta.WindowActor && this._isDesktop(actor.meta_window));
        const below = desktops.at(-1) ?? Main.layoutManager._backgroundGroup;
        global.window_group.set_child_above_sibling(this.actor, below);
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
    }

    _openMenu() {
        const dock = this._settings.get_string('recycle-bin-dock');
        for (const item of this._dockItems) {
            item.setOrnament(item.dockValue === dock
                ? PopupMenu.Ornament.DOT
                : PopupMenu.Ornament.NO_DOT ?? PopupMenu.Ornament.NONE);
        }
        // Above it in the lower half of the screen, below it in the upper
        const monitor = Main.layoutManager.primaryMonitor;
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
