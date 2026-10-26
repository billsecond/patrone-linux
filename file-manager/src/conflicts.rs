//! What to do when files being copied or moved already exist at the
//! destination: Windows' "Replace or Skip Files" dialog, and its "Let me
//! decide for each file" list, where keeping both versions adds a number
//! to the copied file's name.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::{gio, prelude::*};

use crate::dialogs::{self, Dialog};
use crate::glyph::Glyph;
use crate::icons;
use crate::util;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    Replace,
    Skip,
    KeepBoth,
}

pub struct Conflict {
    pub source: gio::File,
    pub dest: gio::File,
}

/// Asks once for all conflicts. `None` means cancel the whole operation.
pub async fn resolve(
    parent: Option<&gtk::Window>,
    verb: &str,
    item_count: u64,
    from: &str,
    to: &str,
    conflicts: &[Conflict],
) -> Option<Vec<Decision>> {
    let count = conflicts.len();
    let dialog = Dialog::new(parent, "Replace or Skip Files");
    dialog.window.set_default_size(460, -1);
    let what = if item_count == 1 {
        "1 item".to_owned()
    } else {
        format!("{} items", util::thousands(item_count))
    };
    let intro = gtk::Label::builder()
        .label(format!("{verb} {what} from {from} to {to}"))
        .xalign(0.0)
        .wrap(true)
        .width_chars(36)
        .build();
    intro.add_css_class("dialog-detail");
    dialog.body.append(&intro);
    let heading = if count == 1 {
        let name = conflicts[0]
            .dest
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        format!("The destination already has a file named \"{name}\"")
    } else {
        format!("The destination has {count} files with the same names")
    };
    let heading = gtk::Label::builder()
        .label(heading)
        .xalign(0.0)
        .wrap(true)
        .width_chars(36)
        .max_width_chars(48)
        .build();
    heading.add_css_class("dialog-heading");
    dialog.body.append(&heading);

    const REPLACE: i32 = 1;
    const SKIP: i32 = 2;
    const DECIDE: i32 = 3;
    let choices: [(i32, &str, String); 3] = [
        (
            REPLACE,
            "check",
            if count == 1 {
                "Replace the file in the destination".into()
            } else {
                "Replace the files in the destination".into()
            },
        ),
        (
            SKIP,
            "redo",
            if count == 1 {
                "Skip this file".into()
            } else {
                "Skip these files".into()
            },
        ),
        (
            DECIDE,
            "copy",
            if count == 1 {
                "Compare info for both files".into()
            } else {
                "Let me decide for each file".into()
            },
        ),
    ];
    for (response, glyph, label) in choices {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.append(&Glyph::sized(glyph, 20));
        let text = gtk::Label::builder().label(&label).xalign(0.0).build();
        row.append(&text);
        let button = gtk::Button::builder().child(&row).build();
        button.add_css_class("flat");
        button.add_css_class("choice-button");
        let weak = Rc::downgrade(&dialog);
        button.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.respond(response);
            }
        });
        dialog.body.append(&button);
    }
    dialog.add_button("Cancel", dialogs::CLOSED, false);
    match dialog.run().await {
        REPLACE => Some(vec![Decision::Replace; count]),
        SKIP => Some(vec![Decision::Skip; count]),
        DECIDE => decide_each(parent, from, to, conflicts).await,
        _ => None,
    }
}

/// "N File Conflicts": a check box for each version of each file.
async fn decide_each(
    parent: Option<&gtk::Window>,
    from: &str,
    to: &str,
    conflicts: &[Conflict],
) -> Option<Vec<Decision>> {
    let title = if conflicts.len() == 1 {
        "1 File Conflict".to_owned()
    } else {
        format!("{} File Conflicts", conflicts.len())
    };
    let dialog = Dialog::new(parent, &title);
    dialog.window.set_default_size(640, 480);
    dialog.window.set_resizable(true);
    let heading = gtk::Label::builder()
        .label("Which files do you want to keep?")
        .xalign(0.0)
        .build();
    heading.add_css_class("dialog-heading");
    dialog.body.append(&heading);
    let note = gtk::Label::builder()
        .label("If you select both versions, the copied file will have a number added to its name.")
        .xalign(0.0)
        .wrap(true)
        .width_chars(40)
        .build();
    note.add_css_class("dialog-detail");
    dialog.body.append(&note);

    let grid = gtk::Grid::builder()
        .column_spacing(24)
        .row_spacing(6)
        .column_homogeneous(true)
        .build();
    let all_sources = gtk::CheckButton::with_label(&format!("Files from {from}"));
    let all_dests = gtk::CheckButton::with_label(&format!("Files already in {to}"));
    all_sources.add_css_class("conflict-column");
    all_dests.add_css_class("conflict-column");
    grid.attach(&all_sources, 0, 0, 1, 1);
    grid.attach(&all_dests, 1, 0, 1, 1);

    let checks: Rc<RefCell<Vec<(gtk::CheckButton, gtk::CheckButton)>>> = Rc::default();
    for (index, conflict) in conflicts.iter().enumerate() {
        let row = (index as i32) * 2 + 1;
        let name = conflict
            .dest
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let label = gtk::Label::builder()
            .label(&name)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .build();
        label.add_css_class("conflict-name");
        grid.attach(&label, 0, row, 2, 1);
        let source = version(&conflict.source).await;
        let dest = version(&conflict.dest).await;
        grid.attach(&source.0, 0, row + 1, 1, 1);
        grid.attach(&dest.0, 1, row + 1, 1, 1);
        checks.borrow_mut().push((source.1, dest.1));
    }
    {
        let checks = checks.clone();
        all_sources.connect_toggled(move |all| {
            for (source, _) in checks.borrow().iter() {
                source.set_active(all.is_active());
            }
        });
    }
    {
        let checks = checks.clone();
        all_dests.connect_toggled(move |all| {
            for (_, dest) in checks.borrow().iter() {
                dest.set_active(all.is_active());
            }
        });
    }
    let scroller = gtk::ScrolledWindow::builder()
        .child(&grid)
        .vexpand(true)
        .min_content_height(240)
        .build();
    dialog.body.append(&scroller);
    dialog.add_button("_Continue", 1, true);
    dialog.add_button("Cancel", dialogs::CLOSED, false);
    if dialog.run().await != 1 {
        return None;
    }
    let decisions = checks
        .borrow()
        .iter()
        .map(
            |(source, dest)| match (source.is_active(), dest.is_active()) {
                (true, true) => Decision::KeepBoth,
                (true, false) => Decision::Replace,
                _ => Decision::Skip,
            },
        )
        .collect();
    Some(decisions)
}

/// One version of a file: a check box with its icon, date and size.
async fn version(file: &gio::File) -> (gtk::Box, gtk::CheckButton) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let check = gtk::CheckButton::new();
    row.append(&check);
    let info = file
        .query_info_future(
            crate::folderview::ATTRIBUTES,
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gtk::glib::Priority::DEFAULT,
        )
        .await
        .ok();
    let icon = icons::image(info.as_ref().map(icons::for_info).unwrap_or("pfm-file"), 48);
    if let Some(info) = &info {
        info.set_attribute_object("standard::file", file);
        let image = icon.clone();
        crate::thumbs::request(info, 48 * icon.scale_factor().max(1), move |texture| {
            if let Some(texture) = texture {
                image.set_paintable(Some(&texture));
                image.set_pixel_size(48);
            }
        });
    }
    row.append(&icon);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let date = info
        .as_ref()
        .and_then(util::modified)
        .map(|time| util::date_time(&time))
        .unwrap_or_default();
    let size = info
        .as_ref()
        .map(|info| util::size(info.size().max(0) as u64))
        .unwrap_or_default();
    for line in [date, size] {
        let label = gtk::Label::builder().label(&line).xalign(0.0).build();
        label.add_css_class("dialog-detail");
        text.append(&label);
    }
    row.append(&text);
    (row, check)
}
