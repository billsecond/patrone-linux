// Patrone Taskbar: Patrone Desktop's taskbar for its GNOME-based desktop,
// until Patrone's own shell replaces GNOME Shell. See shell/gnome/README.md.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {Taskbar} from './taskbar.js';

export default class PatroneTaskbarExtension extends Extension {
    enable() {
        this._taskbar = new Taskbar(this);
    }

    disable() {
        this._taskbar.destroy();
        this._taskbar = null;
    }
}
