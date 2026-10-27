// The Recycle Bin's drop target: a clear window the size of the Recycle Bin,
// which the taskbar (recycleBin.js) launches, places right under it and
// stacks with the desktop icons. GNOME Shell's own actors can't take drops
// from apps, so while something is dragged the Recycle Bin lets the drop
// through to this window, which moves what's dropped to the trash, as on
// Windows (the user, 2026-10-26: "I can't drag into the trash a folder").
//
// Run by the taskbar, never imported by GNOME Shell:
//   gjs -m dropTarget.js WIDTH HEIGHT
//
// A Gtk.Application, with a background that's all but clear: drawn fully
// clear, GTK gave the window no buffer, so GNOME Shell never showed it (the
// Recycle Bin covers it anyway).

import Gdk from 'gi://Gdk?version=4.0';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk?version=4.0';
import System from 'system';

const [width, height] = System.programArgs.map(Number);

function trash(file) {
    file.trash_async(GLib.PRIORITY_DEFAULT, null, (trashed, result) => {
        try {
            trashed.trash_finish(result);
            print(`Moved ${trashed.get_uri()} to the trash`);
        } catch (e) {
            printerr(`Can't move ${trashed.get_uri()} to the trash: ${e.message}`);
        }
    });
}

const app = new Gtk.Application({
    application_id: 'org.patrone.RecycleBin.DropTarget',
    flags: Gio.ApplicationFlags.NON_UNIQUE,
});

app.connect('activate', () => {
    const style = new Gtk.CssProvider();
    style.load_from_string('window { background: rgba(0, 0, 0, 0.01); box-shadow: none; border-radius: 0; } ' +
        'box { background: none; }');
    Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), style,
        Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION);

    const window = new Gtk.ApplicationWindow({
        application: app,
        title: 'Patrone Recycle Bin',
        decorated: false,
        default_width: width || 92,
        default_height: height || 85,
    });
    const area = new Gtk.Box({hexpand: true, vexpand: true});
    const target = Gtk.DropTarget.new(Gdk.FileList.$gtype,
        Gdk.DragAction.MOVE | Gdk.DragAction.COPY | Gdk.DragAction.LINK);
    target.connect('drop', (_target, files) => {
        for (const file of files.get_files())
            trash(file);
        return true;
    });
    area.add_controller(target);
    window.set_child(area);
    window.present();
});

app.run([]);
