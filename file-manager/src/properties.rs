//! The Properties window (Alt+Enter), as on Windows: General (name, type,
//! the app it opens with, location, size and size on disk, what a folder
//! contains, dates, Read-only and Hidden), Permissions (Linux's version of
//! Windows' Security tab) and Details. Drives show a chart of used and
//! free space instead.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::{gdk_pixbuf, gio, glib, prelude::*};

use crate::dialogs::Dialog;
use crate::filecell;
use crate::icons;
use crate::ops;
use crate::places;
use crate::util;
use crate::window::Window;

/// Running totals for folders, counted in the background.
#[derive(Default)]
struct Totals {
    bytes: Cell<u64>,
    allocated: Cell<u64>,
    files: Cell<u64>,
    folders: Cell<u64>,
    done: Cell<bool>,
}

fn row(grid: &gtk::Grid, line: &mut i32, label: &str, value: &impl IsA<gtk::Widget>) {
    let key = gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .yalign(0.0)
        .build();
    key.add_css_class("property-key");
    grid.attach(&key, 0, *line, 1, 1);
    grid.attach(value, 1, *line, 1, 1);
    *line += 1;
}

fn text(value: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(value)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .hexpand(true)
        .width_chars(28)
        .max_width_chars(40)
        .build()
}

fn rule(grid: &gtk::Grid, line: &mut i32) {
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.add_css_class("property-rule");
    grid.attach(&separator, 0, *line, 2, 1);
    *line += 1;
}

fn page() -> (gtk::Box, gtk::Grid) {
    let grid = gtk::Grid::builder()
        .row_spacing(8)
        .column_spacing(16)
        .build();
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("property-page");
    page.append(&grid);
    (page, grid)
}

/// Counts everything under the folders, updating `totals` as it goes.
async fn count(files: Vec<gio::File>, totals: Rc<Totals>, cancellable: gio::Cancellable) {
    let mut stack = files;
    while let Some(folder) = stack.pop() {
        if cancellable.is_cancelled() {
            return;
        }
        let Ok(enumerator) = folder
            .enumerate_children_future(
                "standard::type,standard::size,standard::allocated-size,standard::name",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                glib::Priority::LOW,
            )
            .await
        else {
            continue;
        };
        while let Ok(infos) = enumerator.next_files_future(500, glib::Priority::LOW).await {
            if infos.is_empty() || cancellable.is_cancelled() {
                break;
            }
            for info in infos {
                if info.file_type() == gio::FileType::Directory {
                    totals.folders.set(totals.folders.get() + 1);
                    stack.push(folder.child(info.name()));
                } else {
                    totals.files.set(totals.files.get() + 1);
                    totals
                        .bytes
                        .set(totals.bytes.get() + info.size().max(0) as u64);
                    totals.allocated.set(
                        totals.allocated.get() + info.attribute_uint64("standard::allocated-size"),
                    );
                }
            }
        }
    }
    totals.done.set(true);
}

fn contains_text(files: u64, folders: u64) -> String {
    let files = if files == 1 {
        "1 File".to_owned()
    } else {
        format!("{} Files", util::thousands(files))
    };
    let folders = if folders == 1 {
        "1 Folder".to_owned()
    } else {
        format!("{} Folders", util::thousands(folders))
    };
    format!("{files}, {folders}")
}

pub fn show(window: &Window, infos: Vec<gio::FileInfo>) {
    if infos.is_empty() {
        return;
    }
    if infos.len() == 1 && infos[0].boolean("pfm::drive") {
        show_drive(window, &infos[0]);
        return;
    }
    let single = infos.len() == 1;
    let first = infos[0].clone();
    let title = if single {
        format!("{} Properties", first.display_name())
    } else {
        "Properties".to_owned()
    };
    let dialog = Dialog::new(None, &title);
    dialog.window.set_transient_for(Some(&window.gtk_window()));
    dialog.window.set_modal(false);
    dialog.window.set_default_size(400, -1);
    dialog.window.add_css_class("properties-window");
    let notebook = gtk::Notebook::new();
    notebook.add_css_class("property-tabs");
    dialog.body.append(&notebook);

    let files: Vec<gio::File> = infos.iter().filter_map(util::file_of).collect();
    let folders: Vec<gio::File> = infos
        .iter()
        .filter(|info| util::is_dir(info))
        .filter_map(util::file_of)
        .collect();
    let in_trash = files.iter().any(places::is_trash);

    // General.
    let (general, grid) = page();
    let mut line = 0;
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let icon = if single {
        icons::for_info(&first)
    } else {
        "pfm-file"
    };
    header.append(&icons::image(icon, 32));
    let name_entry = gtk::Entry::builder().hexpand(true).build();
    if single {
        name_entry.set_text(&first.edit_name());
        name_entry.set_sensitive(!in_trash && first.boolean("access::can-rename"));
    } else {
        let file_count = infos.iter().filter(|info| !util::is_dir(info)).count() as u64;
        let folder_count = infos.len() as u64 - file_count;
        name_entry.set_text(&contains_text(file_count, folder_count));
        name_entry.set_sensitive(false);
    }
    header.append(&name_entry);
    grid.attach(&header, 0, line, 2, 1);
    line += 1;
    rule(&grid, &mut line);

    let mime = util::content_type(&first);
    if single && !util::is_dir(&first) {
        let name = first.name().to_string_lossy().into_owned();
        let (_, extension) = util::split_extension(&name);
        let kind = if extension.is_empty() {
            filecell::type_text(&first)
        } else {
            format!("{} ({extension})", filecell::type_text(&first))
        };
        row(&grid, &mut line, "Type of file:", &text(&kind));
        let opens = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let app = gio::AppInfo::default_for_type(&mime, false);
        if let Some(app) = &app
            && let Some(gicon) = app.icon()
        {
            let image = gtk::Image::from_gicon(&gicon);
            image.set_pixel_size(16);
            opens.append(&image);
        }
        let app_name = gtk::Label::builder()
            .label(
                app.as_ref()
                    .map(|a| a.display_name().to_string())
                    .unwrap_or_else(|| "Pick an app".into()),
            )
            .xalign(0.0)
            .hexpand(true)
            .build();
        opens.append(&app_name);
        let change = gtk::Button::with_mnemonic("C_hange…");
        change.add_css_class("dialog-button");
        {
            let window = window.clone();
            let files = files.clone();
            let mime = mime.clone();
            change.connect_clicked(move |_| {
                crate::openwith::show_choose_default(&window, files.clone(), mime.clone())
            });
        }
        opens.append(&change);
        row(&grid, &mut line, "Opens with:", &opens);
    } else if single {
        row(&grid, &mut line, "Type:", &text("File folder"));
    } else {
        let same_type = infos
            .iter()
            .all(|info| filecell::type_text(info) == filecell::type_text(&first));
        row(
            &grid,
            &mut line,
            "Type:",
            &text(&if same_type {
                format!("All of type {}", filecell::type_text(&first))
            } else {
                "Multiple types".into()
            }),
        );
    }
    rule(&grid, &mut line);

    let parents: Vec<String> = files
        .iter()
        .filter_map(|f| f.parent())
        .map(|p| util::display_path(&p))
        .collect();
    let location = if in_trash && single {
        filecell::column_text(&first, crate::viewstate::Column::Origin, false)
    } else if parents.iter().all(|p| Some(p) == parents.first()) {
        let path = parents.first().cloned().unwrap_or_default();
        if single {
            path
        } else {
            format!("All in {path}")
        }
    } else {
        "Various folders".into()
    };
    row(&grid, &mut line, "Location:", &text(&location));
    let size_label = text("");
    let disk_label = text("");
    row(&grid, &mut line, "Size:", &size_label);
    row(&grid, &mut line, "Size on disk:", &disk_label);
    let contains_label = text("");
    if !folders.is_empty() {
        row(&grid, &mut line, "Contains:", &contains_label);
    }

    // Folder contents are counted in the background, as on Windows.
    let totals = Rc::new(Totals::default());
    for info in infos.iter().filter(|info| !util::is_dir(info)) {
        totals
            .bytes
            .set(totals.bytes.get() + info.size().max(0) as u64);
        totals
            .allocated
            .set(totals.allocated.get() + info.attribute_uint64("standard::allocated-size"));
    }
    let cancellable = gio::Cancellable::new();
    {
        let totals = totals.clone();
        let cancellable = cancellable.clone();
        glib::spawn_future_local(count(folders.clone(), totals, cancellable));
    }
    {
        let totals = totals.clone();
        let has_folders = !folders.is_empty();
        let (size_label, disk_label, contains_label) = (
            size_label.clone(),
            disk_label.clone(),
            contains_label.clone(),
        );
        let update = move || {
            size_label.set_label(&util::size_with_bytes(totals.bytes.get()));
            disk_label.set_label(&util::size_with_bytes(totals.allocated.get()));
            if has_folders {
                contains_label.set_label(&contains_text(totals.files.get(), totals.folders.get()));
            }
            !totals.done.get() && has_folders
        };
        update();
        glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
            if update() {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });
    }
    {
        let cancellable = cancellable.clone();
        dialog.window.connect_destroy(move |_| cancellable.cancel());
    }
    rule(&grid, &mut line);

    if single {
        let long = |time: Option<glib::DateTime>| {
            time.map(|t| util::date_time_long(&t)).unwrap_or_default()
        };
        if let Some(deleted) = filecell::deletion_date(&first) {
            row(
                &grid,
                &mut line,
                "Deleted:",
                &text(&util::date_time_long(&deleted)),
            );
        }
        row(
            &grid,
            &mut line,
            "Created:",
            &text(&long(util::created(&first))),
        );
        row(
            &grid,
            &mut line,
            "Modified:",
            &text(&long(util::modified(&first))),
        );
        row(
            &grid,
            &mut line,
            "Accessed:",
            &text(&long(util::accessed(&first))),
        );
        rule(&grid, &mut line);
    }

    let attributes = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let read_only = gtk::CheckButton::with_mnemonic("_Read-only");
    let hidden = gtk::CheckButton::with_mnemonic("H_idden");
    let writable = |info: &gio::FileInfo| info.attribute_uint32("unix::mode") & 0o200 != 0;
    read_only.set_active(
        infos
            .iter()
            .all(|info| info.has_attribute("unix::mode") && !writable(info)),
    );
    hidden.set_active(infos.iter().all(|info| info.is_hidden()));
    // Names starting with a dot are always hidden on Linux.
    let dot_named = infos
        .iter()
        .any(|info| info.name().to_string_lossy().starts_with('.'));
    hidden.set_sensitive(!dot_named && !in_trash);
    read_only.set_sensitive(!in_trash && infos.iter().all(|info| info.has_attribute("unix::mode")));
    attributes.append(&read_only);
    attributes.append(&hidden);
    row(&grid, &mut line, "Attributes:", &attributes);
    notebook.append_page(&general, Some(&gtk::Label::new(Some("General"))));

    // Permissions.
    let permissions = (single && first.has_attribute("unix::mode") && !in_trash)
        .then(|| files.first())
        .flatten()
        .map(|file| crate::permissions::Permissions::new(&first, file));
    if let Some(permissions) = &permissions {
        notebook.append_page(
            &permissions.widget,
            Some(&gtk::Label::new(Some("Permissions"))),
        );
        // General's Read-only box and the Write boxes stay in step.
        let page = Rc::downgrade(permissions);
        read_only.connect_toggled(move |read_only| {
            if let Some(page) = page.upgrade() {
                page.set_read_only(read_only.is_active());
            }
        });
    }

    // Details.
    if single && !util::is_dir(&first) {
        let (details, grid) = page();
        let mut line = 0;
        let heading = |grid: &gtk::Grid, line: &mut i32, title: &str| {
            let label = gtk::Label::builder().label(title).xalign(0.0).build();
            label.add_css_class("details-heading");
            grid.attach(&label, 0, *line, 2, 1);
            *line += 1;
        };
        heading(&grid, &mut line, "File");
        row(&grid, &mut line, "Name", &text(&first.display_name()));
        row(
            &grid,
            &mut line,
            "Item type",
            &text(&filecell::type_text(&first)),
        );
        if let Some(folder) = files.first().and_then(|f| f.parent()) {
            row(
                &grid,
                &mut line,
                "Folder path",
                &text(&util::display_path(&folder)),
            );
        }
        row(
            &grid,
            &mut line,
            "Size",
            &text(&util::size(first.size().max(0) as u64)),
        );
        if let Some(time) = util::created(&first) {
            row(
                &grid,
                &mut line,
                "Date created",
                &text(&util::date_time(&time)),
            );
        }
        if let Some(time) = util::modified(&first) {
            row(
                &grid,
                &mut line,
                "Date modified",
                &text(&util::date_time(&time)),
            );
        }
        row(
            &grid,
            &mut line,
            "Owner",
            &text(&filecell::column_text(
                &first,
                crate::viewstate::Column::Owner,
                false,
            )),
        );
        row(
            &grid,
            &mut line,
            "Permissions",
            &text(&filecell::permissions_text(&first)),
        );
        if mime.starts_with("image/")
            && let Some((format, width, height)) = files
                .first()
                .and_then(|f| f.path())
                .and_then(|p| gdk_pixbuf::Pixbuf::file_info(&p))
        {
            heading(&grid, &mut line, "Image");
            row(
                &grid,
                &mut line,
                "Dimensions",
                &text(&format!("{width} x {height}")),
            );
            row(&grid, &mut line, "Width", &text(&format!("{width} pixels")));
            row(
                &grid,
                &mut line,
                "Height",
                &text(&format!("{height} pixels")),
            );
            row(
                &grid,
                &mut line,
                "Format",
                &text(&format.description().map(String::from).unwrap_or_default()),
            );
        }
        notebook.append_page(&details, Some(&gtk::Label::new(Some("Details"))));
    }

    // OK, Cancel, Apply.
    let initial_name = first.edit_name().to_string();
    let initial_read_only = read_only.is_active();
    let initial_hidden = hidden.is_active();
    let apply = {
        let window = window.clone();
        let infos = infos.clone();
        let files = files.clone();
        let name_entry = name_entry.clone();
        let read_only = read_only.clone();
        let hidden = hidden.clone();
        let permissions = permissions.clone();
        let applied_read_only = Rc::new(Cell::new(initial_read_only));
        let applied_hidden = Rc::new(Cell::new(initial_hidden));
        let applied_name = Rc::new(RefCell::new(initial_name));
        Rc::new(move || {
            let parent = Some(window.gtk_window());
            // With a Permissions tab, Read-only is one of its Write boxes.
            if let Some(permissions) = permissions.clone() {
                applied_read_only.set(read_only.is_active());
                let parent = parent.clone();
                glib::spawn_future_local(async move {
                    if let Err(problem) = permissions.apply().await {
                        crate::dialogs::error(
                            parent.as_ref(),
                            "Permissions",
                            "The permissions couldn't be changed.",
                            &problem,
                        );
                    }
                });
            } else if read_only.is_active() != applied_read_only.get() {
                applied_read_only.set(read_only.is_active());
                for (info, file) in infos.iter().zip(&files) {
                    let mode = info.attribute_uint32("unix::mode");
                    let mode = if read_only.is_active() {
                        mode & !0o222
                    } else {
                        mode | 0o200
                    };
                    let _ = file.set_attribute_uint32(
                        "unix::mode",
                        mode,
                        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                        gio::Cancellable::NONE,
                    );
                }
            }
            if hidden.is_active() != applied_hidden.get() {
                applied_hidden.set(hidden.is_active());
                for file in &files {
                    set_hidden(file, hidden.is_active());
                }
            }
            let name = name_entry.text().to_string();
            if files.len() == 1 && name != *applied_name.borrow() && !name.trim().is_empty() {
                applied_name.replace(name.clone());
                let file = files[0].clone();
                glib::spawn_future_local(async move {
                    ops::rename(parent, file, name, true).await;
                });
            }
        })
    };
    dialog.add_button("OK", 0, true);
    dialog.add_button("Cancel", 1, false);
    // Apply keeps the window open, unlike OK.
    let apply_button = gtk::Button::with_mnemonic("_Apply");
    apply_button.add_css_class("dialog-button");
    {
        let apply = apply.clone();
        apply_button.connect_clicked(move |_| apply());
    }
    dialog.footer.append(&apply_button);
    let dialog_ref = dialog.clone();
    glib::spawn_future_local(async move {
        if dialog_ref.run().await == 0 {
            apply();
        }
    });
}

/// Adds or removes a name in its folder's `.hidden` file, which GIO (and
/// so every GTK app) honors.
fn set_hidden(file: &gio::File, hide: bool) {
    let (Some(folder), Some(name)) = (file.parent(), file.basename()) else {
        return;
    };
    let name = name.to_string_lossy().into_owned();
    let list = folder.child(".hidden");
    let current = list
        .load_contents(gio::Cancellable::NONE)
        .map(|(bytes, _)| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let mut names: Vec<String> = current
        .lines()
        .filter(|line| !line.is_empty() && *line != name)
        .map(str::to_owned)
        .collect();
    if hide {
        names.push(name);
    }
    if names.is_empty() {
        // Nothing hidden any more: no empty .hidden is left behind.
        let _ = list.delete(gio::Cancellable::NONE);
        return;
    }
    let mut contents = names.join("\n");
    contents.push('\n');
    let _ = list.replace_contents(
        contents.as_bytes(),
        None,
        false,
        gio::FileCreateFlags::NONE,
        gio::Cancellable::NONE,
    );
}

/// A drive's Properties: its type, file system, and a chart of used and
/// free space.
fn show_drive(window: &Window, info: &gio::FileInfo) {
    let Some(root) = util::file_of(info) else {
        return;
    };
    let name = info.display_name().to_string();
    let dialog = Dialog::new(None, &format!("{name} Properties"));
    dialog.window.set_transient_for(Some(&window.gtk_window()));
    dialog.window.set_modal(false);
    dialog.window.set_default_size(380, -1);
    let notebook = gtk::Notebook::new();
    notebook.add_css_class("property-tabs");
    dialog.body.append(&notebook);
    let (general, grid) = page();
    let mut line = 0;
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let icon = if places::special_of(&root) == Some(places::Special::Root) {
        "pfm-drive-system"
    } else {
        "pfm-drive"
    };
    header.append(&icons::image(icon, 32));
    let entry = gtk::Entry::builder()
        .text(&name)
        .hexpand(true)
        .sensitive(false)
        .build();
    header.append(&entry);
    grid.attach(&header, 0, line, 2, 1);
    line += 1;
    rule(&grid, &mut line);
    let kind = if root.is_native() {
        "Local Disk"
    } else {
        "Network Drive"
    };
    row(&grid, &mut line, "Type:", &text(kind));
    let file_system = text("");
    row(&grid, &mut line, "File system:", &file_system);
    rule(&grid, &mut line);
    let used_label = text("");
    let free_label = text("");
    let capacity_label = text("");
    let swatch = |class: &str| {
        let square = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        square.set_size_request(14, 14);
        square.add_css_class(class);
        square
    };
    let used_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    used_row.append(&swatch("swatch-used"));
    used_row.append(&used_label);
    let free_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    free_row.append(&swatch("swatch-free"));
    free_row.append(&free_label);
    row(&grid, &mut line, "Used space:", &used_row);
    row(&grid, &mut line, "Free space:", &free_row);
    rule(&grid, &mut line);
    row(&grid, &mut line, "Capacity:", &capacity_label);
    let fraction = Rc::new(Cell::new(0.0f64));
    let chart = gtk::DrawingArea::builder()
        .content_width(140)
        .content_height(140)
        .halign(gtk::Align::Center)
        .build();
    {
        let fraction = fraction.clone();
        chart.set_draw_func(move |_, cr, width, height| {
            let (cx, cy) = (width as f64 / 2.0, height as f64 / 2.0);
            let radius = cx.min(cy) - 4.0;
            let used = fraction.get().clamp(0.0, 1.0);
            let start = -std::f64::consts::FRAC_PI_2;
            cr.set_source_rgb(0.85, 0.87, 0.9);
            cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
            cr.set_source_rgb(0.157, 0.545, 0.98);
            cr.move_to(cx, cy);
            cr.arc(cx, cy, radius, start, start + used * std::f64::consts::TAU);
            cr.close_path();
            let _ = cr.fill();
        });
    }
    grid.attach(&chart, 0, line, 2, 1);
    line += 1;
    let drive_label = gtk::Label::new(Some(&format!("Drive {}", util::display_path(&root))));
    grid.attach(&drive_label, 0, line, 2, 1);
    notebook.append_page(&general, Some(&gtk::Label::new(Some("General"))));
    dialog.add_button("OK", 0, true);
    dialog.add_button("Cancel", 1, false);
    let dialog_ref = dialog.clone();
    glib::spawn_future_local(async move {
        if let Ok(info) = root
            .query_filesystem_info_future(
                "filesystem::size,filesystem::free,filesystem::type",
                glib::Priority::DEFAULT,
            )
            .await
        {
            let size = info.attribute_uint64("filesystem::size");
            let free = info.attribute_uint64("filesystem::free");
            let used = size.saturating_sub(free);
            used_label.set_label(&util::size_with_bytes(used));
            free_label.set_label(&util::size_with_bytes(free));
            capacity_label.set_label(&util::size_with_bytes(size));
            file_system.set_label(
                &info
                    .attribute_string("filesystem::type")
                    .map(|t| t.to_uppercase())
                    .unwrap_or_default(),
            );
            if size > 0 {
                fraction.set(used as f64 / size as f64);
                chart.queue_draw();
            }
        }
        dialog_ref.run().await;
    });
}
