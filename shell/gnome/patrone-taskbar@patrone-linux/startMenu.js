// The Start menu, Windows 11's in a smaller size. It opens with the search
// box focused, so typing searches right away: apps first, then everything
// GNOME's own search covers (Settings, Files, Calculator and the rest), with
// "Search the web" last. Enter opens the best match, or searches the web when
// nothing else matched.
//
// Empty, it shows the pinned apps (GNOME's favorites, which the Activities
// overview shares), recently used files with the suggested apps among them
// (Software at first), "All apps", and the account's avatar, Settings and
// power buttons. Opened from the taskbar's search button, it shows the most
// used apps and recent files instead.

import AccountsService from 'gi://AccountsService';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as AppFavorites from 'resource:///org/gnome/shell/ui/appFavorites.js';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as DND from 'resource:///org/gnome/shell/ui/dnd.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as ShellEntry from 'resource:///org/gnome/shell/ui/shellEntry.js';
import * as SystemActions from 'resource:///org/gnome/shell/misc/systemActions.js';
import * as UserWidget from 'resource:///org/gnome/shell/ui/userWidget.js';
import {ensureActorVisibleInScrollView} from 'resource:///org/gnome/shell/misc/animationUtils.js';

import {launchApp} from './util.js';
import {timeAgo} from './recent.js';

const PINNED_COLUMNS = 6;
const PINNED_ROWS = 3;
const RECOMMENDED = 6;
const TOP_APPS = 6;
const SEARCH_DELAY = 100; // ms after the last key before asking the slower search providers
const APP_RESULTS = 6;
const OTHER_RESULTS = 3;

function makeLabel(text, styleClass, params = {}) {
    const label = new St.Label({
        text,
        style_class: styleClass,
        y_align: Clutter.ActorAlign.CENTER,
        ...params,
    });
    label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    return label;
}

function makeHeader(text, button = null) {
    const box = new St.BoxLayout({style_class: 'patrone-start-header', x_expand: true});
    box.add_child(makeLabel(text, 'patrone-start-header-label', {x_expand: true}));
    if (button)
        box.add_child(button);
    return box;
}

function makeLinkButton(text, onClicked) {
    const button = new St.Button({
        style_class: 'patrone-start-link',
        label: text,
        can_focus: true,
        track_hover: true,
    });
    button.connect('clicked', onClicked);
    return button;
}

/**
 * A row with an icon, a name and an optional second line.
 *
 * @param {Clutter.Actor} icon
 * @param {string} name
 * @param {string} detail
 * @param {string} styleClass
 * @returns {St.Button}
 */
function makeRow(icon, name, detail, styleClass = 'patrone-start-row') {
    const box = new St.BoxLayout({style_class: 'patrone-start-row-box', x_expand: true});
    box.add_child(new St.Bin({child: icon, style_class: 'patrone-start-row-icon', y_align: Clutter.ActorAlign.CENTER}));
    const labels = new St.BoxLayout({
        orientation: Clutter.Orientation.VERTICAL,
        y_align: Clutter.ActorAlign.CENTER,
        x_expand: true,
    });
    // One line each: search providers' descriptions can hold line breaks
    const oneLine = text => text.replace(/\s+/g, ' ').trim();
    labels.add_child(makeLabel(oneLine(name), 'patrone-start-row-name'));
    if (detail)
        labels.add_child(makeLabel(oneLine(detail), 'patrone-start-row-detail'));
    box.add_child(labels);
    return new St.Button({
        style_class: styleClass,
        child: box,
        can_focus: true,
        track_hover: true,
        x_expand: true,
        accessible_name: name,
    });
}

function fallbackIcon(size) {
    return new St.Icon({icon_name: 'text-x-generic-symbolic', icon_size: size});
}

export class StartMenu {
    /**
     * @param {Taskbar} taskbar
     * @param {Clutter.Actor} sourceActor - the Start button
     */
    constructor(taskbar, sourceActor) {
        this._taskbar = taskbar;
        this._appSystem = Shell.AppSystem.get_default();
        this._favorites = AppFavorites.getAppFavorites();
        this._mode = 'start';
        this._cancellable = null;
        this._searchTimeoutId = 0;
        this._results = new Map();
        this._resultRows = [];
        this._selected = 0;
        this._userMovedSelection = false;
        this._allAppsDirty = true;
        this._pinnedTiles = [];
        this._dropAt = null;
        this._dragMonitor = {dragMotion: event => this._onDragMotion(event)};
        DND.addDragMonitor(this._dragMonitor);

        this.menu = new PopupMenu.PopupMenu(sourceActor, 0, St.Side.BOTTOM);
        this.menu.actor.add_style_class_name('patrone-menu');
        this.menu.actor.add_style_class_name('patrone-start-menu');
        Main.uiGroup.add_child(this.menu.actor);
        this.menu.actor.hide();
        this._manager = new PopupMenu.PopupMenuManager(sourceActor);
        this._manager.addMenu(this.menu);
        this.menu.connect('open-state-changed', (menu, open) => this._onOpenStateChanged(open));

        this.actor = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'patrone-start',
        });
        this.menu.box.add_child(this.actor);

        this._buildSearchBox();

        this._pages = new St.Widget({
            style_class: 'patrone-start-pages',
            layout_manager: new Clutter.BinLayout(),
            x_expand: true,
            y_expand: true,
        });
        this.actor.add_child(this._pages);

        this._homePage = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'patrone-start-page',
            x_expand: true,
            y_expand: true,
        });
        this._allAppsPage = this._buildAllAppsPage();
        this._resultsPage = this._buildResultsPage();
        for (const page of [this._homePage, this._allAppsPage, this._resultsPage])
            this._pages.add_child(page);
        this._showPage(this._homePage);

        this._buildFooter();

        this._appSystem.connectObject('installed-changed', () => {
            this._allAppsDirty = true;
        }, this.actor);
        // Pinning from a tile's own menu changes the favorites, so wait for
        // that menu to finish before replacing the tiles
        this._favorites.connectObject('changed', () => {
            GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                if (this.menu.isOpen && this._currentPage === this._homePage)
                    this._fillHome();
                return GLib.SOURCE_REMOVE;
            });
        }, this.actor);
    }

    destroy() {
        DND.removeDragMonitor(this._dragMonitor);
        this._cancelSearch();
        this._subMenu?.destroy();
        this.menu.destroy();
    }

    /**
     * Opens the menu, or closes it if it's open the same way.
     *
     * @param {string} mode - "start" (Start button, Super) or "search" (the
     *   search button)
     */
    toggle(mode) {
        if (this.menu.isOpen && this._mode === mode) {
            this.close();
            return;
        }
        this._mode = mode;
        if (this.menu.isOpen) {
            this._reset();
            this._entry.grab_key_focus();
        } else {
            this.menu.open(BoxPointer.PopupAnimation.FULL);
        }
    }

    close() {
        this.menu.close(BoxPointer.PopupAnimation.FULL);
    }

    _onOpenStateChanged(open) {
        if (open) {
            this._taskbar.tooltip.hide();
            this._reset();
            this._avatar.update();
            this._powerButton.visible = this._powerActions().length > 0;
            this._entry.grab_key_focus();
        } else {
            this._cancelSearch();
            this._subMenu?.close();
            this._entry.set_text('');
        }
    }

    _reset() {
        this._entry.set_text('');
        this._fillHome();
        this._showPage(this._homePage);
        this.actor.remove_style_class_name(this._mode === 'start' ? 'search-mode' : 'start-mode');
        this.actor.add_style_class_name(this._mode === 'start' ? 'start-mode' : 'search-mode');
    }

    _showPage(page) {
        for (const child of this._pages.get_children())
            child.visible = child === page;
        this._currentPage = page;
    }

    launch(app) {
        this.close();
        launchApp(app, true);
    }

    // The search box

    _buildSearchBox() {
        this._entry = new St.Entry({
            style_class: 'patrone-start-search',
            hint_text: 'Type here to search',
            can_focus: true,
            x_expand: true,
            primary_icon: new St.Icon({
                icon_name: 'system-search-symbolic',
                style_class: 'patrone-start-search-icon',
            }),
        });
        ShellEntry.addContextMenu(this._entry);
        this._entry.clutter_text.connect('text-changed', () => this._onTextChanged());
        this._entry.clutter_text.connect('key-press-event', (actor, event) => this._onKeyPress(event));
        this._entry.clutter_text.connect('activate', () => this._activateSelected());
        this.actor.add_child(this._entry);
    }

    _onKeyPress(event) {
        const symbol = event.get_key_symbol();
        if (symbol === Clutter.KEY_Super_L || symbol === Clutter.KEY_Super_R) {
            this.close();
            return Clutter.EVENT_STOP;
        }

        if (this._currentPage === this._resultsPage) {
            if (symbol === Clutter.KEY_Down) {
                this._select(this._selected + 1, true);
                return Clutter.EVENT_STOP;
            }
            if (symbol === Clutter.KEY_Up) {
                this._select(this._selected - 1, true);
                return Clutter.EVENT_STOP;
            }
        } else if (symbol === Clutter.KEY_Down) {
            this._currentPage.navigate_focus(null, St.DirectionType.TAB_FORWARD, false);
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    // Home: pinned apps and recommended files, or (from the search button)
    // top apps and recent files

    _fillHome() {
        this._homePage.destroy_all_children();
        if (this._mode === 'search')
            this._fillSearchHome();
        else
            this._fillStartHome();
    }

    _fillStartHome() {
        const allApps = makeLinkButton('All apps  ›', () => this._showAllApps());
        this._homePage.add_child(makeHeader('Pinned', allApps));

        const pinned = this._favorites.getFavorites().slice(0, PINNED_COLUMNS * PINNED_ROWS);
        const grid = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'patrone-start-grid',
        });
        // Pinned apps are dragged to reorder them
        this._pinnedTiles = [];
        grid._delegate = {
            handleDragOver: (source, actor, x, y) => this._handleDragOver(grid, source, x, y),
            acceptDrop: source => this._acceptDrop(source),
        };
        for (let i = 0; i < pinned.length; i += PINNED_COLUMNS) {
            const line = new St.BoxLayout({style_class: 'patrone-start-grid-row'});
            for (const app of pinned.slice(i, i + PINNED_COLUMNS)) {
                const tile = this._makeTile(app);
                this._pinnedTiles.push(tile);
                line.add_child(tile);
            }
            grid.add_child(line);
        }
        if (pinned.length === 0)
            grid.add_child(makeLabel('Right-click an app in All apps to pin it here.', 'patrone-start-empty'));
        this._homePage.add_child(grid);

        this._homePage.add_child(makeHeader('Recommended'));
        const items = this._recommended(RECOMMENDED);
        if (items.length === 0) {
            this._homePage.add_child(makeLabel('Files you open show up here.', 'patrone-start-empty'));
            return;
        }
        const columns = new St.BoxLayout({style_class: 'patrone-start-recommended', x_expand: true});
        const half = Math.ceil(items.length / 2);
        for (const part of [items.slice(0, half), items.slice(half)]) {
            const column = new St.BoxLayout({
                orientation: Clutter.Orientation.VERTICAL,
                x_expand: true,
            });
            for (const item of part)
                column.add_child(this._makeRecommendedRow(item));
            columns.add_child(column);
        }
        this._homePage.add_child(columns);
    }

    // Recent files and, among them, the suggested apps (Software at first),
    // each as if opened when it was first suggested: it starts at the top
    // and moves down as files are opened, then drops off like any recent
    // file (the user, 2026-10-27: "letting it move over time so people get
    // familiar with it")
    _recommended(limit) {
        return [...this._suggestedApps(), ...this._taskbar.recent.recent(limit)]
            .sort((a, b) => b.time - a.time)
            .slice(0, limit);
    }

    _suggestedApps() {
        const settings = this._taskbar.settings;
        const since = settings.get_value('suggested-apps-since').deep_unpack();
        const now = Math.floor(Date.now() / 1000);
        let first = false;
        const items = [];
        for (const id of settings.get_strv('suggested-apps')) {
            const app = this._appSystem.lookup_app(id);
            if (!app?.app_info?.should_show())
                continue;
            if (!(id in since)) {
                since[id] = now;
                first = true;
            }
            items.push({app, name: app.get_name(), time: Number(since[id])});
        }
        if (first)
            settings.set_value('suggested-apps-since', new GLib.Variant('a{sx}', since));
        return items;
    }

    _makeRecommendedRow(item) {
        if (!item.app)
            return this._makeFileRow(item);
        const {app} = item;
        // A short line, like the files' "Just now": the app's description
        // made its column twice as wide
        const row = makeRow(app.create_icon_texture(24), app.get_name(), 'Suggested app');
        row.connect('clicked', () => this.launch(app));
        this._addAppMenu(row, app);
        this._makeDraggable(row, app, 32);
        return row;
    }

    _fillSearchHome() {
        this._homePage.add_child(makeHeader('Top apps'));
        const usage = Shell.AppUsage.get_default();
        const top = usage.get_most_used().filter(app => app.app_info?.should_show());
        for (const app of this._favorites.getFavorites()) {
            if (top.length >= TOP_APPS)
                break;
            if (!top.includes(app))
                top.push(app);
        }
        const line = new St.BoxLayout({style_class: 'patrone-start-grid-row'});
        for (const app of top.slice(0, TOP_APPS))
            line.add_child(this._makeTile(app));
        this._homePage.add_child(line);

        this._homePage.add_child(makeHeader('Recent'));
        const items = this._recommended(RECOMMENDED);
        if (items.length === 0)
            this._homePage.add_child(makeLabel('Files you open show up here.', 'patrone-start-empty'));
        for (const item of items)
            this._homePage.add_child(this._makeRecommendedRow(item));
    }

    _makeTile(app) {
        const box = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
        });
        box.add_child(new St.Bin({
            child: app.create_icon_texture(32),
            x_align: Clutter.ActorAlign.CENTER,
        }));
        box.add_child(makeLabel(app.get_name(), 'patrone-start-tile-label', {
            x_align: Clutter.ActorAlign.CENTER,
        }));
        const tile = new St.Button({
            style_class: 'patrone-start-tile',
            child: box,
            can_focus: true,
            track_hover: true,
            accessible_name: app.get_name(),
        });
        tile.connect('clicked', () => this.launch(app));
        this._addAppMenu(tile, app);
        this._makeDraggable(tile, app, 32);
        return tile;
    }

    /**
     * Lets an app's tile or row be dragged: within Pinned to reorder it, or
     * onto the taskbar to pin it there.
     *
     * @param {St.Widget} actor
     * @param {Shell.App} app
     * @param {number} iconSize - of the icon that follows the pointer
     */
    _makeDraggable(actor, app, iconSize) {
        actor._delegate = {
            app,
            getDragActor: () => app.create_icon_texture(iconSize),
            getDragActorSource: () => actor,
        };
        const draggable = DND.makeDraggable(actor, {timeoutThreshold: 200});
        draggable.connect('drag-begin', () => {
            actor.opacity = 96;
        });
        draggable.connect('drag-end', () => {
            actor.opacity = 255;
            this._clearDropMarks();
        });
    }

    // Dropping in Pinned: the tile a drop goes before (or after, at the end
    // of a row) shows a blue line on that side

    _dropIndex(grid, x, y) {
        const [gridX, gridY] = grid.get_transformed_position();
        const stageX = gridX + x;
        const stageY = gridY + y;
        let index = this._pinnedTiles.length;
        for (let i = 0; i < this._pinnedTiles.length; i++) {
            const [tileX, tileY] = this._pinnedTiles[i].get_transformed_position();
            const [width, height] = this._pinnedTiles[i].get_transformed_size();
            if (stageY < tileY)
                return i; // above this row: before its first tile
            if (stageY <= tileY + height) {
                if (stageX < tileX + width / 2)
                    return i;
                index = i + 1;
            }
        }
        return index;
    }

    _clearDropMarks() {
        for (const tile of this._pinnedTiles ?? []) {
            tile.remove_style_pseudo_class('drop-before');
            tile.remove_style_pseudo_class('drop-after');
        }
        this._dropAt = null;
    }

    _handleDragOver(grid, source, x, y) {
        if (!source?.app || source.app.is_window_backed())
            return DND.DragMotionResult.NO_DROP;
        const index = this._dropIndex(grid, x, y);
        if (index !== this._dropAt) {
            this._clearDropMarks();
            if (index < this._pinnedTiles.length)
                this._pinnedTiles[index].add_style_pseudo_class('drop-before');
            else if (this._pinnedTiles.length)
                this._pinnedTiles.at(-1).add_style_pseudo_class('drop-after');
            this._dropAt = index;
        }
        return DND.DragMotionResult.MOVE_DROP;
    }

    _acceptDrop(source) {
        const id = source?.app?.get_id();
        if (!id || this._dropAt === null)
            return false;
        let position = this._dropAt;
        const ids = this._favorites.getFavorites().map(app => app.get_id());
        const from = ids.indexOf(id);
        // Moving it later in the list: its own place goes first
        if (from >= 0 && from < position)
            position--;
        this._clearDropMarks();
        if (from >= 0)
            this._favorites.moveFavoriteToPos(id, position);
        else
            this._favorites._addFavorite(id, position);
        return true;
    }

    _onDragMotion(event) {
        if (this._dropAt !== null && !this._homePage.contains(event.targetActor))
            this._clearDropMarks();
        return DND.DragMotionResult.CONTINUE;
    }

    _makeFileRow(file) {
        const row = makeRow(new St.Icon({gicon: file.gicon, icon_size: 24}),
            file.name, timeAgo(file.time));
        row.connect('clicked', () => {
            this.close();
            const context = global.create_app_launch_context(global.get_current_time(), -1);
            Gio.AppInfo.launch_default_for_uri_async(file.uri, context, null, (o, result) => {
                try {
                    Gio.AppInfo.launch_default_for_uri_finish(result);
                } catch (e) {
                    Main.notifyError(`Can't open “${file.name}”`, e.message);
                }
            });
        });
        return row;
    }

    // All apps

    _buildAllAppsPage() {
        const page = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            style_class: 'patrone-start-page',
            x_expand: true,
            y_expand: true,
        });
        const back = makeLinkButton('‹  Back', () => {
            this._fillHome();
            this._showPage(this._homePage);
        });
        page.add_child(makeHeader('All apps', back));

        this._allAppsList = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
        });
        this._allAppsScroll = new St.ScrollView({
            style_class: 'patrone-start-scroll',
            child: this._allAppsList,
            hscrollbar_policy: St.PolicyType.NEVER,
            vscrollbar_policy: St.PolicyType.AUTOMATIC,
            overlay_scrollbars: true,
            x_expand: true,
            y_expand: true,
        });
        page.add_child(this._allAppsScroll);
        return page;
    }

    _showAllApps() {
        if (this._allAppsDirty) {
            this._allAppsList.destroy_all_children();
            const apps = this._appSystem.get_installed()
                .filter(info => info.should_show())
                .map(info => this._appSystem.lookup_app(info.get_id()))
                .filter(Boolean)
                .sort((a, b) => a.get_name().localeCompare(b.get_name()));

            let letter = null;
            for (const app of apps) {
                const first = app.get_name().charAt(0).toUpperCase();
                const group = /\p{L}/u.test(first) ? first : '#';
                if (group !== letter) {
                    letter = group;
                    this._allAppsList.add_child(makeLabel(group, 'patrone-start-letter'));
                }
                const row = makeRow(app.create_icon_texture(24), app.get_name(), '');
                row.connect('clicked', () => this.launch(app));
                this._addAppMenu(row, app);
                this._makeDraggable(row, app, 32);
                this._allAppsList.add_child(row);
            }
            this._allAppsDirty = false;
        }
        this._allAppsScroll.vadjustment.value = 0;
        this._showPage(this._allAppsPage);
    }

    // Right-click on an app: pin it to Start or the taskbar

    _addAppMenu(actor, app) {
        const gesture = new Clutter.ClickGesture({
            required_button: Clutter.BUTTON_SECONDARY,
            recognize_on_press: true,
        });
        gesture.connect('recognize', () => this._popupAppMenu(actor, app));
        actor.add_action(gesture);
    }

    /**
     * A menu over the Start menu (an app's right-click menu, the account
     * menu, the power menu), one at a time, gone once it closes.
     *
     * @param {Clutter.Actor} sourceActor - what it opens from
     * @param {number} alignment - along the source: 0 left, 0.5 centered
     * @param {St.Side} side - St.Side.BOTTOM opens it above the source
     * @returns {PopupMenu.PopupMenu}
     */
    _makeSubMenu(sourceActor, alignment, side) {
        this._subMenu?.destroy();

        const menu = new PopupMenu.PopupMenu(sourceActor, alignment, side);
        menu.actor.add_style_class_name('patrone-menu');
        Main.uiGroup.add_child(menu.actor);
        menu.actor.hide();
        const manager = new PopupMenu.PopupMenuManager(sourceActor);
        manager.addMenu(menu);

        menu.connect('menu-closed', () => {
            if (this._subMenu === menu)
                this._subMenu = null;
            menu.destroy();
        });
        this._subMenu = menu;
        return menu;
    }

    /**
     * Closes the Start menu, then does something that may take over the
     * screen (lock, sign out), once the menus have let go of it.
     *
     * @param {Function} action
     */
    _closeThen(action) {
        this.close();
        GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            action();
            return GLib.SOURCE_REMOVE;
        });
    }

    _popupAppMenu(actor, app) {
        const menu = this._makeSubMenu(actor, 0.5, St.Side.TOP);

        const id = app.get_id();
        menu.addAction('Open', () => this.launch(app), app.app_info?.get_icon() ?? undefined);
        menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        if (this._favorites.isFavorite(id)) {
            menu.addAction('Unpin from Start', () => this._favorites._removeFavorite(id),
                'view-pin-symbolic');
        } else {
            menu.addAction('Pin to Start', () => this._favorites._addFavorite(id, -1),
                'view-pin-symbolic');
        }
        if (this._taskbar.isPinned(id)) {
            menu.addAction('Unpin from taskbar', () => this._taskbar.unpin(id),
                'view-pin-symbolic');
        } else {
            menu.addAction('Pin to taskbar', () => this._taskbar.pin(id),
                'view-pin-symbolic');
        }

        menu.open(BoxPointer.PopupAnimation.FULL);
    }

    // Search

    _buildResultsPage() {
        this._resultsBox = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
        });
        this._resultsScroll = new St.ScrollView({
            style_class: 'patrone-start-scroll patrone-start-page',
            child: this._resultsBox,
            hscrollbar_policy: St.PolicyType.NEVER,
            vscrollbar_policy: St.PolicyType.AUTOMATIC,
            overlay_scrollbars: true,
            x_expand: true,
            y_expand: true,
        });
        return this._resultsScroll;
    }

    _providers() {
        const providers = Main.overview.searchController?._searchResults?._providers ?? [];
        // Ubuntu's web search provider: the menu has its own "Search the web"
        return providers.filter(provider =>
            provider.constructor?.name !== 'WebSearcherSearchProvider' &&
            typeof provider.getInitialResultSet === 'function');
    }

    _cancelSearch() {
        if (this._searchTimeoutId) {
            GLib.source_remove(this._searchTimeoutId);
            this._searchTimeoutId = 0;
        }
        this._cancellable?.cancel();
        this._cancellable = null;
    }

    _onTextChanged() {
        const text = this._entry.get_text().trim();
        this._cancelSearch();
        this._results.clear();
        this._userMovedSelection = false;

        if (!text) {
            this._fillHome();
            this._showPage(this._homePage);
            return;
        }

        this._query = text;
        this._terms = text.split(/\s+/);
        this._cancellable = new Gio.Cancellable();
        this._showPage(this._resultsPage);
        this._renderResults();

        // Apps right away, so Enter right after typing opens the app; the
        // rest once typing pauses.
        const providers = this._providers();
        const cancellable = this._cancellable;
        for (const provider of providers.filter(p => p.id === 'applications'))
            this._searchProvider(provider, cancellable);
        this._searchTimeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SEARCH_DELAY, () => {
            this._searchTimeoutId = 0;
            for (const provider of providers.filter(p => p.id !== 'applications'))
                this._searchProvider(provider, cancellable);
            return GLib.SOURCE_REMOVE;
        });
    }

    async _searchProvider(provider, cancellable) {
        const terms = this._terms;
        try {
            const ids = await provider.getInitialResultSet(terms, cancellable);
            if (cancellable.is_cancelled())
                return;
            const max = provider.id === 'applications' ? APP_RESULTS : OTHER_RESULTS;
            const shown = provider.filterResults(ids, max);
            const metas = shown.length ? await provider.getResultMetas(shown, cancellable) : [];
            if (cancellable.is_cancelled())
                return;
            this._results.set(provider, metas.filter(Boolean));
            this._renderResults();
        } catch (e) {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Patrone taskbar: search provider ${provider.id} failed: ${e.message}`);
        }
    }

    _groupName(provider) {
        if (provider.id === 'applications')
            return 'Apps';
        return provider.appInfo?.get_name() ?? '';
    }

    _activateResult(provider, meta) {
        this.close();
        if (provider.id === 'applications') {
            if (meta.id.endsWith('.desktop')) {
                const app = this._appSystem.lookup_app(meta.id);
                if (app)
                    launchApp(app, true);
            } else {
                SystemActions.getDefault().activateAction(meta.id);
            }
            return;
        }
        provider.activateResult(meta.id, this._terms);
        Main.overview.hide();
    }

    _searchWeb() {
        const template = this._taskbar.settings.get_string('web-search-url');
        const url = template.replace('%s', encodeURIComponent(this._query));
        this.close();
        const context = global.create_app_launch_context(global.get_current_time(), -1);
        Gio.AppInfo.launch_default_for_uri_async(url, context, null, (o, result) => {
            try {
                Gio.AppInfo.launch_default_for_uri_finish(result);
            } catch (e) {
                Main.notifyError("Can't search the web", e.message);
            }
        });
    }

    _renderResults() {
        this._resultsBox.destroy_all_children();
        this._resultRows = [];

        const addRow = (row, activate) => {
            row.connect('clicked', activate);
            row.connect('notify::hover', () => {
                if (row.hover)
                    this._select(this._resultRows.findIndex(r => r.row === row), false);
            });
            this._resultRows.push({row, activate});
            return row;
        };

        // Best match: the first result, apps first
        const groups = this._providers()
            .filter(provider => this._results.get(provider)?.length)
            .map(provider => ({provider, metas: [...this._results.get(provider)]}));
        if (groups.length) {
            const {provider} = groups[0];
            const meta = groups[0].metas.shift();
            this._resultsBox.add_child(makeHeader('Best match'));
            const kind = provider.id === 'applications' ? 'App' : this._groupName(provider);
            this._resultsBox.add_child(addRow(
                makeRow(meta.createIcon(32) ?? fallbackIcon(32), meta.name, kind,
                    'patrone-start-row patrone-start-best'),
                () => this._activateResult(provider, meta)));
        }

        for (const {provider, metas} of groups) {
            if (metas.length === 0)
                continue;
            this._resultsBox.add_child(makeHeader(this._groupName(provider)));
            for (const meta of metas) {
                this._resultsBox.add_child(addRow(
                    makeRow(meta.createIcon(24) ?? fallbackIcon(24), meta.name, meta.description ?? ''),
                    () => this._activateResult(provider, meta)));
            }
        }

        this._resultsBox.add_child(makeHeader('Search the web'));
        this._resultsBox.add_child(addRow(
            makeRow(new St.Icon({icon_name: 'web-browser-symbolic', icon_size: 24}),
                this._query, 'See web results'),
            () => this._searchWeb()));

        if (!this._userMovedSelection)
            this._selected = 0;
        this._select(this._selected, false);
    }

    _select(index, byKeyboard) {
        if (this._resultRows.length === 0)
            return;
        index = Math.max(0, Math.min(index, this._resultRows.length - 1));
        this._resultRows[this._selected]?.row.remove_style_pseudo_class('selected');
        this._selected = index;
        const {row} = this._resultRows[index];
        row.add_style_pseudo_class('selected');
        if (byKeyboard) {
            this._userMovedSelection = true;
            ensureActorVisibleInScrollView(this._resultsScroll, row);
        }
    }

    _activateSelected() {
        if (this._currentPage !== this._resultsPage)
            return;
        this._resultRows[this._selected]?.activate();
    }

    // Footer, as on Windows 11: the account (its menu has Lock and Sign
    // out), Settings, and power (Disconnect, Sleep, Shut down, Restart)

    _buildFooter() {
        const footer = new St.BoxLayout({style_class: 'patrone-start-footer', x_expand: true});

        const userManager = AccountsService.UserManager.get_default();
        this._user = userManager.get_user(GLib.get_user_name());
        // Not the theme's .user-icon style, which is for the login screen
        this._avatar = new UserWidget.Avatar(this._user, {
            iconSize: 28,
            styleClass: 'patrone-start-avatar',
        });
        const name = makeLabel('', 'patrone-start-user-name');
        const updateUser = () => {
            name.text = this._realName();
            this._avatar.update();
        };
        this._user.connectObject(
            'notify::is-loaded', updateUser,
            'changed', updateUser, this.actor);
        updateUser();
        const userBox = new St.BoxLayout({style_class: 'patrone-start-user-box'});
        userBox.add_child(this._avatar);
        userBox.add_child(name);
        const userButton = new St.Button({
            style_class: 'patrone-start-user',
            child: userBox,
            can_focus: true,
            track_hover: true,
            accessible_name: 'Account',
        });
        userButton.connect('clicked', () => this._popupAccountMenu(userButton));
        footer.add_child(userButton);
        footer.add_child(new St.Widget({x_expand: true}));

        const settings = new St.Button({
            style_class: 'patrone-start-footer-button',
            child: new St.Icon({icon_name: 'emblem-system-symbolic', icon_size: 16}),
            can_focus: true,
            track_hover: true,
            accessible_name: 'Settings',
        });
        settings.connect('clicked', () => {
            const app = this._appSystem.lookup_app('org.gnome.Settings.desktop');
            if (app)
                this.launch(app);
        });
        footer.add_child(settings);

        this._powerButton = new St.Button({
            style_class: 'patrone-start-footer-button',
            child: new St.Icon({icon_name: 'system-shutdown-symbolic', icon_size: 16}),
            can_focus: true,
            track_hover: true,
            accessible_name: 'Power',
        });
        this._powerButton.connect('clicked', () => this._popupPowerMenu(this._powerButton));
        footer.add_child(this._powerButton);

        this.actor.add_child(footer);
    }

    _realName() {
        if (this._user.is_loaded && this._user.get_real_name())
            return this._user.get_real_name();
        return GLib.get_real_name() || GLib.get_user_name();
    }

    /** The account's menu: who's signed in, account settings, Lock, Sign out. */
    _popupAccountMenu(sourceActor) {
        const systemActions = SystemActions.getDefault();
        const menu = this._makeSubMenu(sourceActor, 0, St.Side.BOTTOM);

        // Not a menu item: those are greyed out when they can't be clicked
        const header = new St.BoxLayout({style_class: 'patrone-account-header'});
        header.add_child(new UserWidget.Avatar(this._user, {
            iconSize: 40,
            styleClass: 'patrone-account-avatar',
        }));
        const labels = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            y_align: Clutter.ActorAlign.CENTER,
        });
        labels.add_child(makeLabel(this._realName(), 'patrone-account-name'));
        const detail = makeLabel(GLib.get_user_name(), 'patrone-account-detail');
        detail.opacity = 170;
        labels.add_child(detail);
        header.add_child(labels);
        header.get_first_child().update();
        menu.box.add_child(header);

        menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        menu.addAction('Change account settings', () => this._closeThen(() => {
            const context = global.create_app_launch_context(global.get_current_time(), -1);
            try {
                Gio.DesktopAppInfo.new('gnome-users-panel.desktop')?.launch([], context);
            } catch (e) {
                logError(e, 'Patrone taskbar: can\'t open account settings');
            }
        }), 'system-users-symbolic');

        const sessionItems = [
            ['Lock', 'system-lock-screen-symbolic', systemActions.canLockScreen,
                () => systemActions.activateLockScreen()],
            ['Sign out', 'system-log-out-symbolic', systemActions.canLogout,
                () => systemActions.activateLogout()],
        ].filter(([, , available]) => available);
        if (sessionItems.length)
            menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        for (const [text, iconName, , activate] of sessionItems)
            menu.addAction(text, () => this._closeThen(activate), iconName);

        menu.open(BoxPointer.PopupAnimation.FULL);
    }

    /**
     * What the power menu offers, as on Windows: Disconnect when the desktop
     * is being seen through Remote Desktop (which leaves it running for next
     * time), then Sleep, Shut down and Restart, where the system allows them.
     *
     * @returns {Array} [text, icon name, action] for each
     */
    _powerActions() {
        const systemActions = SystemActions.getDefault();
        const actions = [];

        // Patrone's desktops have no screen of their own: every screen and
        // client sees them through GNOME Remote Desktop, whose connections
        // GNOME counts as screen sharing. Stopping that is a disconnect.
        const sharing = Main.panel.statusArea.screenSharing;
        if (global.backend.is_headless() && sharing?._handles?.size) {
            actions.push(['Disconnect', 'network-wired-disconnected-symbolic',
                () => sharing._stopSharing()]);
        }
        if (systemActions.canSuspend) {
            actions.push(['Sleep', 'weather-clear-night-symbolic',
                () => systemActions.activateSuspend()]);
        }
        if (systemActions.canPowerOff) {
            actions.push(['Shut down', 'system-shutdown-symbolic',
                () => systemActions.activatePowerOff()]);
        }
        if (systemActions.canRestart) {
            actions.push(['Restart', 'system-reboot-symbolic',
                () => systemActions.activateRestart()]);
        }
        return actions;
    }

    _popupPowerMenu(sourceActor) {
        const actions = this._powerActions();
        if (actions.length === 0)
            return;
        const menu = this._makeSubMenu(sourceActor, 1, St.Side.BOTTOM);
        for (const [text, iconName, activate] of actions)
            menu.addAction(text, () => this._closeThen(activate), iconName);
        menu.open(BoxPointer.PopupAnimation.FULL);
    }
}
