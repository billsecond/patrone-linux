// Patrone Taskbar: Patrone Desktop's taskbar for its GNOME-based desktop,
// until Patrone's own shell replaces GNOME Shell. See shell/gnome/README.md.

import Gio from 'gi://Gio';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {DesktopBubble} from './desktopBubble.js';
import {DesktopHost} from './desktopHost.js';
import {Taskbar} from './taskbar.js';
import {startExtra} from './util.js';

// The taskbar's ID until 0.4.8, under the system's old name. The session
// mode loads the taskbar whatever an account's own lists say, but those
// lists may still name the old ID: it's replaced with today's, once, so a
// choice made under it (turning the taskbar off, say) still holds.
const OLD_UUID = 'patrone-taskbar@patrone-linux';

function renameInShellLists(uuid) {
    const shell = new Gio.Settings({schema_id: 'org.gnome.shell'});
    for (const key of ['enabled-extensions', 'disabled-extensions']) {
        const list = shell.get_strv(key);
        if (list.includes(OLD_UUID))
            shell.set_strv(key, [...new Set(list.map(id => (id === OLD_UUID ? uuid : id)))]);
    }
}

export default class PatroneTaskbarExtension extends Extension {
    enable() {
        try {
            renameInShellLists(this.uuid);
        } catch (e) {
            console.warn(`Patrone Taskbar: couldn't rename its old ID in the account's lists: ${e}`);
        }
        this._taskbar = new Taskbar(this);
        this._desktopHost = startExtra('desktop',
            () => new DesktopHost(this._taskbar.settings, this._taskbar.recycleBin));
        if (this._desktopHost) {
            this._desktopBubble = startExtra('desktop bubble',
                () => new DesktopBubble(this._taskbar.settings, this._taskbar.recycleBin, this._desktopHost));
        }
    }

    disable() {
        this._desktopBubble?.destroy();
        this._desktopBubble = null;
        this._desktopHost?.destroy();
        this._desktopHost = null;
        this._taskbar.destroy();
        this._taskbar = null;
    }
}
