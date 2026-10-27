// Patrone Taskbar: Patrone Desktop's taskbar for its GNOME-based desktop,
// until Patrone's own shell replaces GNOME Shell. See shell/gnome/README.md.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {DesktopBubble} from './desktopBubble.js';
import {DesktopHost} from './desktopHost.js';
import {Taskbar} from './taskbar.js';
import {startExtra} from './util.js';

export default class PatroneTaskbarExtension extends Extension {
    enable() {
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
