//! The navigation pane, laid out as in Windows 11: Home, Gallery and the
//! home folder; then the folders pinned to Quick access; then This PC (its
//! drives inside), Network and the Recycle Bin. Folders expand to show
//! their subfolders; chevrons appear when the pointer is over the pane.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gtk::{gdk, gio, glib, prelude::*};

use crate::config;
use crate::folderview::ATTRIBUTES;
use crate::glyph::Glyph;
use crate::icons;
use crate::location::Location;
use crate::places;
use crate::thispc::{self, Drive};
use crate::util;

#[derive(Clone)]
pub enum NodeKind {
    Home,
    Gallery,
    Folder(gio::File),
    ThisPc,
    Drive(Drive),
    Network,
    Trash,
    Separator,
}

impl NodeKind {
    /// Where activating this row goes (a volume mounts first).
    pub fn location(&self) -> Option<Location> {
        Some(match self {
            NodeKind::Home => Location::Home,
            NodeKind::Gallery => Location::Gallery,
            NodeKind::Folder(file) => Location::Folder(file.clone()),
            NodeKind::ThisPc => Location::ThisPc,
            NodeKind::Drive(drive) => Location::Folder(drive.root()?),
            NodeKind::Network => Location::Folder(places::network()),
            NodeKind::Trash => Location::Folder(places::trash()),
            NodeKind::Separator => return None,
        })
    }

    pub fn file(&self) -> Option<gio::File> {
        match self.location()? {
            Location::Folder(file) => Some(file),
            _ => None,
        }
    }
}

struct Node {
    kind: NodeKind,
    depth: i32,
    row: gtk::ListBoxRow,
    expander: Option<Glyph>,
    expanded: Cell<bool>,
    pinned: bool,
}

pub enum NavEvent {
    Navigate(Location),
    OpenInNewTab(Location),
    OpenVolume(Drive),
    ContextMenu {
        kind: NodeKind,
        pinned: bool,
        widget: gtk::Widget,
        x: f64,
        y: f64,
    },
    Drop {
        files: Vec<gio::File>,
        folder: gio::File,
        action: gdk::DragAction,
    },
}

pub struct NavPane {
    pub widget: gtk::ScrolledWindow,
    list: gtk::ListBox,
    nodes: RefCell<Vec<Rc<Node>>>,
    syncing: Cell<bool>,
    current: RefCell<Option<Location>>,
    trash_monitor: RefCell<Option<gio::FileMonitor>>,
    events: crate::util::Handler<NavEvent>,
    me: RefCell<Weak<NavPane>>,
}

impl NavPane {
    pub fn new() -> Rc<NavPane> {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .activate_on_single_click(true)
            .build();
        list.add_css_class("navigation-pane");
        let widget = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        widget.add_css_class("navigation-scroller");
        let pane = Rc::new(NavPane {
            widget,
            list,
            nodes: RefCell::default(),
            syncing: Cell::new(false),
            current: RefCell::default(),
            trash_monitor: RefCell::default(),
            events: RefCell::default(),
            me: RefCell::default(),
        });
        *pane.me.borrow_mut() = Rc::downgrade(&pane);
        pane.connect_signals();
        pane.rebuild();
        pane
    }

    pub fn connect_events(&self, events: impl Fn(NavEvent) + 'static) {
        *self.events.borrow_mut() = Some(Rc::new(events));
    }

    fn emit(&self, event: NavEvent) {
        let events = self.events.borrow().clone();
        if let Some(events) = events {
            events(event);
        }
    }

    fn connect_signals(&self) {
        let weak = self.me.borrow().clone();
        self.list.connect_row_activated(move |_, row| {
            let Some(pane) = weak.upgrade() else { return };
            let Some(node) = pane.node_for_row(row) else {
                return;
            };
            match &node.kind {
                NodeKind::Drive(drive @ Drive::Volume(volume)) if volume.get_mount().is_none() => {
                    pane.emit(NavEvent::OpenVolume(drive.clone()));
                }
                kind => {
                    if let Some(location) = kind.location() {
                        pane.emit(NavEvent::Navigate(location));
                    }
                }
            }
        });

        // Middle-click opens in a new tab; right-click opens a menu.
        let click = gtk::GestureClick::new();
        click.set_button(0);
        let weak = self.me.borrow().clone();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(pane) = weak.upgrade() else { return };
            let button = gesture.current_button();
            if button != gdk::BUTTON_MIDDLE && button != gdk::BUTTON_SECONDARY {
                return;
            }
            let Some(row) = pane.list.row_at_y(y as i32) else {
                return;
            };
            let Some(node) = pane.node_for_row(&row) else {
                return;
            };
            gesture.set_state(gtk::EventSequenceState::Claimed);
            if button == gdk::BUTTON_MIDDLE {
                if let Some(location) = node.kind.location() {
                    pane.emit(NavEvent::OpenInNewTab(location));
                }
            } else if !matches!(node.kind, NodeKind::Separator) {
                pane.emit(NavEvent::ContextMenu {
                    kind: node.kind.clone(),
                    pinned: node.pinned,
                    widget: pane.list.clone().upcast(),
                    x,
                    y,
                });
            }
        });
        self.list.add_controller(click);

        let weak = self.me.borrow().clone();
        config::settings().connect_changed(None, move |_, key| {
            if matches!(key, "pinned-folders" | "show-hidden" | "hide-empty-drives")
                && let Some(pane) = weak.upgrade()
            {
                pane.rebuild();
            }
        });

        let monitor = gio::VolumeMonitor::get();
        let rebuild = {
            let weak = self.me.borrow().clone();
            move || {
                if let Some(pane) = weak.upgrade() {
                    pane.refresh_drives();
                }
            }
        };
        {
            let rebuild = rebuild.clone();
            monitor.connect_mount_added(move |_, _| rebuild());
        }
        {
            let rebuild = rebuild.clone();
            monitor.connect_mount_removed(move |_, _| rebuild());
        }
        {
            let rebuild = rebuild.clone();
            monitor.connect_volume_added(move |_, _| rebuild());
        }
        monitor.connect_volume_removed(move |_, _| rebuild());

        // The Recycle Bin's icon shows whether it has anything in it.
        if let Ok(monitor) =
            places::trash().monitor_directory(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
        {
            let weak = self.me.borrow().clone();
            monitor.connect_changed(move |_, _, _, _| {
                if let Some(pane) = weak.upgrade() {
                    pane.update_trash_icon();
                }
            });
            self.trash_monitor.replace(Some(monitor));
        }
    }

    fn node_for_row(&self, row: &gtk::ListBoxRow) -> Option<Rc<Node>> {
        self.nodes
            .borrow()
            .iter()
            .find(|node| &node.row == row)
            .cloned()
    }

    /// Builds the tree's top level again.
    pub fn rebuild(&self) {
        self.list.remove_all();
        self.nodes.borrow_mut().clear();
        let mut top: Vec<(NodeKind, bool, bool)> = vec![
            (NodeKind::Home, false, false),
            (NodeKind::Gallery, false, false),
            (NodeKind::Folder(places::home()), true, false),
            (NodeKind::Separator, false, false),
        ];
        for folder in config::pinned() {
            top.push((NodeKind::Folder(folder), true, true));
        }
        top.push((NodeKind::Separator, false, false));
        top.push((NodeKind::ThisPc, true, false));
        top.push((NodeKind::Network, true, false));
        top.push((NodeKind::Trash, false, false));
        let mut nodes = Vec::new();
        for (kind, expandable, pinned) in top {
            let node = self.make_node(kind, 0, expandable, pinned);
            self.list.append(&node.row);
            nodes.push(node);
        }
        self.nodes.replace(nodes);
        let current = self.current.borrow().clone();
        if let Some(location) = current {
            self.select_location(&location);
        }
    }

    fn make_node(&self, kind: NodeKind, depth: i32, expandable: bool, pinned: bool) -> Rc<Node> {
        if matches!(kind, NodeKind::Separator) {
            let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
            separator.set_valign(gtk::Align::Center);
            let row = gtk::ListBoxRow::builder()
                .child(&separator)
                .selectable(false)
                .activatable(false)
                .build();
            row.add_css_class("nav-separator");
            return Rc::new(Node {
                kind,
                depth,
                row,
                expander: None,
                expanded: Cell::new(false),
                pinned,
            });
        }
        let (label, icon) = match &kind {
            NodeKind::Home => ("Home".to_owned(), "pfm-nav-home"),
            NodeKind::Gallery => ("Gallery".to_owned(), "pfm-nav-gallery"),
            NodeKind::Folder(file) => (places::display_name(file), icons::small_folder_icon(file)),
            NodeKind::ThisPc => ("This PC".to_owned(), "pfm-nav-computer"),
            NodeKind::Drive(drive) => (drive.name(), small_icon_for(drive)),
            NodeKind::Network => ("Network".to_owned(), "pfm-nav-network"),
            NodeKind::Trash => (
                "Recycle Bin".to_owned(),
                icons::small_folder_icon(&places::trash()),
            ),
            NodeKind::Separator => unreachable!(),
        };
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        content.add_css_class("nav-row");
        content.set_margin_start(depth * 16);
        let expander = Glyph::sized("chevron-right", 12);
        expander.add_css_class("nav-expander");
        let expander_button = gtk::Button::builder().child(&expander).build();
        expander_button.add_css_class("flat");
        expander_button.add_css_class("nav-expander-button");
        expander_button.set_can_focus(false);
        expander_button.set_opacity(if expandable { 1.0 } else { 0.0 });
        expander_button.set_sensitive(expandable);
        content.append(&expander_button);
        content.append(&icons::image(icon, 16));
        let name = gtk::Label::builder()
            .label(&label)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        content.append(&name);
        if pinned {
            let pin = Glyph::sized("pin", 12);
            pin.add_css_class("nav-pin");
            content.append(&pin);
        }
        let row = gtk::ListBoxRow::builder().child(&content).build();
        row.set_tooltip_text(
            match &kind {
                NodeKind::Folder(file) => Some(util::display_path(file)),
                _ => None,
            }
            .as_deref(),
        );
        let node = Rc::new(Node {
            kind,
            depth,
            row,
            expander: expandable.then_some(expander),
            expanded: Cell::new(false),
            pinned,
        });
        if expandable {
            let weak = self.me.borrow().clone();
            let weak_node = Rc::downgrade(&node);
            expander_button.connect_clicked(move |_| {
                if let (Some(pane), Some(node)) = (weak.upgrade(), weak_node.upgrade()) {
                    pane.toggle(&node);
                }
            });
        }
        self.add_drop_target(&node);
        node
    }

    fn add_drop_target(&self, node: &Rc<Node>) {
        let Some(folder) = node.kind.file() else {
            return;
        };
        if matches!(node.kind, NodeKind::Network) {
            return;
        }
        let target = gtk::DropTarget::new(
            gdk::FileList::static_type(),
            gdk::DragAction::COPY | gdk::DragAction::MOVE,
        );
        target.set_preload(true);
        {
            let folder = folder.clone();
            target
                .connect_motion(move |target, _, _| crate::dnd::preferred_action(target, &folder));
        }
        let weak = self.me.borrow().clone();
        target.connect_drop(move |target, value, _, _| {
            let Some(pane) = weak.upgrade() else {
                return false;
            };
            let Ok(list) = value.get::<gdk::FileList>() else {
                return false;
            };
            let files = list.files();
            if files.iter().any(|file| file.equal(&folder)) {
                return false;
            }
            let action = crate::dnd::drop_action(target, &folder);
            pane.emit(NavEvent::Drop {
                files,
                folder: folder.clone(),
                action,
            });
            true
        });
        node.row.add_controller(target);
    }

    fn toggle(&self, node: &Rc<Node>) {
        if node.expanded.get() {
            self.collapse(node);
        } else {
            self.expand(node, None);
        }
    }

    fn collapse(&self, node: &Rc<Node>) {
        node.expanded.set(false);
        if let Some(expander) = &node.expander {
            expander.set_name("chevron-right");
        }
        let mut nodes = self.nodes.borrow_mut();
        let Some(index) = nodes.iter().position(|n| Rc::ptr_eq(n, node)) else {
            return;
        };
        let end = nodes[index + 1..]
            .iter()
            .position(|n| n.depth <= node.depth)
            .map(|offset| index + 1 + offset)
            .unwrap_or(nodes.len());
        for removed in nodes.drain(index + 1..end) {
            self.list.remove(&removed.row);
        }
    }

    /// Shows a node's children; `then` runs once they're there.
    fn expand(&self, node: &Rc<Node>, then: Option<Box<dyn FnOnce()>>) {
        if node.expanded.get() {
            if let Some(then) = then {
                then();
            }
            return;
        }
        node.expanded.set(true);
        if let Some(expander) = &node.expander {
            expander.set_name("chevron-down");
        }
        let weak = self.me.borrow().clone();
        let node = node.clone();
        glib::spawn_future_local(async move {
            let children = children_of(&node.kind).await;
            let Some(pane) = weak.upgrade() else { return };
            if !node.expanded.get() {
                return;
            }
            let Some(index) = pane
                .nodes
                .borrow()
                .iter()
                .position(|n| Rc::ptr_eq(n, &node))
            else {
                return;
            };
            let mut position = index + 1;
            for (kind, expandable) in children {
                let child = pane.make_node(kind, node.depth + 1, expandable, false);
                pane.list.insert(&child.row, position as i32);
                pane.nodes.borrow_mut().insert(position, child);
                position += 1;
            }
            if node.expander.is_some() && position == index + 1 {
                // Nothing inside: no chevron.
                if let Some(expander) = &node.expander {
                    expander.set_opacity(0.0);
                }
            }
            let current = pane.current.borrow().clone();
            if let Some(location) = current {
                pane.select_location(&location);
            }
            if let Some(then) = then {
                then();
            }
        });
    }

    fn refresh_drives(&self) {
        let this_pc = self
            .nodes
            .borrow()
            .iter()
            .find(|n| matches!(n.kind, NodeKind::ThisPc))
            .cloned();
        if let Some(node) = this_pc
            && node.expanded.get()
        {
            self.collapse(&node);
            self.expand(&node, None);
        }
    }

    fn update_trash_icon(&self) {
        let trash = self
            .nodes
            .borrow()
            .iter()
            .find(|n| matches!(n.kind, NodeKind::Trash))
            .cloned();
        let Some(node) = trash else { return };
        let image = node
            .row
            .child()
            .and_then(|content| content.first_child())
            .and_then(|expander| expander.next_sibling())
            .and_downcast::<gtk::Image>();
        if let Some(image) = image {
            icons::set_image(&image, icons::small_folder_icon(&places::trash()), 16);
        }
    }

    /// Highlights the row for where the window is.
    pub fn select_location(&self, location: &Location) {
        self.current.replace(Some(location.clone()));
        let found = self
            .nodes
            .borrow()
            .iter()
            .find(|node| node_matches(&node.kind, location))
            .cloned();
        self.syncing.set(true);
        match found {
            Some(node) => self.list.select_row(Some(&node.row)),
            None => self.list.unselect_all(),
        }
        self.syncing.set(false);
        if config::flag("nav-expand-to-folder")
            && let Location::Folder(file) = location
        {
            self.reveal(file);
        }
    }

    /// Expands the tree down to `file` (Ctrl+Shift+E).
    pub fn reveal(&self, file: &gio::File) {
        // The nearest shown ancestor, expanded step by step.
        let ancestor = self
            .nodes
            .borrow()
            .iter()
            .filter(|node| {
                node.kind
                    .file()
                    .is_some_and(|folder| file.has_prefix(&folder))
                    && node.expander.is_some()
            })
            .max_by_key(|node| node.depth)
            .cloned();
        let Some(ancestor) = ancestor else { return };
        let weak = self.me.borrow().clone();
        let file = file.clone();
        self.expand(
            &ancestor,
            Some(Box::new(move || {
                let Some(pane) = weak.upgrade() else { return };
                let found = pane
                    .nodes
                    .borrow()
                    .iter()
                    .any(|node| node.kind.file().is_some_and(|f| f.equal(&file)));
                if found {
                    let location = Location::Folder(file.clone());
                    pane.current.replace(Some(location.clone()));
                    let node = pane
                        .nodes
                        .borrow()
                        .iter()
                        .find(|node| node_matches(&node.kind, &location))
                        .cloned();
                    if let Some(node) = node {
                        pane.list.select_row(Some(&node.row));
                    }
                } else {
                    let next = pane
                        .nodes
                        .borrow()
                        .iter()
                        .filter(|node| {
                            node.kind
                                .file()
                                .is_some_and(|folder| file.has_prefix(&folder))
                                && !node.expanded.get()
                                && node.expander.is_some()
                        })
                        .max_by_key(|node| node.depth)
                        .cloned();
                    if next.is_some() {
                        pane.reveal(&file);
                    }
                }
            })),
        );
    }
}

fn small_icon_for(drive: &Drive) -> &'static str {
    match drive {
        Drive::Root => "pfm-nav-drive-system",
        Drive::Mount(mount) => icons::small_drive_icon(mount),
        Drive::Volume(volume) => {
            if volume.can_eject() {
                "pfm-nav-drive-usb"
            } else {
                "pfm-nav-drive"
            }
        }
    }
}

fn node_matches(kind: &NodeKind, location: &Location) -> bool {
    match (kind, location) {
        (NodeKind::Home, Location::Home) => true,
        (NodeKind::Gallery, Location::Gallery) => true,
        (NodeKind::ThisPc, Location::ThisPc) => true,
        (NodeKind::Separator, _) => false,
        (kind, Location::Folder(file)) => kind.file().is_some_and(|folder| folder.equal(file)),
        _ => false,
    }
}

/// What appears under a node when it expands.
async fn children_of(kind: &NodeKind) -> Vec<(NodeKind, bool)> {
    match kind {
        NodeKind::ThisPc => thispc::drives()
            .into_iter()
            .map(|drive| (NodeKind::Drive(drive), true))
            .collect(),
        NodeKind::Network => list_folders(&places::network(), true).await,
        NodeKind::Folder(file) => list_folders(file, false).await,
        NodeKind::Drive(drive) => match drive.root() {
            Some(root) => list_folders(&root, false).await,
            None => Vec::new(),
        },
        _ => Vec::new(),
    }
}

async fn list_folders(folder: &gio::File, network: bool) -> Vec<(NodeKind, bool)> {
    let show_hidden = config::flag("show-hidden");
    let Ok(enumerator) = folder
        .enumerate_children_future(
            ATTRIBUTES,
            gio::FileQueryInfoFlags::NONE,
            glib::Priority::DEFAULT,
        )
        .await
    else {
        return Vec::new();
    };
    let mut folders: Vec<(String, gio::File)> = Vec::new();
    while let Ok(infos) = enumerator
        .next_files_future(200, glib::Priority::DEFAULT)
        .await
    {
        if infos.is_empty() {
            break;
        }
        for info in infos {
            let is_folder = matches!(
                info.file_type(),
                gio::FileType::Directory | gio::FileType::Mountable | gio::FileType::Shortcut
            );
            if !is_folder || (!show_hidden && (info.is_hidden() || info.is_backup())) {
                continue;
            }
            let child = match info.attribute_string("standard::target-uri") {
                Some(target) if network => gio::File::for_uri(&target),
                _ => folder.child(info.name()),
            };
            folders.push((info.display_name().to_string(), child));
        }
    }
    folders.sort_by(|a, b| util::natural_cmp(&a.0, &b.0));
    folders
        .into_iter()
        .map(|(_, file)| (NodeKind::Folder(file), !network))
        .collect()
}
