// Patrone Taskbar: Patrone Desktop's taskbar for its GNOME-based desktop,
// until Patrone's own shell replaces GNOME Shell. See shell/gnome/README.md.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {DesktopBubble} from './desktopBubble.js';
import {Taskbar} from './taskbar.js';

export default class PatroneTaskbarExtension extends Extension {
    enable() {
        this._taskbar = new Taskbar(this);
        this._desktopBubble = new DesktopBubble(this._taskbar.settings, this._taskbar.recycleBin);
    }

    disable() {
        this._desktopBubble.destroy();
        this._desktopBubble = null;
        this._taskbar.destroy();
        this._taskbar = null;
    }
}
