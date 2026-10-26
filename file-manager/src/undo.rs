//! Undo and redo (Ctrl+Z and Ctrl+Y), shared by every window as on Windows.
//! Undoing a copy moves the copies to the Recycle Bin, a move moves the
//! items back, a delete restores them from the Recycle Bin, a rename
//! renames back, and a new item goes to the Recycle Bin.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gtk::{gio, prelude::*};

use crate::ops;

#[derive(Clone)]
pub enum Action {
    Copy {
        sources: Vec<gio::File>,
        target: gio::File,
        created: Vec<gio::File>,
    },
    Move {
        moves: Vec<(gio::File, gio::File)>,
    },
    Rename {
        from: gio::File,
        to: gio::File,
    },
    Trash {
        files: Vec<gio::File>,
    },
    Create {
        file: gio::File,
        folder: bool,
        template: Option<gio::File>,
    },
    Link {
        sources: Vec<gio::File>,
        target: gio::File,
        created: Vec<gio::File>,
    },
}

impl Action {
    fn noun(&self) -> &'static str {
        match self {
            Action::Copy { .. } => "Copy",
            Action::Move { .. } => "Move",
            Action::Rename { .. } => "Rename",
            Action::Trash { .. } => "Delete",
            Action::Create { .. } => "New",
            Action::Link { .. } => "Create shortcut",
        }
    }
}

thread_local! {
    static UNDO: RefCell<Vec<Action>> = const { RefCell::new(Vec::new()) };
    static REDO: RefCell<Vec<Action>> = const { RefCell::new(Vec::new()) };
    static LISTENERS: RefCell<Vec<Rc<dyn Fn()>>> = const { RefCell::new(Vec::new()) };
}

/// Runs `listener` whenever what can be undone changes.
pub fn connect_changed(listener: impl Fn() + 'static) {
    LISTENERS.with(|listeners| listeners.borrow_mut().push(Rc::new(listener)));
}

fn notify() {
    let listeners: Vec<_> = LISTENERS.with(|listeners| listeners.borrow().clone());
    for listener in listeners {
        listener();
    }
}

/// Records an operation that just finished.
pub fn record(action: Action) {
    UNDO.with(|undo| {
        let mut undo = undo.borrow_mut();
        undo.push(action);
        // Windows keeps a long history; this keeps a generous one.
        let len = undo.len();
        if len > 200 {
            undo.drain(..len - 200);
        }
    });
    REDO.with(|redo| redo.borrow_mut().clear());
    notify();
}

pub fn can_undo() -> bool {
    UNDO.with(|undo| !undo.borrow().is_empty())
}

pub fn can_redo() -> bool {
    REDO.with(|redo| !redo.borrow().is_empty())
}

/// "Undo Copy", "Undo Delete"...
pub fn undo_label() -> String {
    UNDO.with(|undo| match undo.borrow().last() {
        Some(action) => format!("Undo {}", action.noun()),
        None => "Undo".into(),
    })
}

pub fn redo_label() -> String {
    REDO.with(|redo| match redo.borrow().last() {
        Some(action) => format!("Redo {}", action.noun()),
        None => "Redo".into(),
    })
}

/// Moves each file to the folder it came from, a folder at a time.
async fn move_back(parent: Option<gtk::Window>, moves: &[(gio::File, gio::File)], forward: bool) {
    let mut by_folder: BTreeMap<String, (gio::File, Vec<gio::File>)> = BTreeMap::new();
    for (from, to) in moves {
        let (item, destination) = if forward { (from, to) } else { (to, from) };
        let Some(folder) = destination.parent() else {
            continue;
        };
        by_folder
            .entry(folder.uri().to_string())
            .or_insert_with(|| (folder.clone(), Vec::new()))
            .1
            .push(item.clone());
    }
    for (_, (folder, items)) in by_folder {
        ops::move_to(parent.clone(), items, folder, false).await;
    }
}

/// Undoes the last operation. Returns the items now back in place, to select.
pub async fn undo(parent: Option<gtk::Window>) -> Vec<gio::File> {
    let Some(action) = UNDO.with(|undo| undo.borrow_mut().pop()) else {
        return Vec::new();
    };
    notify();
    let back = match &action {
        Action::Copy { created, .. } => {
            ops::trash(parent, created.clone(), false).await;
            Vec::new()
        }
        Action::Move { moves } => {
            move_back(parent, moves, false).await;
            moves.iter().map(|(from, _)| from.clone()).collect()
        }
        Action::Rename { from, to } => {
            let name = from
                .basename()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            ops::rename(parent, to.clone(), name, false)
                .await
                .into_iter()
                .collect()
        }
        Action::Trash { files } => ops::restore_originals(parent, files).await.created,
        Action::Create { file, .. } => {
            ops::trash(parent, vec![file.clone()], false).await;
            Vec::new()
        }
        Action::Link { created, .. } => {
            ops::delete(parent, created.clone(), "Deleting").await;
            Vec::new()
        }
    };
    REDO.with(|redo| redo.borrow_mut().push(action));
    notify();
    back
}

/// Does the last undone operation again. Returns the items it made, to select.
pub async fn redo(parent: Option<gtk::Window>) -> Vec<gio::File> {
    let Some(action) = REDO.with(|redo| redo.borrow_mut().pop()) else {
        return Vec::new();
    };
    notify();
    let redone = match action {
        Action::Copy {
            sources, target, ..
        } => {
            let outcome = ops::copy(parent, sources.clone(), target.clone(), false).await;
            Action::Copy {
                sources,
                target,
                created: outcome.created,
            }
        }
        Action::Move { moves } => {
            move_back(parent, &moves, true).await;
            Action::Move { moves }
        }
        Action::Rename { from, to } => {
            let name = to
                .basename()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let renamed = ops::rename(parent, from.clone(), name, false).await;
            Action::Rename {
                from,
                to: renamed.unwrap_or(to),
            }
        }
        Action::Trash { files } => {
            ops::trash(parent, files.clone(), false).await;
            Action::Trash { files }
        }
        Action::Create {
            file,
            folder,
            template,
        } => {
            if let Some(parent_folder) = file.parent() {
                let name = file
                    .basename()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if folder {
                    let _ = file.make_directory(gio::Cancellable::NONE);
                } else {
                    let (stem, extension) = crate::util::split_extension(&name);
                    ops::new_file(
                        parent,
                        parent_folder,
                        stem,
                        extension,
                        template.clone(),
                        false,
                    )
                    .await;
                }
            }
            Action::Create {
                file,
                folder,
                template,
            }
        }
        Action::Link {
            sources, target, ..
        } => {
            let outcome = ops::link(parent, sources.clone(), target.clone(), false).await;
            Action::Link {
                sources,
                target,
                created: outcome.created,
            }
        }
    };
    let made = match &redone {
        Action::Copy { created, .. } | Action::Link { created, .. } => created.clone(),
        Action::Move { moves } => moves.iter().map(|(_, to)| to.clone()).collect(),
        Action::Rename { to, .. } => vec![to.clone()],
        Action::Create { file, .. } => vec![file.clone()],
        Action::Trash { .. } => Vec::new(),
    };
    UNDO.with(|undo| undo.borrow_mut().push(redone));
    notify();
    made
}
