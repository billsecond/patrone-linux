//! `org.freedesktop.FileManager1`, the D-Bus interface other apps use to
//! show a file in the file manager (a browser's "Show in folder", for
//! example): ShowFolders, ShowItems and ShowItemProperties.

use gtk::{gio, glib, prelude::*};

use crate::location::Location;
use crate::window::{self, Window};

const INTERFACE: &str = r#"
<node>
  <interface name="org.freedesktop.FileManager1">
    <method name="ShowFolders">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
    <method name="ShowItems">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
    <method name="ShowItemProperties">
      <arg type="as" name="URIs" direction="in"/>
      <arg type="s" name="StartupId" direction="in"/>
    </method>
  </interface>
</node>
"#;

/// Registers the interface on the app's session bus connection. Another
/// file manager may already own the name; then this one doesn't answer.
pub fn register(app: &gtk::Application) {
    let Some(connection) = app.dbus_connection() else {
        return;
    };
    let Ok(node) = gio::DBusNodeInfo::for_xml(INTERFACE) else {
        return;
    };
    let Some(interface) = node.lookup_interface("org.freedesktop.FileManager1") else {
        return;
    };
    let app_weak = app.downgrade();
    let registered = connection
        .register_object("/org/freedesktop/FileManager1", &interface)
        .method_call(move |_, _, _, _, method, parameters, invocation| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let uris: Vec<String> = parameters
                .child_value(0)
                .get::<Vec<String>>()
                .unwrap_or_default();
            match method {
                "ShowFolders" => show_folders(&app, &uris),
                "ShowItems" => show_items(&app, &uris),
                "ShowItemProperties" => show_properties(&app, &uris),
                _ => {}
            }
            invocation.return_value(None);
        })
        .build();
    if registered.is_ok() {
        gio::bus_own_name_on_connection(
            &connection,
            "org.freedesktop.FileManager1",
            gio::BusNameOwnerFlags::NONE,
            |_, _| {},
            |_, _| {},
        );
    }
}

fn show_folders(app: &gtk::Application, uris: &[String]) {
    for uri in uris {
        Window::new(app, Location::Folder(gio::File::for_uri(uri))).present();
    }
}

/// Opens each item's folder with the item selected.
pub fn show_items(app: &gtk::Application, uris: &[String]) {
    for uri in uris {
        let file = gio::File::for_uri(uri);
        let Some(folder) = file.parent() else {
            continue;
        };
        let window = Window::new(app, Location::Folder(folder));
        window.present();
        if let Some(name) = file.basename() {
            window
                .tab()
                .folder
                .select_when_present(vec![name.to_string_lossy().into_owned()], false);
        }
    }
}

fn show_properties(app: &gtk::Application, uris: &[String]) {
    let window = window::all().into_iter().next().unwrap_or_else(|| {
        let window = Window::new(app, window::start_location());
        window.present();
        window
    });
    let files: Vec<gio::File> = uris.iter().map(|uri| gio::File::for_uri(uri)).collect();
    glib::spawn_future_local(async move {
        let mut infos = Vec::new();
        for file in files {
            if let Some(info) = crate::home::info_for(&file).await {
                infos.push(info);
            }
        }
        crate::properties::show(&window, infos);
    });
}
