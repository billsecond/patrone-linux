// The desktop bubble (the user's design, 2026-10-26). With the
// desktop-bubble setting on, the desktop's files show only in a bubble that
// comes out of the left edge of the primary screen, not across the desktop.
// A small pill at that edge, shown while no window covers its spot, opens
// it when clicked or dragged out. The bubble is half an ellipse, at most
// half the screen wide and smaller with fewer files (bubbleShape.js); it
// opens over any window, like a menu, and the Recycle Bin rides on its lower
// right edge while it's open. Until the pill is first opened, it bounces now
// and then with a tip saying what it does.
//
// The desktop icons extension keeps running, but its window is kept out of
// sight while the setting is on: it neither draws nor takes clicks, so a
// right-click on the empty desktop gets GNOME's own menu (Change
// Background, Display Settings).

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {CELL_HEIGHT, CELL_WIDTH, bubbleShape} from './bubbleShape.js';

const PILL_WIDTH = 12;
const PILL_HEIGHT = 72;
// A window this close to the pill counts as covering it
const PILL_MARGIN = 6;
const ICON_SIZE = 48;
// The bubble's soft shadow, around its edge
const SHADOW = 12;

const OPEN_TIME = 320;
const CLOSE_TIME = 200;
// Until it's first opened, the pill bounces this often (seconds), its tip
// showing for a moment (milliseconds)
const BOUNCE_EVERY = 6;
const TIP_TIME = 3500;
const HOP = 10;
const TIP_FIRST = 'Your desktop files are in here: click, or drag it out';

const ATTRIBUTES = [
    'standard::name', 'standard::display-name', 'standard::icon', 'standard::type',
    'standard::is-hidden', 'standard::is-backup', 'standard::content-type',
    'thumbnail::path', 'thumbnail::is-valid', 'metadata::trusted', 'access::can-execute',
].join(',');

export class DesktopBubble {
    /**
     * @param {Gio.Settings} settings - the taskbar's settings
     * @param {object} recycleBin - the taskbar's Recycle Bin (recycleBin.js)
     */
    constructor(settings, recycleBin) {
        this._settings = settings;
        this._bin = recycleBin;
        this._items = [];
        this._open = false;
        this._covered = false;
        this._coverLater = 0;
        this._desktopsLater = 0;
        this._bounceId = 0;
        this._firstBounceId = 0;
        this._tipId = 0;
        this._readId = 0;

        // Above the windows, below the taskbar and every menu
        this._layer = new St.Widget({reactive: false, visible: false});
        this._layer.add_constraint(new Clutter.BindConstraint({
            source: global.stage,
            coordinate: Clutter.BindCoordinate.SIZE,
        }));
        Main.uiGroup.add_child(this._layer);
        Main.uiGroup.set_child_above_sibling(this._layer, global.window_group);
        this._layer.connect('button-press-event', (_, event) => this._pressed(event));
        this._layer.connect('touch-event', (_, event) => this._pressed(event));
        this._layer.connect('key-press-event', (_, event) => this._keyPressed(event));

        this._bubble = new St.Widget({
            visible: false,
            pivot_point: new Graphene.Point({x: 0, y: 0.5}),
        });
        this._bubble.connect('notify::scale-x', () => this._follow());
        this._shape = new St.DrawingArea({style_class: 'patrone-desktop-bubble', reactive: true});
        this._shape.connect('repaint', () => this._paint());
        this._bubble.add_child(this._shape);
        this._cells = new St.Widget();
        this._bubble.add_child(this._cells);
        this._layer.add_child(this._bubble);

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

        // Where a right-click on the bubble's empty part opens its menu
        this._anchor = new St.Widget({width: 1, height: 1});
        this._layer.add_child(this._anchor);
        this._menuManager = new PopupMenu.PopupMenuManager(this._layer);

        // Where the desktop icons' windows wait, unseen, while the setting is
        // on (_hideDesktopIcons)
        this._parking = new Clutter.Actor({visible: false});
        Main.uiGroup.add_child(this._parking);

        this._settings.connectObject(
            'changed::desktop-bubble', () => this._sync(),
            'changed::show-recycle-bin', () => this.close(false),
            'changed::desktop-bubble-introduced', () => this._introduce(), this);
        global.display.connectObject(
            'workareas-changed', () => this._sync(),
            'restacked', () => this._queueChecks(),
            'window-created', (_, window) => this._watchWindow(window),
            'notify::focus-window', () => this._keepFocusOffDesktop(), this);
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

        this._watchFolder();
        this._sync();
        this._introduce();
    }

    destroy() {
        this.close(false);
        this._stopIntroduction();
        for (const id of [this._tipId, this._readId]) {
            if (id)
                GLib.source_remove(id);
        }
        for (const later of [this._coverLater, this._desktopsLater]) {
            if (later)
                global.compositor.get_laters().remove(later);
        }
        this._readCancellable?.cancel();
        this._folderMonitor?.cancel();
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

    // Shows or hides it all for the setting, the overview and the screen
    _sync() {
        const on = this._on;
        this._hideDesktopIcons(on);
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
        if (!this._bubble.visible)
            return;
        if (animate) {
            this._settle(false);
            return;
        }
        this._bubble.remove_all_transitions();
        this._bubble.set_scale(0, 0);
        this._closed();
    }

    // Lays the bubble out for the files, shown at nothing, ready to grow
    _prepare() {
        if (this._bubble.visible)
            return true;
        if (!this._layer.visible || !this._layout())
            return false;
        this._bubble.set_scale(0, 0);
        this._bubble.show();
        this._pill.add_style_class_name('open');
        this._pillIcon.icon_name = 'pan-start-symbolic';
        this._hideTip();
        this._holdBin();
        if (!this._settings.get_boolean('desktop-bubble-introduced'))
            this._settings.set_boolean('desktop-bubble-introduced', true);
        return true;
    }

    // Grows it to full size, or shrinks it away
    _settle(open) {
        this._open = open;
        if (open)
            this._takeModal();
        else
            this._dropModal();
        this._bubble.remove_all_transitions();
        this._bubble.ease({
            scale_x: open ? 1 : 0,
            scale_y: open ? 1 : 0,
            duration: open ? OPEN_TIME : CLOSE_TIME,
            mode: open ? Clutter.AnimationMode.EASE_OUT_BACK : Clutter.AnimationMode.EASE_IN_QUAD,
            onComplete: () => {
                if (!open)
                    this._closed();
            },
        });
    }

    _closed() {
        this._open = false;
        this._dropModal();
        this._bubble.hide();
        this._cells.destroy_all_children();
        this._pill.remove_style_class_name('open');
        this._pillIcon.icon_name = 'pan-end-symbolic';
        this._releaseBin();
        this._follow();
        this._queueChecks();
    }

    // Like a menu: the keyboard comes here, and a click outside closes it
    _takeModal() {
        this._layer.reactive = true;
        if (!this._grab)
            this._grab = Main.pushModal(this._layer, {actionMode: Shell.ActionMode.POPUP});
    }

    _dropModal() {
        this._layer.reactive = false;
        if (this._grab)
            Main.popModal(this._grab);
        this._grab = null;
    }

    // Dragging the pill pulls the bubble out (or pushes it back) with the
    // pointer; let go, it opens if it was pulled out far enough
    _dragStarted() {
        if (!this._prepare()) {
            this._dragFrom = null;
            return;
        }
        this._bubble.remove_all_transitions();
        this._dragFrom = this._bubble.scale_x;
        this._dragGrab = global.stage.grab(this._pill);
    }

    _dragged() {
        if (this._dragFrom === null || this._dragFrom === undefined)
            return;
        const delta = this._pan.get_accumulated_delta_abs().get_x();
        const scale = Math.min(Math.max(this._dragFrom + delta / this._a, 0), 1.08);
        this._bubble.set_scale(scale, scale);
    }

    _dragEnded(cancelled) {
        this._dragGrab?.dismiss();
        this._dragGrab = null;
        if (this._dragFrom === null || this._dragFrom === undefined)
            return;
        const wasOpen = this._dragFrom >= 0.5;
        const reached = this._bubble.scale_x;
        this._dragFrom = null;
        this._settle(cancelled ? wasOpen : reached > (wasOpen ? 0.75 : 0.25));
    }

    // The bubble's size and its files' places, for the files there are
    _layout() {
        const area = this._area();
        if (!area)
            return false;
        const scale = this._scale;
        const showBin = this._bin?.hold && this._settings.get_boolean('show-recycle-bin');
        let bin = null;
        if (showBin) {
            const [, , width, height] = this._bin.actor.get_preferred_size();
            bin = [width, height];
        }
        const shape = bubbleShape({
            width: area.width,
            height: area.height,
            count: this._items.length,
            bin,
            scale,
        });
        this._a = shape.a;
        this._b = shape.b;
        this._binSpot = shape.bin;
        this._bubble.set_position(area.x, Math.round(area.y + area.height / 2 - shape.b));
        this._bubble.set_size(shape.a, 2 * shape.b);
        const shadow = SHADOW * scale;
        this._shape.set_position(0, -shadow);
        this._shape.set_size(shape.a + shadow, 2 * shape.b + 2 * shadow);
        this._shape.queue_repaint();

        this._cells.destroy_all_children();
        const [width, height] = [CELL_WIDTH * scale, CELL_HEIGHT * scale];
        const overflow = this._items.length > shape.cells.length;
        const shown = overflow ? shape.cells.length - 1 : this._items.length;
        this._items.slice(0, shown).forEach((item, i) => {
            const [x, y] = shape.cells[i];
            this._cells.add_child(this._itemButton(item, x, y, width, height));
        });
        if (overflow) {
            const [x, y] = shape.cells[shown];
            const more = this._items.length - shown;
            this._cells.add_child(this._itemButton({
                name: `${more} more`,
                icon: new Gio.ThemedIcon({name: 'folder-open'}),
                file: this._folder,
                folder: true,
            }, x, y, width, height));
        }
        if (this._items.length === 0) {
            const [x, y] = shape.cells[0] ?? [0, shape.b - height / 2];
            this._cells.add_child(new St.Label({
                text: this._folder ? 'Nothing on the desktop' : 'No Desktop folder',
                style_class: 'patrone-bubble-empty',
                x,
                y: Math.round(y + height / 2 - 10 * scale),
            }));
        }
        return true;
    }

    _itemButton(item, x, y, width, height) {
        const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        box.add_child(new St.Icon({
            gicon: item.icon,
            fallback_icon_name: 'text-x-generic',
            icon_size: ICON_SIZE,
            x_align: Clutter.ActorAlign.CENTER,
        }));
        const label = new St.Label({
            text: item.name,
            style_class: 'patrone-bubble-item-label',
            x_align: Clutter.ActorAlign.CENTER,
            y_expand: true,
        });
        label.clutter_text.set({
            line_wrap: true,
            line_wrap_mode: Pango.WrapMode.WORD_CHAR,
            ellipsize: Pango.EllipsizeMode.END,
            line_alignment: Pango.Alignment.CENTER,
        });
        box.add_child(label);
        const button = new St.Button({
            style_class: 'patrone-bubble-item',
            child: box,
            can_focus: true,
            accessible_name: item.name,
            x, y, width, height,
        });
        button.connect('clicked', () => this._openItem(item));
        const rightClick = new Clutter.ClickGesture({
            required_button: Clutter.BUTTON_SECONDARY,
            recognize_on_press: true,
        });
        rightClick.connect('recognize', () => this._itemMenu(button, item));
        button.add_action(rightClick);
        return button;
    }

    _paint() {
        const cr = this._shape.get_context();
        const [width, height] = this._shape.get_surface_size();
        const node = this._shape.get_theme_node();
        const shadow = SHADOW * this._scale;
        const [a, b, middle] = [width - shadow, height / 2 - shadow, height / 2];
        const half = (ra, rb, y = middle) => {
            cr.save();
            cr.translate(0, y);
            cr.scale(ra, rb);
            cr.arc(0, 0, 1, -Math.PI / 2, Math.PI / 2);
            cr.restore();
        };
        const source = (name, fallback) => {
            const [found, color] = node.lookup_color(name, false);
            if (found)
                cr.setSourceRGBA(color.red / 255, color.green / 255, color.blue / 255, color.alpha / 255);
            else
                cr.setSourceRGBA(...fallback);
        };

        // The shadow: rings fading out from the edge, a little lower
        for (let ring = shadow; ring > 0; ring -= 2) {
            half(a + ring - shadow / 2, b + ring - shadow / 2, middle + shadow / 6);
            cr.closePath();
            cr.setSourceRGBA(0, 0, 0, 0.02);
            cr.fill();
        }
        half(a - 1, b - 1);
        cr.closePath();
        source('-patrone-bubble-fill', [0.98, 0.98, 0.99, 0.96]);
        cr.fill();
        // The border along the curve only: the straight side is the screen's edge
        half(a - 1, b - 1);
        source('-patrone-bubble-border', [0, 0, 0, 0.12]);
        cr.setLineWidth(this._scale);
        cr.stroke();
        cr.$dispose();
    }

    // The pill, and the Recycle Bin while it's held, follow the bubble's edge
    _follow() {
        const area = this._area();
        if (!area)
            return;
        const scale = this._scale;
        const [pillWidth, pillHeight] = [PILL_WIDTH * scale, PILL_HEIGHT * scale];
        const middle = area.y + area.height / 2;
        const grown = this._bubble.visible ? this._bubble.scale_x : 0;
        const x = grown > 0 ? area.x + this._a * grown - pillWidth / 2 : area.x;
        this._pill.set_size(pillWidth, pillHeight);
        this._pill.set_position(Math.round(x), Math.round(middle - pillHeight / 2));

        if (this._binFrom && this._binSpot) {
            // From where it sat towards its spot on the edge, which moves out
            // as the bubble grows
            const [width, height] = this._bin.actor.get_size();
            const spotX = area.x + (this._binSpot[0] + width / 2) * grown - width / 2;
            const spotY = middle + (this._binSpot[1] + height / 2 - this._b) * grown - height / 2;
            const along = Math.min(Math.max(grown, 0), 1);
            this._bin.hold(this._layer,
                Math.round(this._binFrom[0] + (spotX - this._binFrom[0]) * along),
                Math.round(this._binFrom[1] + (spotY - this._binFrom[1]) * along));
        }
    }

    _holdBin() {
        if (!this._bin?.hold || !this._binSpot || !this._bin.actor.visible)
            return;
        this._binFrom = this._bin.actor.get_transformed_position();
        this._bin.hold(this._layer, ...this._binFrom.map(Math.round));
    }

    _releaseBin() {
        if (!this._binFrom)
            return;
        this._binFrom = null;
        this._bin.release();
    }

    _pressed(event) {
        const type = event.type();
        if (type === Clutter.EventType.TOUCH_BEGIN || type === Clutter.EventType.BUTTON_PRESS) {
            // (event.get_source() is always null in GNOME 50)
            const source = global.stage.get_event_actor(event);
            const [x, y] = event.get_coords();
            if (source === this._layer || (source === this._shape && !this._insideBubble(x, y))) {
                this.close();
                return Clutter.EVENT_STOP;
            }
            if (source === this._shape && event.get_button?.() === Clutter.BUTTON_SECONDARY) {
                this._bubbleMenu(x, y);
                return Clutter.EVENT_STOP;
            }
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _insideBubble(x, y) {
        const [bx, by] = this._bubble.get_transformed_position();
        const [width, height] = this._bubble.get_transformed_size();
        const [dx, dy] = [x - bx, y - by - height / 2];
        return dx >= 0 && (dx / width) ** 2 + (dy / (height / 2)) ** 2 <= 1;
    }

    _keyPressed(event) {
        const directions = {
            [Clutter.KEY_Up]: St.DirectionType.UP,
            [Clutter.KEY_Down]: St.DirectionType.DOWN,
            [Clutter.KEY_Left]: St.DirectionType.LEFT,
            [Clutter.KEY_Right]: St.DirectionType.RIGHT,
            [Clutter.KEY_Tab]: St.DirectionType.TAB_FORWARD,
            [Clutter.KEY_ISO_Left_Tab]: St.DirectionType.TAB_BACKWARD,
        };
        const key = event.get_key_symbol();
        if (key === Clutter.KEY_Escape) {
            this.close();
            return Clutter.EVENT_STOP;
        }
        if (key in directions) {
            this._cells.navigate_focus(global.stage.key_focus, directions[key], false);
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _menu(source, side) {
        const menu = new PopupMenu.PopupMenu(source, 0.5, side);
        menu.actor.add_style_class_name('patrone-menu');
        Main.uiGroup.add_child(menu.actor);
        menu.actor.hide();
        this._menuManager.addMenu(menu);
        // One menu for each right-click, gone once closed
        menu.connect('open-state-changed', (_, open) => {
            if (!open) {
                GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                    menu.destroy();
                    return GLib.SOURCE_REMOVE;
                });
            }
        });
        return menu;
    }

    _itemMenu(button, item) {
        const menu = this._menu(button, St.Side.TOP);
        menu.addAction('Open', () => this._openItem(item),
            item.folder ? 'folder-open-symbolic' : 'document-open-symbolic');
        if (item.info) {
            menu.addAction('Show in folder', () => this._showInFolder(item), 'folder-symbolic');
            menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
            menu.addAction('Move to Recycle Bin', () => this._trash(item), 'user-trash-symbolic');
        }
        menu.open(BoxPointer.PopupAnimation.FULL);
    }

    _bubbleMenu(x, y) {
        this._anchor.set_position(Math.round(x), Math.round(y));
        const menu = this._menu(this._anchor, St.Side.TOP);
        if (this._folder) {
            menu.addAction('Open Desktop folder', () => this._openItem({file: this._folder, name: 'Desktop'}),
                'folder-open-symbolic');
        }
        if (GLib.find_program_in_path('patrone-settings')) {
            menu.addAction('Desktop settings', () => {
                this.close();
                this._run(['patrone-settings', '--page', 'desktop']);
            }, 'emblem-system-symbolic');
        }
        if (!menu.isEmpty())
            menu.open(BoxPointer.PopupAnimation.FULL);
    }

    _openItem(item) {
        this.close();
        const context = global.create_app_launch_context(0, -1);
        try {
            if (item.app)
                item.app.launch([], context);
            else
                Gio.AppInfo.launch_default_for_uri(item.file.get_uri(), context);
        } catch (e) {
            Main.notifyError(`Can't open ${item.name}`, e.message);
        }
    }

    // The file manager opens the Desktop folder with the file selected
    _showInFolder(item) {
        this.close();
        Gio.DBus.session.call('org.freedesktop.FileManager1', '/org/freedesktop/FileManager1',
            'org.freedesktop.FileManager1', 'ShowItems',
            new GLib.Variant('(ass)', [[item.file.get_uri()], '']), null,
            Gio.DBusCallFlags.NONE, -1, null, (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch {
                    this._openItem({file: this._folder, name: 'Desktop'});
                }
            });
    }

    _trash(item) {
        item.file.trash_async(GLib.PRIORITY_DEFAULT, null, (file, result) => {
            try {
                file.trash_finish(result);
            } catch (e) {
                Main.notifyError(`Can't move ${item.name} to the Recycle Bin`, e.message);
            }
        });
    }

    _run(argv) {
        try {
            Gio.Subprocess.new(argv, Gio.SubprocessFlags.NONE);
        } catch (e) {
            Main.notifyError(`Can't start ${argv[0]}`, e.message);
        }
    }

    // The desktop's files: those in the Desktop folder, kept up to date
    _watchFolder() {
        const path = GLib.get_user_special_dir(GLib.UserDirectory.DIRECTORY_DESKTOP);
        // With no Desktop folder of its own, the desktop is the home folder,
        // which isn't for showing here
        if (!path || path === GLib.get_home_dir())
            return;
        this._folder = Gio.File.new_for_path(path);
        try {
            this._folderMonitor = this._folder.monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
            this._folderMonitor.connect('changed', () => this._queueRead());
        } catch (e) {
            console.warn(`Patrone taskbar: can't watch the Desktop folder: ${e.message}`);
        }
        this._read();
    }

    _queueRead() {
        if (this._readId)
            return;
        this._readId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 250, () => {
            this._readId = 0;
            this._read();
            return GLib.SOURCE_REMOVE;
        });
    }

    _read() {
        this._readCancellable?.cancel();
        const cancellable = new Gio.Cancellable();
        this._readCancellable = cancellable;
        const infos = [];
        this._folder.enumerate_children_async(ATTRIBUTES, Gio.FileQueryInfoFlags.NONE,
            GLib.PRIORITY_LOW, cancellable, (folder, result) => {
                let enumerator;
                try {
                    enumerator = folder.enumerate_children_finish(result);
                } catch (e) {
                    if (!cancellable.is_cancelled())
                        this._setItems([]);
                    return;
                }
                const next = () => enumerator.next_files_async(100, GLib.PRIORITY_LOW, cancellable,
                    (_, batchResult) => {
                        let batch;
                        try {
                            batch = enumerator.next_files_finish(batchResult);
                        } catch {
                            return;
                        }
                        if (batch.length > 0) {
                            infos.push(...batch);
                            next();
                            return;
                        }
                        enumerator.close_async(GLib.PRIORITY_LOW, null, null);
                        this._setItems(infos);
                    });
                next();
            });
    }

    _setItems(infos) {
        this._items = infos
            .filter(info => !info.get_is_hidden() && !info.get_is_backup())
            .map(info => this._item(info))
            // Folders first, then by name, as on Windows' desktop
            .sort((p, q) => (q.folder - p.folder) ||
                p.name.localeCompare(q.name, undefined, {sensitivity: 'base', numeric: true}));
        if (this._bubble.visible) {
            this._layout();
            this._follow();
        }
    }

    _item(info) {
        const file = this._folder.get_child(info.get_name());
        let name = info.get_display_name();
        let icon = info.get_icon();
        let app = null;
        // A launcher the account allowed to run (Allow Launching) starts its
        // app; any other opens as a file
        if (info.get_content_type() === 'application/x-desktop' &&
            info.get_attribute_string('metadata::trusted') === 'true' &&
            info.get_attribute_boolean('access::can-execute')) {
            app = Gio.DesktopAppInfo.new_from_filename(file.get_path());
            if (app) {
                name = app.get_name();
                icon = app.get_icon() ?? icon;
            }
        }
        const thumbnail = info.get_attribute_byte_string('thumbnail::path');
        const valid = !info.has_attribute('thumbnail::is-valid') ||
            info.get_attribute_boolean('thumbnail::is-valid');
        if (thumbnail && valid)
            icon = new Gio.FileIcon({file: Gio.File.new_for_path(thumbnail)});
        return {file, info, name, icon, app, folder: info.get_file_type() === Gio.FileType.DIRECTORY};
    }

    // The desktop icons extension's windows, which cover the screen, by that
    // extension's own mark: the Recycle Bin's drop target is a desktop
    // window too, and must stay
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
        if (this._desktopsLater)
            return;
        this._desktopsLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._desktopsLater = 0;
            this._hideDesktopIcons(this._on);
            return GLib.SOURCE_REMOVE;
        });
    }

    // The desktop icons' windows wait in a hidden group of their own, where
    // they neither draw nor take clicks. (Clipped to nothing, they still
    // drew.) Mutter keeps restacking them there, as it does windows GNOME
    // Shell's own effects move. They're the window group's own children,
    // since that extension takes its windows out of
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

    // Unseen, the desktop icons' window mustn't take the keyboard (it gets
    // it when the last window closes), or keys would act on hidden files
    _keepFocusOffDesktop() {
        const window = global.display.focus_window;
        if (this._on && this._isDesktopIcons(window))
            global.display.unset_input_focus(global.get_current_time());
    }

    _queueCoverCheck() {
        if (this._coverLater)
            return;
        this._coverLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._coverLater = 0;
            this._checkCover();
            return GLib.SOURCE_REMOVE;
        });
    }

    // The pill shows while no window covers its spot
    _checkCover() {
        const area = this._area();
        if (!area || this._bubble.visible)
            return;
        const scale = this._scale;
        const margin = PILL_MARGIN * scale;
        const top = area.y + area.height / 2 - PILL_HEIGHT * scale / 2 - margin;
        const bottom = top + PILL_HEIGHT * scale + 2 * margin;
        const right = area.x + (PILL_WIDTH + HOP) * scale + margin;
        const workspace = global.workspace_manager.get_active_workspace();
        const covered = global.display.list_all_windows().some(window => {
            if (this._isDesktopIcons(window) || window.get_window_type() === Meta.WindowType.DESKTOP ||
                window.minimized || !window.located_on_workspace(workspace) ||
                !window.showing_on_its_workspace() || !window.get_compositor_private()?.visible)
                return false;
            const frame = window.get_frame_rect();
            return frame.x < right && frame.x + frame.width > area.x &&
                frame.y < bottom && frame.y + frame.height > top;
        });
        const show = this._layer.visible && !covered;
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
        if (!this._layer.visible || !this._pill.visible || this._pill.opacity < 255 || this._bubble.visible)
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
        if (this._bubble.visible || !this._pill.visible)
            return;
        const count = this._items.length;
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
