// The menu a taskbar button opens on right-click, as on Windows 11: the app's
// recent files (or, for the file manager, its pinned folders), its tasks
// (New window and the like), its open windows, then the app itself,
// "Pin to taskbar"/"Unpin from taskbar", the slot's default app, and "Close".

import St from 'gi://St';

import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {ROLES, candidatesForRole, setAppForRole} from './roles.js';
import {taskbarWindows, launchApp} from './util.js';

const RECENT_ITEMS = 8;

export class JumpList extends PopupMenu.PopupMenu {
    /**
     * @param {TaskbarItem} item - the taskbar button
     */
    constructor(item) {
        super(item, 0.5, St.Side.BOTTOM);
        this._item = item;

        this.actor.add_style_class_name('patrone-menu');
        Main.uiGroup.add_child(this.actor);
        this.actor.hide();

        this._manager = new PopupMenu.PopupMenuManager(item);
        this._manager.addMenu(this);
    }

    popup() {
        this._rebuild();
        // Above the button, or below it when the taskbar is at the top
        this._boxPointer.updateArrowSide(this._item.taskbar.menuSide);
        this.open(BoxPointer.PopupAnimation.FULL);
    }

    _launchWith(app, uri) {
        const context = global.create_app_launch_context(global.get_current_time(), -1);
        try {
            app.app_info.launch_uris([uri], context);
        } catch (e) {
            logError(e, `Patrone taskbar: can't open ${uri} with ${app.get_id()}`);
        }
        Main.overview.hide();
    }

    _addHeader(text) {
        this.addMenuItem(new PopupMenu.PopupSeparatorMenuItem(text));
    }

    _rebuild() {
        this.removeAll();

        const {app, role, taskbar} = this._item;
        const recent = taskbar.recent;

        if (app?.app_info) {
            if (role === 'files' || app.app_info.get_categories()?.split(';').includes('FileManager')) {
                const folders = recent.pinnedFolders();
                if (folders.length) {
                    this._addHeader('Pinned');
                    for (const folder of folders) {
                        this.addAction(folder.name, () => this._launchWith(app, folder.uri),
                            'folder-symbolic');
                    }
                }
            } else {
                const files = recent.recentForApp(app, RECENT_ITEMS, role !== 'web');
                if (files.length) {
                    this._addHeader('Recent');
                    for (const file of files)
                        this.addAction(file.name, () => this._launchWith(app, file.uri), file.gicon);
                }
            }

            const actions = app.app_info.list_actions();
            if (actions.length) {
                this._addHeader('Tasks');
                for (const action of actions) {
                    this.addAction(app.app_info.get_action_name(action), () => {
                        app.launch_action(action, global.get_current_time(), -1);
                        Main.overview.hide();
                    });
                }
            }
        }

        const windows = app ? taskbarWindows(app) : [];
        if (windows.length > 1) {
            this._addHeader('Open windows');
            for (const window of windows) {
                this.addAction(window.title || app.get_name(),
                    () => Main.activateWindow(window, global.get_current_time()));
            }
        }

        this.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        if (app) {
            this.addAction(app.get_name(), () => launchApp(app, true),
                app.app_info?.get_icon() ?? 'application-x-executable-symbolic');
        }

        if (role)
            this._addDefaultAppChoices(role, app);

        if (this._item.entry) {
            this.addAction('Unpin from taskbar', () => taskbar.unpin(this._item.entry),
                'view-pin-symbolic');
        } else if (app && !app.is_window_backed()) {
            this.addAction('Pin to taskbar', () => taskbar.pin(app.get_id()),
                'view-pin-symbolic');
        }

        if (windows.length) {
            this.addAction(windows.length > 1 ? 'Close all windows' : 'Close window', () => {
                for (const window of windows)
                    window.delete(global.get_current_time());
            }, 'window-close-symbolic');
        }
    }

    _addDefaultAppChoices(role, current) {
        const candidates = candidatesForRole(role);
        if (current && !candidates.includes(current))
            candidates.unshift(current);
        if (candidates.length === 0)
            return;

        const submenu = new PopupMenu.PopupSubMenuMenuItem(
            `Default ${ROLES[role].name.toLowerCase()}`, true);
        submenu.icon.icon_name = 'emblem-default-symbolic';
        for (const candidate of candidates) {
            const item = submenu.menu.addAction(candidate.get_name(), () => {
                setAppForRole(role, candidate, this._item.taskbar.settings);
                this._item.taskbar.queueRebuild();
            }, candidate.app_info?.get_icon() ?? undefined);
            if (candidate === current)
                item.setOrnament(PopupMenu.Ornament.CHECK);
        }
        this.addMenuItem(submenu);
    }
}
