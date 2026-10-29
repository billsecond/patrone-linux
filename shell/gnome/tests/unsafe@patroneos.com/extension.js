// Tests only: turns on GNOME Shell's unsafe mode, which lets
// tests/headless.sh run JavaScript in the throwaway shell and take
// screenshots of it over D-Bus. Never install it on a real desktop.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class UnsafeModeExtension extends Extension {
    enable() {
        global.context.unsafe_mode = true;
    }

    disable() {
        global.context.unsafe_mode = false;
    }
}
