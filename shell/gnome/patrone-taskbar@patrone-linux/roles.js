// Role slots: taskbar pins that stand for a job (web browser, terminal, files,
// mail) instead of a particular app. A slot opens whichever app is the
// default for its job, and that app's windows show under the slot.

import Gio from 'gi://Gio';
import Shell from 'gi://Shell';

export const ROLES = {
    web: {
        name: 'Web browser',
        icon: 'web-browser',
        category: 'WebBrowser',
        types: ['x-scheme-handler/https', 'x-scheme-handler/http', 'text/html'],
    },
    terminal: {
        name: 'Terminal',
        icon: 'utilities-terminal',
        category: 'TerminalEmulator',
        preferred: [
            'org.patrone.Terminal.desktop',
            'org.gnome.Ptyxis.desktop',
            'org.gnome.Terminal.desktop',
        ],
    },
    files: {
        name: 'File manager',
        icon: 'system-file-manager',
        category: 'FileManager',
        types: ['inode/directory'],
    },
    mail: {
        name: 'Mail',
        icon: 'internet-mail',
        category: 'Email',
        types: ['x-scheme-handler/mailto'],
    },
};

const ROLE_PREFIX = 'role:';

/**
 * @param {string} entry - a pinned entry: a desktop file ID or "role:NAME"
 * @returns {string|null} the role's name, or null for an app
 */
export function roleOf(entry) {
    if (!entry?.startsWith(ROLE_PREFIX))
        return null;
    const role = entry.slice(ROLE_PREFIX.length);
    return ROLES[role] ? role : null;
}

export function roleEntry(role) {
    return `${ROLE_PREFIX}${role}`;
}

function lookup(id) {
    return id ? Shell.AppSystem.get_default().lookup_app(id) : null;
}

/**
 * The app a role slot opens: the system's default for the role's file or
 * link type, or for the terminal (which has no such type), the one chosen in
 * the taskbar's settings, else the first terminal installed.
 *
 * @param {string} role
 * @param {Gio.Settings} settings - the taskbar's settings
 * @returns {Shell.App|null}
 */
export function appForRole(role, settings) {
    const def = ROLES[role];
    if (!def)
        return null;

    if (def.types) {
        const info = Gio.AppInfo.get_default_for_type(def.types[0], false);
        return lookup(info?.get_id()) ?? candidatesForRole(role)[0] ?? null;
    }

    for (const id of [settings.get_string('terminal-app'), ...def.preferred]) {
        const app = lookup(id);
        if (app)
            return app;
    }
    return candidatesForRole(role)[0] ?? null;
}

/**
 * The apps a role slot could open, for its "Change default app" menu.
 *
 * @param {string} role
 * @returns {Shell.App[]}
 */
export function candidatesForRole(role) {
    const def = ROLES[role];
    const infos = def.types
        ? Gio.AppInfo.get_all_for_type(def.types[0])
        : Gio.AppInfo.get_all();

    const apps = new Map();
    for (const info of infos) {
        if (!info.should_show())
            continue;
        const categories = info.get_categories?.()?.split(';') ?? [];
        if (!categories.includes(def.category))
            continue;
        const app = lookup(info.get_id());
        if (app)
            apps.set(app.get_id(), app);
    }
    return [...apps.values()].sort(
        (a, b) => a.get_name().localeCompare(b.get_name()));
}

/**
 * Makes an app the role's default: the system default for the role's types
 * (as Settings → Apps → Default Apps does), or the taskbar's terminal.
 *
 * @param {string} role
 * @param {Shell.App} app
 * @param {Gio.Settings} settings - the taskbar's settings
 */
export function setAppForRole(role, app, settings) {
    const def = ROLES[role];
    if (def.types) {
        for (const type of def.types)
            app.app_info.set_as_default_for_type(type);
    } else {
        settings.set_string('terminal-app', app.get_id());
    }
}
