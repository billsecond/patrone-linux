import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

/**
 * An app's windows that belong on the taskbar (not dialogs and the like),
 * most recently used first.
 *
 * @param {Shell.App} app
 * @returns {Meta.Window[]}
 */
export function taskbarWindows(app) {
    return app.get_windows().filter(window => !window.skip_taskbar);
}

/**
 * Opens an app the way Windows users expect from a menu: a new window when
 * it's running and can open one, else starts it (or shows it, if it can't).
 *
 * @param {Shell.App} app
 * @param {boolean} newWindow - open a new window of a running app
 */
export function launchApp(app, newWindow = true) {
    if (newWindow && app.state === Shell.AppState.RUNNING && app.can_open_new_window())
        app.open_new_window(-1);
    else
        app.activate();
    Main.overview.hide();
}

/**
 * Starts one of the taskbar's extras (the Recycle Bin, the windows placed at
 * sign-in, the desktop panel), or logs why it couldn't: a fault in one
 * mustn't cost the whole taskbar, as one in the Recycle Bin did at the
 * user's sign-in (2026-10-27).
 *
 * @param {string} name - what it is, for the log
 * @param {Function} start - makes it
 * @returns {object|null} what start made, or null
 */
export function startExtra(name, start) {
    try {
        return start();
    } catch (e) {
        logError(e, `Patrone taskbar: the ${name} didn't start`);
        return null;
    }
}
