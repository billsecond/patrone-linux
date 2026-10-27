// Role slots: taskbar pins that stand for a job (web browser, terminal, files,
// mail, documents, spreadsheets, presentations, PDF files, art) instead of a
// particular app. A slot opens whichever app is the default for its job, and
// that app's windows show under the slot: WordCraft for documents while it's
// the default for them, LibreOffice Writer once that is (the user,
// 2026-10-26). A slot with no app for its job doesn't show.

import Gio from 'gi://Gio';
import Shell from 'gi://Shell';

// Each role: its name; the file or link types whose default app it opens,
// tried in order, or else the taskbar setting that names its app and the
// apps it prefers; and the desktop file categories its other choices have
// (any of them, and none of `exclude`)
export const ROLES = {
    web: {
        name: 'Web browser',
        categories: ['WebBrowser'],
        types: ['x-scheme-handler/https', 'x-scheme-handler/http', 'text/html'],
    },
    terminal: {
        name: 'Terminal',
        categories: ['TerminalEmulator'],
        setting: 'terminal-app',
        preferred: [
            'org.patrone.Terminal.desktop',
            'org.gnome.Ptyxis.desktop',
            'org.gnome.Terminal.desktop',
        ],
    },
    files: {
        name: 'File manager',
        categories: ['FileManager'],
        types: ['inode/directory'],
    },
    mail: {
        name: 'Mail',
        categories: ['Email'],
        types: ['x-scheme-handler/mailto'],
    },
    documents: {
        name: 'Documents',
        menuName: 'Default documents app',
        categories: ['WordProcessor'],
        types: [
            'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
            'application/vnd.oasis.opendocument.text',
            'application/msword',
        ],
    },
    spreadsheets: {
        name: 'Spreadsheets',
        menuName: 'Default spreadsheet app',
        categories: ['Spreadsheet'],
        types: [
            'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
            'application/vnd.oasis.opendocument.spreadsheet',
            'application/vnd.ms-excel',
        ],
    },
    presentations: {
        name: 'Presentations',
        menuName: 'Default presentation app',
        categories: ['Presentation'],
        types: [
            'application/vnd.openxmlformats-officedocument.presentationml.presentation',
            'application/vnd.oasis.opendocument.presentation',
            'application/vnd.ms-powerpoint',
        ],
    },
    pdf: {
        name: 'PDF files',
        menuName: 'Default PDF app',
        types: ['application/pdf'],
    },
    // Drawing has no file type of its own (a PNG's default is a viewer), so
    // like the terminal it has a setting, and apps it looks for first
    art: {
        name: 'Art',
        menuName: 'Default art app',
        categories: ['2DGraphics', 'RasterGraphics', 'VectorGraphics'],
        exclude: ['Viewer'],
        setting: 'art-app',
        preferred: [
            'ai.storyteller.artcraft.desktop',
            'libreoffice-draw.desktop',
            'org.gimp.GIMP.desktop',
            'org.kde.krita.desktop',
            'org.inkscape.Inkscape.desktop',
            'com.github.PintaProject.Pinta.desktop',
        ],
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

/** What a slot's "Default ..." menu is called. */
export function roleMenuName(role) {
    return ROLES[role].menuName ?? `Default ${ROLES[role].name.toLowerCase()}`;
}

function lookup(id) {
    return id ? Shell.AppSystem.get_default().lookup_app(id) : null;
}

/**
 * The app a role slot opens: the system's default for the first of the
 * role's types that has one; or, for a role without types, the one chosen in
 * the taskbar's settings, else the first preferred app installed. Failing
 * those, the first app that could do the job.
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
        for (const type of def.types) {
            const app = lookup(Gio.AppInfo.get_default_for_type(type, false)?.get_id());
            if (app)
                return app;
        }
        return candidatesForRole(role)[0] ?? null;
    }

    for (const id of [settings.get_string(def.setting), ...def.preferred]) {
        const app = lookup(id);
        if (app)
            return app;
    }
    return candidatesForRole(role)[0] ?? null;
}

/**
 * The apps a role slot could open, for its "Default ..." menu: those that
 * open its types, or have its categories, and the preferred ones installed.
 *
 * @param {string} role
 * @returns {Shell.App[]}
 */
export function candidatesForRole(role) {
    const def = ROLES[role];
    const infos = def.types
        ? def.types.flatMap(type => Gio.AppInfo.get_all_for_type(type))
        : Gio.AppInfo.get_all();

    const apps = new Map();
    for (const info of infos) {
        if (!info.should_show())
            continue;
        const categories = info.get_categories?.()?.split(';') ?? [];
        if (def.categories && !def.categories.some(category => categories.includes(category)))
            continue;
        if (def.exclude?.some(category => categories.includes(category)))
            continue;
        const app = lookup(info.get_id());
        if (app)
            apps.set(app.get_id(), app);
    }
    for (const app of (def.preferred ?? []).map(lookup).filter(Boolean))
        apps.set(app.get_id(), app);
    return [...apps.values()].sort(
        (a, b) => a.get_name().localeCompare(b.get_name()));
}

/**
 * Makes an app the role's default: the system default for the role's types
 * (as Settings → Apps → Default Apps does), or the taskbar's setting for it.
 *
 * @param {string} role
 * @param {Shell.App} app
 * @param {Gio.Settings} settings - the taskbar's settings
 */
export function setAppForRole(role, app, settings) {
    const def = ROLES[role];
    if (def.types) {
        // Only for the types it opens (WordCraft needn't take old .doc files)
        const supported = app.app_info.get_supported_types?.() ?? [];
        for (const [i, type] of def.types.entries()) {
            if (i === 0 || supported.length === 0 || supported.includes(type))
                app.app_info.set_as_default_for_type(type);
        }
    } else {
        settings.set_string(def.setting, app.get_id());
    }
}
