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
 * Opens an app the way Windows' Start menu does: a new window when it's
 * running and can open one, else starts it (or shows it, if it can't).
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
