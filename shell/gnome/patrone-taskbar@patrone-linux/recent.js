// Recently used files, read from the list GTK apps keep
// (~/.local/share/recently-used.xbel), for the Patrone menu's "Recent" and
// the taskbar's jump lists, and the folders pinned in the file manager
// (GTK's bookmarks), for the file manager's jump list.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

Gio._promisify(Gio.File.prototype, 'load_contents_async');

const RECENT_FILE = GLib.build_filenamev([GLib.get_user_data_dir(), 'recently-used.xbel']);
const BOOKMARKS_FILE = GLib.build_filenamev([GLib.get_user_config_dir(), 'gtk-3.0', 'bookmarks']);

function decode(bytes) {
    return new TextDecoder().decode(bytes);
}

function displayName(uri) {
    const file = Gio.File.new_for_uri(uri);
    const name = file.get_basename();
    try {
        return GLib.filename_display_name(name) || uri;
    } catch {
        return name || uri;
    }
}

/**
 * @typedef {object} RecentItem
 * @property {string} uri
 * @property {string} name - shown to the user
 * @property {string} mimeType
 * @property {Gio.Icon} gicon
 * @property {number} time - when last used, in seconds since 1970
 * @property {string[]} apps - the names apps registered it under
 * @property {string[]} execs - the commands those apps run it with
 */

export class RecentFiles {
    constructor() {
        /** @type {RecentItem[]} */
        this.items = [];
        this.bookmarks = [];
        this._cancellable = new Gio.Cancellable();

        this._recentFile = Gio.File.new_for_path(RECENT_FILE);
        this._recentMonitor = this._recentFile.monitor_file(Gio.FileMonitorFlags.NONE, null);
        this._recentMonitor.connect('changed', () => this._loadRecent());

        this._bookmarksFile = Gio.File.new_for_path(BOOKMARKS_FILE);
        this._bookmarksMonitor = this._bookmarksFile.monitor_file(Gio.FileMonitorFlags.NONE, null);
        this._bookmarksMonitor.connect('changed', () => this._loadBookmarks());

        this._loadRecent();
        this._loadBookmarks();
    }

    destroy() {
        this._cancellable.cancel();
        this._recentMonitor.cancel();
        this._bookmarksMonitor.cancel();
    }

    async _read(file) {
        try {
            const [contents] = await file.load_contents_async(this._cancellable);
            return contents;
        } catch (e) {
            if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND) &&
                !e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Patrone taskbar: can't read ${file.get_path()}: ${e.message}`);
            return null;
        }
    }

    async _loadRecent() {
        const contents = await this._read(this._recentFile);
        if (contents === null) {
            this.items = [];
            return;
        }

        const bookmarks = new GLib.BookmarkFile();
        try {
            bookmarks.load_from_data(contents);
        } catch (e) {
            console.warn(`Patrone taskbar: can't parse the recent files list: ${e.message}`);
            return;
        }

        const items = [];
        for (const uri of bookmarks.get_uris()) {
            try {
                if (bookmarks.get_is_private(uri))
                    continue;
            } catch {}

            const times = [];
            for (const get of ['get_modified_date_time', 'get_visited_date_time', 'get_added_date_time']) {
                try {
                    times.push(bookmarks[get](uri)?.to_unix() ?? 0);
                } catch {}
            }

            let mimeType = 'application/octet-stream';
            try {
                mimeType = bookmarks.get_mime_type(uri);
            } catch {}

            let apps = [];
            const execs = [];
            try {
                apps = bookmarks.get_applications(uri);
                for (const app of apps) {
                    try {
                        const [, exec] = bookmarks.get_application_info(uri, app);
                        if (exec)
                            execs.push(exec);
                    } catch {}
                }
            } catch {}

            items.push({
                uri,
                name: displayName(uri),
                mimeType,
                gicon: Gio.content_type_get_icon(mimeType),
                time: Math.max(0, ...times),
                apps,
                execs,
            });
        }
        items.sort((a, b) => b.time - a.time);
        this.items = items;
    }

    async _loadBookmarks() {
        const contents = await this._read(this._bookmarksFile);
        this.bookmarks = (contents ? decode(contents) : '').split('\n').filter(line => line.trim()).map(line => {
            const space = line.indexOf(' ');
            const uri = space < 0 ? line : line.slice(0, space);
            const label = space < 0 ? '' : line.slice(space + 1).trim();
            return {uri, name: label || displayName(uri)};
        });
    }

    /**
     * Recent files, newest first, that still exist (local files are
     * checked; others are trusted).
     *
     * @param {number} limit
     * @param {(item: RecentItem) => boolean} [filter]
     * @returns {RecentItem[]}
     */
    recent(limit, filter = () => true) {
        const result = [];
        for (const item of this.items) {
            if (result.length >= limit)
                break;
            if (!filter(item))
                continue;
            if (item.uri.startsWith('file://')) {
                const path = Gio.File.new_for_uri(item.uri).get_path();
                // Don't wait on network mounts
                if (path && !path.startsWith('/run/user/') &&
                    !GLib.file_test(path, GLib.FileTest.EXISTS))
                    continue;
            }
            result.push(item);
        }
        return result;
    }

    /**
     * The files an app opened recently: those GTK recorded under the app,
     * then, if there are few, recent files of types it opens (not for web
     * browsers, which can open almost anything).
     *
     * @param {Shell.App} app
     * @param {number} limit
     * @param {boolean} byType - fill up with files of types the app opens
     * @returns {RecentItem[]}
     */
    recentForApp(app, limit, byType) {
        const info = app.app_info;
        if (!info)
            return [];

        const id = app.get_id().replace(/\.desktop$/, '').toLowerCase();
        const names = new Set([
            id,
            id.split('.').at(-1),
            id.split('_').at(0),
            app.get_name().toLowerCase(),
        ]);
        const executable = info.get_executable();
        if (executable)
            names.add(GLib.path_get_basename(executable).toLowerCase());

        const ownedBy = item =>
            item.apps.some(name => names.has(name.toLowerCase())) ||
            item.execs.some(exec => names.has(
                GLib.path_get_basename(exec.split(' ')[0].replace(/'/g, '')).toLowerCase()));

        const result = this.recent(limit, ownedBy);
        if (!byType || result.length >= limit)
            return result;

        const types = new Set(info.get_supported_types?.() ?? []);
        if (types.size === 0)
            return result;
        const seen = new Set(result.map(item => item.uri));
        return result.concat(this.recent(limit - result.length,
            item => !seen.has(item.uri) && types.has(item.mimeType)));
    }

    /**
     * Folders for the file manager's jump list: the ones pinned in the file
     * manager, or the usual folders if none are.
     *
     * @returns {{uri: string, name: string}[]}
     */
    pinnedFolders() {
        if (this.bookmarks.length)
            return this.bookmarks.slice(0, 10);

        const folders = [{uri: Gio.File.new_for_path(GLib.get_home_dir()).get_uri(), name: 'Home'}];
        for (const dir of [
            GLib.UserDirectory.DIRECTORY_DESKTOP,
            GLib.UserDirectory.DIRECTORY_DOCUMENTS,
            GLib.UserDirectory.DIRECTORY_DOWNLOAD,
            GLib.UserDirectory.DIRECTORY_PICTURES,
            GLib.UserDirectory.DIRECTORY_MUSIC,
            GLib.UserDirectory.DIRECTORY_VIDEOS,
        ]) {
            const path = GLib.get_user_special_dir(dir);
            if (path && path !== GLib.get_home_dir())
                folders.push({uri: Gio.File.new_for_path(path).get_uri(), name: GLib.path_get_basename(path)});
        }
        return folders;
    }
}

/**
 * "Just now", "5 min ago", "Yesterday", "Oct 20": when a file was last used.
 *
 * @param {number} time - seconds since 1970
 * @returns {string}
 */
export function timeAgo(time) {
    if (!time)
        return '';
    const now = GLib.DateTime.new_now_local();
    const then = GLib.DateTime.new_from_unix_local(time);
    const seconds = now.to_unix() - time;
    if (seconds < 60)
        return 'Just now';
    if (seconds < 3600)
        return `${Math.floor(seconds / 60)} min ago`;
    if (then.get_day_of_year() === now.get_day_of_year() && then.get_year() === now.get_year())
        return `${Math.floor(seconds / 3600)} h ago`;
    const yesterday = now.add_days(-1);
    if (then.get_day_of_year() === yesterday.get_day_of_year() && then.get_year() === yesterday.get_year())
        return 'Yesterday';
    return then.format(then.get_year() === now.get_year() ? '%b %-d' : '%b %-d, %Y');
}
