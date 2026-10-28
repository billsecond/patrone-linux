//! This PC: every drive as a tile with a bar showing how full it is and
//! where it's mounted, and network locations. Drives that aren't mounted
//! yet mount when opened; mapped network drives connect.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk::{gdk, gio, glib, prelude::*};

use crate::config;
use crate::drives;
use crate::folderview::ViewEvent;
use crate::home;
use crate::icons;
use crate::places;
use crate::util;

/// One tile: a mounted drive, a volume that can be mounted, or a mapped
/// network drive that isn't connected.
#[derive(Clone)]
pub enum Drive {
    Root,
    Mount(gio::Mount),
    Volume(gio::Volume),
    Mapped(String),
}

impl Drive {
    pub fn name(&self) -> String {
        match self {
            Drive::Root => places::root_name(),
            Drive::Mount(mount) => mount.name().into(),
            Drive::Volume(volume) => volume.name().into(),
            Drive::Mapped(uri) => drives::network_name(uri),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Drive::Root => "pfm-drive-system",
            Drive::Mount(mount) => icons::drive_icon(mount),
            Drive::Volume(volume) => {
                if volume.can_eject() {
                    "pfm-drive-usb"
                } else {
                    "pfm-drive"
                }
            }
            Drive::Mapped(_) => "pfm-drive-network",
        }
    }

    pub fn root(&self) -> Option<gio::File> {
        match self {
            Drive::Root => Some(places::root()),
            Drive::Mount(mount) => Some(mount.root()),
            Drive::Volume(volume) => volume.get_mount().map(|mount| mount.root()),
            Drive::Mapped(_) => None,
        }
    }

    /// The drive's mount: the folder its files are in, or its network
    /// address. None when it isn't mounted.
    pub fn mount_point(&self) -> Option<String> {
        match self {
            Drive::Mapped(_) => None,
            drive => drive.root().map(|root| drives::mount_point(&root)),
        }
    }

    /// A drive that can be mounted (or a mapped drive connected) now.
    pub fn can_mount(&self) -> bool {
        match self {
            Drive::Volume(volume) => volume.get_mount().is_none() && volume.can_mount(),
            Drive::Mapped(uri) => !drives::is_connected(uri),
            _ => false,
        }
    }

    /// A mounted drive that can be ejected or unmounted (not the system
    /// drive, and not a network drive: those disconnect).
    pub fn mount(&self) -> Option<gio::Mount> {
        match self {
            Drive::Mount(mount) => Some(mount.clone()),
            Drive::Volume(volume) => volume.get_mount(),
            _ => None,
        }
    }

    pub fn is_network(&self) -> bool {
        match self {
            Drive::Mapped(_) => true,
            Drive::Mount(mount) => !mount.root().is_native(),
            _ => false,
        }
    }
}

/// What the unmount command is called for these mounts: "Eject" when they
/// can be removed (USB drives, discs, disk images), else "Unmount".
pub fn unmount_label(mounts: &[gio::Mount]) -> &'static str {
    if !mounts.is_empty() && mounts.iter().all(|mount| mount.can_eject()) {
        "Eject"
    } else {
        "Unmount"
    }
}

/// Every drive This PC shows, the system drive first.
pub fn drives() -> Vec<Drive> {
    let monitor = gio::VolumeMonitor::get();
    let mut drives = vec![Drive::Root];
    for mount in places::mounts() {
        if mount.root().is_native() {
            drives.push(Drive::Mount(mount));
        }
    }
    let hide_empty = config::flag("hide-empty-drives");
    for volume in monitor.volumes() {
        if volume.get_mount().is_some() || !volume.can_mount() {
            continue;
        }
        if hide_empty
            && volume
                .drive()
                .is_some_and(|drive| drive.is_media_removable() && !drive.has_media())
        {
            continue;
        }
        drives.push(Drive::Volume(volume));
    }
    drives
}

/// Network shares that are mounted, then mapped network drives that
/// aren't connected now.
pub fn network_drives() -> Vec<Drive> {
    let mounts: Vec<gio::Mount> = places::mounts()
        .into_iter()
        .filter(|mount| !mount.root().is_native())
        .collect();
    let mut drives: Vec<Drive> = mounts.iter().cloned().map(Drive::Mount).collect();
    for uri in drives::mapped_drives() {
        let file = gio::File::for_uri(&uri);
        let connected = mounts.iter().any(|mount| {
            let root = mount.root();
            root.equal(&file) || file.has_prefix(&root)
        });
        if !connected {
            drives.push(Drive::Mapped(uri));
        }
    }
    drives
}

fn drive_tile(drive: &Drive) -> (gtk::FlowBoxChild, gtk::LevelBar, gtk::Label) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("drive-tile");
    row.append(&icons::image(drive.icon(), 48));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.set_valign(gtk::Align::Center);
    text.set_hexpand(true);
    let name = gtk::Label::builder()
        .label(drive.name())
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    text.append(&name);
    let bar = gtk::LevelBar::builder()
        .min_value(0.0)
        .max_value(1.0)
        .build();
    bar.add_offset_value("high", 0.9);
    bar.add_css_class("drive-usage");
    bar.set_visible(false);
    text.append(&bar);
    let free = gtk::Label::builder().xalign(0.0).build();
    free.add_css_class("item-detail");
    match drive {
        Drive::Volume(volume) => free.set_label(&match drives::device_size(volume) {
            Some(size) => format!("{} · Not mounted", util::size(size)),
            None => "Not mounted".to_owned(),
        }),
        Drive::Mapped(_) => free.set_label("Disconnected"),
        _ => {}
    }
    text.append(&free);
    // Where its files are: Linux's drive letter.
    let place = match (drive, drive.mount_point()) {
        (Drive::Mapped(uri), _) => Some(uri.clone()),
        (_, Some(point)) if drive.is_network() => Some(point),
        (_, Some(point)) => Some(format!("Mounted at {point}")),
        (_, None) => None,
    };
    if let Some(place) = place {
        let label = gtk::Label::builder()
            .label(&place)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .tooltip_text(&place)
            .build();
        label.add_css_class("item-detail");
        text.append(&label);
    }
    row.append(&text);
    let child = gtk::FlowBoxChild::builder().child(&row).build();
    child.set_size_request(256, -1);
    (child, bar, free)
}

/// Fills in a drive's usage bar and "X GB free of Y GB".
pub fn show_usage(root: gio::File, bar: gtk::LevelBar, free: gtk::Label) {
    glib::spawn_future_local(async move {
        let Ok(info) = root
            .query_filesystem_info_future(
                "filesystem::size,filesystem::free",
                glib::Priority::DEFAULT,
            )
            .await
        else {
            return;
        };
        let size = info.attribute_uint64("filesystem::size");
        let available = info.attribute_uint64("filesystem::free");
        if size == 0 {
            return;
        }
        bar.set_value(1.0 - available as f64 / size as f64);
        bar.set_visible(true);
        free.set_label(&format!(
            "{} free of {}",
            util::size(available),
            util::size(size)
        ));
    });
}

pub struct ThisPcPage {
    pub widget: gtk::ScrolledWindow,
    drives_box: gtk::FlowBox,
    network_box: gtk::FlowBox,
    network_section: gtk::Box,
    drives: RefCell<Vec<Drive>>,
    network: RefCell<Vec<Drive>>,
    events: crate::util::Handler<ViewEvent>,
    me: RefCell<Weak<ThisPcPage>>,
}

impl ThisPcPage {
    pub fn new() -> Rc<ThisPcPage> {
        let flow = || {
            let flow = gtk::FlowBox::builder()
                .selection_mode(gtk::SelectionMode::Multiple)
                .activate_on_single_click(config::flag("single-click"))
                .homogeneous(true)
                .max_children_per_line(6)
                .column_spacing(4)
                .row_spacing(4)
                .build();
            flow.add_css_class("drive-tiles");
            flow
        };
        let drives_box = flow();
        let network_box = flow();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
        content.add_css_class("home-page");
        let (drives_section, heading) = home::section("Devices and drives", &drives_box);
        heading.set_tooltip_text(Some(
            "Every drive and where it's mounted. Linux has no drive letters: \
             each drive's files appear in a folder, its mount. The system drive \
             is mounted at /, others under /media. Open a drive that isn't \
             mounted to mount it.",
        ));
        content.append(&drives_section);
        let (network_section, _) = home::section("Network locations", &network_box);
        content.append(&network_section);
        let widget = gtk::ScrolledWindow::builder()
            .child(&content)
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .build();
        widget.add_css_class("file-list");
        let page = Rc::new(ThisPcPage {
            widget,
            drives_box,
            network_box,
            network_section,
            drives: RefCell::default(),
            network: RefCell::default(),
            events: RefCell::default(),
            me: RefCell::default(),
        });
        *page.me.borrow_mut() = Rc::downgrade(&page);
        page.connect_signals();
        page.reload();
        page
    }

    pub fn connect_events(&self, events: impl Fn(ViewEvent) + 'static) {
        *self.events.borrow_mut() = Some(Rc::new(events));
    }

    fn emit(&self, event: ViewEvent) {
        let events = self.events.borrow().clone();
        if let Some(events) = events {
            events(event);
        }
    }

    fn connect_signals(&self) {
        let weak = self.me.borrow().clone();
        self.drives_box.connect_child_activated(move |_, child| {
            let Some(page) = weak.upgrade() else { return };
            let drive = page.drives.borrow().get(child.index() as usize).cloned();
            if let Some(drive) = drive {
                page.open_drive(drive);
            }
        });
        let weak = self.me.borrow().clone();
        self.network_box.connect_child_activated(move |_, child| {
            let Some(page) = weak.upgrade() else { return };
            let drive = page.network.borrow().get(child.index() as usize).cloned();
            if let Some(drive) = drive {
                page.open_drive(drive);
            }
        });
        for (flow, other) in [
            (&self.drives_box, &self.network_box),
            (&self.network_box, &self.drives_box),
        ] {
            let weak = self.me.borrow().clone();
            let other = other.clone();
            flow.connect_selected_children_changed(move |flow| {
                let Some(page) = weak.upgrade() else { return };
                if !flow.selected_children().is_empty() {
                    other.unselect_all();
                }
                page.emit(ViewEvent::SelectionChanged);
            });
        }
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        let weak = self.me.borrow().clone();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(page) = weak.upgrade() else { return };
            let Some(widget) = gesture.widget() else {
                return;
            };
            gesture.set_state(gtk::EventSequenceState::Claimed);
            let on_item = page.select_at(&widget, x, y);
            page.emit(ViewEvent::ContextMenu {
                on_item,
                widget,
                x,
                y,
            });
        });
        self.widget.add_controller(click);

        let monitor = gio::VolumeMonitor::get();
        let reload = {
            let weak = self.me.borrow().clone();
            move || {
                if let Some(page) = weak.upgrade() {
                    page.reload();
                }
            }
        };
        {
            let reload = reload.clone();
            monitor.connect_mount_added(move |_, _| reload());
        }
        {
            let reload = reload.clone();
            monitor.connect_mount_removed(move |_, _| reload());
        }
        {
            let reload = reload.clone();
            monitor.connect_volume_added(move |_, _| reload());
        }
        {
            let reload = reload.clone();
            monitor.connect_volume_removed(move |_, _| reload());
        }
        config::settings().connect_changed(Some("mapped-drives"), move |_, _| reload());
    }

    fn select_at(&self, widget: &gtk::Widget, x: f64, y: f64) -> bool {
        let mut current = widget.pick(x, y, gtk::PickFlags::DEFAULT);
        while let Some(candidate) = current {
            if let Some(child) = candidate.downcast_ref::<gtk::FlowBoxChild>() {
                if let Some(flow) = child.parent().and_downcast::<gtk::FlowBox>()
                    && !child.is_selected()
                {
                    flow.unselect_all();
                    flow.select_child(child);
                }
                return true;
            }
            current = candidate.parent();
        }
        self.select_none();
        false
    }

    fn emit_open(&self, root: &gio::File) {
        let weak = self.me.borrow().clone();
        let root = root.clone();
        glib::spawn_future_local(async move {
            if let (Some(page), Some(info)) = (weak.upgrade(), home::info_for(&root).await) {
                page.emit(ViewEvent::Activate(vec![info]));
            }
        });
    }

    /// Opens a drive, mounting (or connecting) it first if it isn't.
    pub fn open_drive(&self, drive: Drive) {
        if drive.can_mount() {
            let weak = self.me.borrow().clone();
            let window = self.widget.root().and_downcast::<gtk::Window>();
            mount_drive(&drive, window, move |root| {
                if let Some(page) = weak.upgrade() {
                    page.emit_open(&root);
                }
            });
        } else if let Some(root) = drive.root() {
            self.emit_open(&root);
        }
    }

    pub fn reload(&self) {
        self.drives_box.remove_all();
        let drives = drives();
        for drive in &drives {
            let (tile, bar, free) = drive_tile(drive);
            self.drives_box.append(&tile);
            if let Some(root) = drive.root() {
                show_usage(root, bar, free);
            }
        }
        self.drives.replace(drives);
        self.network_box.remove_all();
        let network = network_drives();
        for drive in &network {
            let (tile, bar, free) = drive_tile(drive);
            self.network_box.append(&tile);
            if let Some(root) = drive.root() {
                show_usage(root, bar, free);
            }
        }
        self.network_section.set_visible(!network.is_empty());
        self.network.replace(network);
        self.emit(ViewEvent::ItemsChanged);
    }

    /// The selected drives, as files (their roots).
    pub fn selected_infos(&self) -> Vec<gio::FileInfo> {
        let mut infos = Vec::new();
        for child in self.drives_box.selected_children() {
            if let Some(drive) = self.drives.borrow().get(child.index() as usize)
                && let Some(root) = drive.root()
            {
                infos.push(drive_info(&root, &drive.name()));
            }
        }
        for child in self.network_box.selected_children() {
            if let Some(drive) = self.network.borrow().get(child.index() as usize)
                && let Some(root) = drive.root()
            {
                infos.push(drive_info(&root, &drive.name()));
            }
        }
        infos
    }

    /// The selected drive tiles themselves, for Mount, Eject and
    /// Properties.
    pub fn selected_drives(&self) -> Vec<Drive> {
        let mut drives = Vec::new();
        for child in self.drives_box.selected_children() {
            if let Some(drive) = self.drives.borrow().get(child.index() as usize) {
                drives.push(drive.clone());
            }
        }
        for child in self.network_box.selected_children() {
            if let Some(drive) = self.network.borrow().get(child.index() as usize) {
                drives.push(drive.clone());
            }
        }
        drives
    }

    pub fn n_items(&self) -> u32 {
        (self.drives.borrow().len() + self.network.borrow().len()) as u32
    }

    pub fn select_all(&self) {
        self.drives_box.select_all();
    }

    pub fn select_none(&self) {
        self.drives_box.unselect_all();
        self.network_box.unselect_all();
    }

    pub fn focus(&self) {
        self.drives_box.grab_focus();
    }
}

/// Mounts a drive, or connects a mapped network drive, then gives its root
/// folder. A failure is shown to the user.
pub fn mount_drive(
    drive: &Drive,
    parent: Option<gtk::Window>,
    done: impl FnOnce(gio::File) + 'static,
) {
    let name = drive.name();
    let failed = {
        let parent = parent.clone();
        move |detail: String| {
            crate::dialogs::error(
                parent.as_ref(),
                "Mount",
                &format!("Can't mount {name}."),
                &detail,
            );
        }
    };
    match drive {
        Drive::Volume(volume) => {
            drives::mount_volume(volume, parent.as_ref(), move |result| match result {
                Ok(root) => done(root),
                // Cancelled at the password prompt.
                Err(error) if error.matches(gio::IOErrorEnum::FailedHandled) => {}
                Err(error) => failed(crate::dialogs::describe(&error)),
            });
        }
        Drive::Mapped(uri) => {
            let uri = uri.clone();
            glib::spawn_future_local(async move {
                match drives::connect(&uri, parent.as_ref()).await {
                    Ok(root) => done(root),
                    Err(error) if error.matches(gio::IOErrorEnum::FailedHandled) => {}
                    Err(error) => failed(crate::dialogs::describe(&error)),
                }
            });
        }
        drive => {
            if let Some(root) = drive.root() {
                done(root);
            }
        }
    }
}

/// A file info for a drive's root, as the lists use.
pub fn drive_info(root: &gio::File, name: &str) -> gio::FileInfo {
    let info = gio::FileInfo::new();
    info.set_name(name);
    info.set_display_name(name);
    info.set_file_type(gio::FileType::Directory);
    info.set_attribute_object("standard::file", root);
    info.set_attribute_boolean("pfm::drive", true);
    info
}
