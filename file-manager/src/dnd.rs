//! Windows' drag-and-drop rules: dragging moves within a drive and copies
//! to another drive; Shift forces a move and Ctrl a copy. Anything dropped
//! on the Recycle Bin goes into it.

use gtk::{gdk, gio, prelude::*};

use crate::places;

pub fn preferred_action(target: &gtk::DropTarget, folder: &gio::File) -> gdk::DragAction {
    if places::is_trash(folder) {
        return gdk::DragAction::MOVE;
    }
    let state = target.current_event_state();
    if state.contains(gdk::ModifierType::SHIFT_MASK) {
        return gdk::DragAction::MOVE;
    }
    if state.contains(gdk::ModifierType::CONTROL_MASK) {
        return gdk::DragAction::COPY;
    }
    let first = target
        .value()
        .and_then(|value| value.get::<gdk::FileList>().ok())
        .and_then(|list| list.files().into_iter().next());
    match first {
        Some(source) if same_drive(&source, folder) => gdk::DragAction::MOVE,
        Some(_) => gdk::DragAction::COPY,
        None => gdk::DragAction::COPY,
    }
}

/// What a drop does once it's let go. Dragged from another window (File
/// Manager to the desktop, or back), this one never had the keyboard, so
/// it can't see Ctrl or Shift itself; GNOME picks the action from them for
/// the drag (Shift moves, Ctrl copies) and the drop carries it.
pub fn drop_action(target: &gtk::DropTarget, folder: &gio::File) -> gdk::DragAction {
    let preferred = preferred_action(target, folder);
    if places::is_trash(folder) {
        return preferred;
    }
    let chosen = target.current_drop().map(|drop| drop.actions());
    match chosen {
        Some(action)
            if action == gdk::DragAction::COPY
                || action == gdk::DragAction::MOVE
                || action == gdk::DragAction::LINK =>
        {
            action
        }
        _ => preferred,
    }
}

/// Whether two locations are on the same drive, where a move is a rename.
pub fn same_drive(a: &gio::File, b: &gio::File) -> bool {
    if a.is_native() && b.is_native() {
        let id = |file: &gio::File| {
            // The folder itself may not exist yet; its parent will.
            let mut current = Some(file.clone());
            while let Some(candidate) = current {
                if let Ok(info) = candidate.query_info(
                    "id::filesystem",
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                ) {
                    return info.attribute_string("id::filesystem").map(String::from);
                }
                current = candidate.parent();
            }
            None
        };
        return match (id(a), id(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        };
    }
    let root = |file: &gio::File| {
        let mut root = file.clone();
        while let Some(parent) = root.parent() {
            root = parent;
        }
        root.uri()
    };
    a.uri_scheme() == b.uri_scheme() && root(a) == root(b)
}
