//! File operations through GIO, so they work the same on local drives,
//! removable drives, network shares and the Recycle Bin: copy, move, move to
//! the Recycle Bin, delete, restore, rename, new folders and files,
//! shortcuts, and compressing and extracting archives.
//!
//! Copies and moves follow Windows: a copy into the same folder becomes
//! "name - Copy", folders merge, conflicting files are asked about once for
//! the whole operation, errors offer Try Again, Skip or Cancel, and every
//! operation can be undone (`undo.rs`).

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::Ordering as AtomicOrdering;

use gtk::{gio, glib, prelude::*};

use crate::archive;
use crate::conflicts::{self, Conflict, Decision};
use crate::dialogs::{self, Dialog};
use crate::places;
use crate::progress::Job;
use crate::undo::{self, Action};
use crate::util;

#[derive(Default)]
pub struct Outcome {
    /// What now exists at the destination, top-level items only.
    pub created: Vec<gio::File>,
    /// Top-level items moved, as (from, to).
    pub moved: Vec<(gio::File, gio::File)>,
    pub cancelled: bool,
}

impl Outcome {
    /// The names to select afterwards.
    pub fn created_names(&self) -> Vec<String> {
        self.created
            .iter()
            .filter_map(|file| {
                file.basename()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Copy,
    Move,
}

const INFO: &str =
    "standard::type,standard::size,standard::name,standard::display-name,standard::is-symlink";

async fn query(file: &gio::File, attributes: &str) -> Result<gio::FileInfo, glib::Error> {
    file.query_info_future(
        attributes,
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        glib::Priority::DEFAULT,
    )
    .await
}

async fn exists(file: &gio::File) -> bool {
    query(file, "standard::type").await.is_ok()
}

fn name_of(file: &gio::File) -> String {
    places::display_name(file)
}

fn folder_name(file: Option<gio::File>) -> String {
    file.map(|folder| name_of(&folder)).unwrap_or_default()
}

async fn copy_file(
    source: &gio::File,
    dest: &gio::File,
    flags: gio::FileCopyFlags,
    job: &Rc<Job>,
    moving: bool,
) -> Result<(), glib::Error> {
    let base = job.bytes_done.get();
    let progress = {
        let job = job.clone();
        Box::new(move |current: i64, _total: i64| {
            job.bytes_done.set(base + current.max(0) as u64);
        }) as Box<dyn FnMut(i64, i64)>
    };
    let (sender, receiver) = dialogs::oneshot();
    if moving {
        source.move_async(
            dest,
            flags,
            glib::Priority::DEFAULT,
            Some(&job.cancellable),
            Some(progress),
            move |result| sender.send(result),
        );
    } else {
        source.copy_async(
            dest,
            flags,
            glib::Priority::DEFAULT,
            Some(&job.cancellable),
            Some(progress),
            move |result| sender.send(result),
        );
    }
    receiver.await
}

async fn delete_one(file: &gio::File, job: &Job) -> Result<(), glib::Error> {
    let (sender, receiver) = dialogs::oneshot();
    file.delete_async(
        glib::Priority::DEFAULT,
        Some(&job.cancellable),
        move |result| sender.send(result),
    );
    receiver.await
}

async fn trash_one(file: &gio::File, job: &Job) -> Result<(), glib::Error> {
    let (sender, receiver) = dialogs::oneshot();
    file.trash_async(
        glib::Priority::DEFAULT,
        Some(&job.cancellable),
        move |result| sender.send(result),
    );
    receiver.await
}

/// What the user chose after an error.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OnError {
    Retry,
    Skip,
    Cancel,
}

/// Windows' error prompt: Try Again, Skip or Cancel, with "Do this for all
/// current items".
async fn ask_error(
    parent: Option<&gtk::Window>,
    job: &Job,
    title: &str,
    heading: &str,
    error: &glib::Error,
    skip_all: &Cell<bool>,
) -> OnError {
    if job.is_cancelled() || error.matches(gio::IOErrorEnum::Cancelled) {
        return OnError::Cancel;
    }
    if skip_all.get() {
        return OnError::Skip;
    }
    job.waiting(true);
    let dialog = Dialog::new(parent, title);
    dialog.window.set_default_size(440, -1);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.append(&crate::glyph::Glyph::sized("error", 32));
    let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let heading_label = gtk::Label::builder()
        .label(heading)
        .xalign(0.0)
        .wrap(true)
        .width_chars(36)
        .max_width_chars(48)
        .build();
    heading_label.add_css_class("dialog-heading");
    text.append(&heading_label);
    let detail = gtk::Label::builder()
        .label(dialogs::describe(error))
        .xalign(0.0)
        .wrap(true)
        .width_chars(36)
        .max_width_chars(48)
        .build();
    detail.add_css_class("dialog-detail");
    text.append(&detail);
    let all = gtk::CheckButton::with_label("Do this for all current items");
    text.append(&all);
    row.append(&text);
    dialog.body.append(&row);
    dialog.add_button("_Try Again", 0, true);
    dialog.add_button("_Skip", 1, false);
    dialog.add_button("Cancel", 2, false);
    let choice = dialog.run().await;
    job.waiting(false);
    match choice {
        0 => OnError::Retry,
        1 => {
            if all.is_active() {
                skip_all.set(true);
            }
            OnError::Skip
        }
        _ => {
            job.cancel();
            OnError::Cancel
        }
    }
}

struct Item {
    source: gio::File,
    dest: gio::File,
    is_dir: bool,
    size: u64,
    top: bool,
    overwrite: bool,
    skip: bool,
}

/// Lists everything under `source`, folders before what they hold.
async fn scan(
    source: &gio::File,
    dest: &gio::File,
    items: &mut Vec<Item>,
    job: &Job,
) -> Result<(), glib::Error> {
    let mut stack = vec![(source.clone(), dest.clone(), true)];
    while let Some((source, dest, top)) = stack.pop() {
        if job.is_cancelled() {
            return Err(glib::Error::new(gio::IOErrorEnum::Cancelled, "cancelled"));
        }
        let info = query(&source, INFO).await?;
        let is_dir = info.file_type() == gio::FileType::Directory;
        items.push(Item {
            source: source.clone(),
            dest: dest.clone(),
            is_dir,
            size: if is_dir { 0 } else { info.size().max(0) as u64 },
            top,
            overwrite: false,
            skip: false,
        });
        job.items_total.set(job.items_total.get() + 1);
        if !is_dir {
            job.bytes_total
                .set(job.bytes_total.get() + info.size().max(0) as u64);
            continue;
        }
        let enumerator = source
            .enumerate_children_future(
                INFO,
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                glib::Priority::DEFAULT,
            )
            .await?;
        let mut children = Vec::new();
        loop {
            let infos = enumerator
                .next_files_future(200, glib::Priority::DEFAULT)
                .await?;
            if infos.is_empty() {
                break;
            }
            for child in infos {
                let name = child.name();
                children.push((source.child(&name), dest.child(&name), false));
            }
        }
        children.sort_by_key(|child| std::cmp::Reverse(child.0.uri()));
        stack.extend(children);
    }
    Ok(())
}

/// Copies files and folders into `target`.
pub async fn copy(
    parent: Option<gtk::Window>,
    sources: Vec<gio::File>,
    target: gio::File,
    record: bool,
) -> Outcome {
    transfer(parent, sources, target, Mode::Copy, record).await
}

/// Moves files and folders into `target`. Moving into the Recycle Bin
/// moves them to it.
pub async fn move_to(
    parent: Option<gtk::Window>,
    sources: Vec<gio::File>,
    target: gio::File,
    record: bool,
) -> Outcome {
    if places::is_trash(&target) {
        return trash(parent, sources, record).await;
    }
    transfer(parent, sources, target, Mode::Move, record).await
}

async fn transfer(
    parent: Option<gtk::Window>,
    sources: Vec<gio::File>,
    target: gio::File,
    mode: Mode,
    record: bool,
) -> Outcome {
    let parent = parent.as_ref();
    let verb = if mode == Mode::Copy {
        "Copying"
    } else {
        "Moving"
    };
    let verb_lower = if mode == Mode::Copy { "copy" } else { "move" };
    let from = folder_name(sources.first().and_then(|file| file.parent()));
    let to = name_of(&target);
    let job = Job::start(verb, &from, &to);
    let mut outcome = Outcome::default();
    let skip_all = Cell::new(false);

    // Where each item goes.
    let mut tops = Vec::new();
    for source in &sources {
        if source.equal(&target) || target.has_prefix(source) {
            job.waiting(true);
            let choice = dialogs::message(
                parent,
                "Interrupted Action",
                "warning",
                "The destination folder is a subfolder of the source folder.",
                &format!(
                    "\"{}\" can't be {} into itself.",
                    name_of(source),
                    if mode == Mode::Copy {
                        "copied"
                    } else {
                        "moved"
                    }
                ),
                &[("_Skip", 0), ("Cancel", 1)],
                0,
            )
            .await;
            job.waiting(false);
            if choice != 0 {
                outcome.cancelled = true;
                job.finish();
                return outcome;
            }
            continue;
        }
        let in_same_folder = source.parent().is_some_and(|folder| folder.equal(&target));
        let Some(name) = source
            .basename()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        let dest = if in_same_folder {
            if mode == Mode::Move {
                continue;
            }
            let is_dir = query(source, "standard::type")
                .await
                .is_ok_and(|info| info.file_type() == gio::FileType::Directory);
            target.child(util::copy_name(&target, &name, is_dir))
        } else {
            target.child(&name)
        };
        tops.push((source.clone(), dest));
    }
    job.items_total.set(tops.len() as u64);

    // Within a drive, a move is a rename: try that first.
    let mut rest = Vec::new();
    if mode == Mode::Move {
        let mut moved = 0;
        for (source, dest) in tops {
            if exists(&dest).await {
                rest.push((source, dest));
                continue;
            }
            job.current.replace(name_of(&source));
            let flags =
                gio::FileCopyFlags::NOFOLLOW_SYMLINKS | gio::FileCopyFlags::NO_FALLBACK_FOR_MOVE;
            match copy_file(&source, &dest, flags, &job, true).await {
                Ok(()) => {
                    outcome.moved.push((source.clone(), dest.clone()));
                    outcome.created.push(dest);
                    moved += 1;
                    job.items_done.set(moved);
                }
                Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {
                    outcome.cancelled = true;
                    break;
                }
                Err(_) => rest.push((source, dest)),
            }
        }
        if outcome.cancelled {
            finish(&job, &outcome, mode, &sources, &target, record);
            return outcome;
        }
        job.items_total.set(moved);
        job.items_done.set(moved);
    } else {
        rest = tops;
        job.items_total.set(0);
    }

    // Everything else is copied item by item (and the originals deleted
    // afterwards when moving).
    let mut items = Vec::new();
    for (source, dest) in &rest {
        if let Err(error) = scan(source, dest, &mut items, &job).await {
            if error.matches(gio::IOErrorEnum::Cancelled) {
                outcome.cancelled = true;
                break;
            }
            let heading = format!("Can't read \"{}\".", name_of(source));
            match ask_error(
                parent,
                &job,
                &format!("Error {verb} File or Folder"),
                &heading,
                &error,
                &skip_all,
            )
            .await
            {
                OnError::Cancel => {
                    outcome.cancelled = true;
                    break;
                }
                _ => {
                    items.retain(|item| {
                        !item.source.equal(source) && !item.source.has_prefix(source)
                    });
                }
            }
        }
    }
    if outcome.cancelled {
        finish(&job, &outcome, mode, &sources, &target, record);
        return outcome;
    }

    // Ask once about every file that already exists.
    let mut conflict_indexes = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if item.is_dir {
            // Folders merge, unless a file has the folder's name.
            if let Ok(info) = query(&item.dest, "standard::type").await
                && info.file_type() != gio::FileType::Directory
            {
                conflict_indexes.push(index);
            }
        } else if exists(&item.dest).await {
            conflict_indexes.push(index);
        }
    }
    if !conflict_indexes.is_empty() {
        let conflicts: Vec<Conflict> = conflict_indexes
            .iter()
            .map(|&index| Conflict {
                source: items[index].source.clone(),
                dest: items[index].dest.clone(),
            })
            .collect();
        job.waiting(true);
        let decisions = conflicts::resolve(
            parent,
            verb,
            job.items_total.get().max(rest.len() as u64),
            &from,
            &to,
            &conflicts,
        )
        .await;
        job.waiting(false);
        let Some(decisions) = decisions else {
            outcome.cancelled = true;
            finish(&job, &outcome, mode, &sources, &target, record);
            return outcome;
        };
        for (index, decision) in conflict_indexes.iter().zip(decisions) {
            match decision {
                Decision::Replace => items[*index].overwrite = true,
                Decision::Skip => {
                    let skipped = items[*index].source.clone();
                    for item in items.iter_mut() {
                        if item.source.equal(&skipped) || item.source.has_prefix(&skipped) {
                            item.skip = true;
                        }
                    }
                }
                Decision::KeepBoth => {
                    let item = &items[*index];
                    let folder = item.dest.parent().unwrap_or_else(|| target.clone());
                    let name = item
                        .dest
                        .basename()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let (stem, extension) = if item.is_dir {
                        (name.as_str(), "")
                    } else {
                        util::split_extension(&name)
                    };
                    let new_dest = folder.child(util::unique_name(&folder, stem, extension));
                    let old_dest = item.dest.clone();
                    for item in items.iter_mut() {
                        if item.dest.equal(&old_dest) {
                            item.dest = new_dest.clone();
                        } else if let Some(relative) = old_dest.relative_path(&item.dest) {
                            item.dest = new_dest.resolve_relative_path(relative);
                        }
                    }
                }
            }
        }
    }

    let mut copied_sources: Vec<gio::File> = Vec::new();
    let mut index = 0;
    while index < items.len() {
        job.checkpoint().await;
        if job.is_cancelled() {
            outcome.cancelled = true;
            break;
        }
        let item = &items[index];
        if item.skip {
            job.items_done.set(job.items_done.get() + 1);
            job.bytes_done.set(job.bytes_done.get() + item.size);
            index += 1;
            continue;
        }
        job.current.replace(name_of(&item.source));
        let result = if item.is_dir {
            match item
                .dest
                .make_directory_future(glib::Priority::DEFAULT)
                .await
            {
                Err(error) if error.matches(gio::IOErrorEnum::Exists) => Ok(()),
                other => other,
            }
        } else {
            let mut flags = gio::FileCopyFlags::NOFOLLOW_SYMLINKS;
            if item.overwrite {
                flags |= gio::FileCopyFlags::OVERWRITE;
            }
            let before = job.bytes_done.get();
            let result = copy_file(&item.source, &item.dest, flags, &job, false).await;
            job.bytes_done
                .set(before + if result.is_ok() { item.size } else { 0 });
            result
        };
        match result {
            Ok(()) => {
                if item.top {
                    outcome.created.push(item.dest.clone());
                    if mode == Mode::Move {
                        outcome.moved.push((item.source.clone(), item.dest.clone()));
                    }
                }
                if mode == Mode::Move {
                    if item.is_dir {
                        copied_sources.push(item.source.clone());
                    } else if delete_one(&item.source, &job).await.is_err() {
                        // The copy stands; the original stays too.
                    }
                }
                job.items_done.set(job.items_done.get() + 1);
                index += 1;
            }
            Err(error) => {
                let heading = format!("Can't {verb_lower} \"{}\".", name_of(&item.source));
                match ask_error(
                    parent,
                    &job,
                    &format!("Error {verb} File or Folder"),
                    &heading,
                    &error,
                    &skip_all,
                )
                .await
                {
                    OnError::Retry => {}
                    OnError::Skip => {
                        let skipped = item.source.clone();
                        for later in items.iter_mut().skip(index) {
                            if later.source.equal(&skipped) || later.source.has_prefix(&skipped) {
                                later.skip = true;
                            }
                        }
                    }
                    OnError::Cancel => {
                        outcome.cancelled = true;
                        break;
                    }
                }
            }
        }
    }
    // The moved folders are empty now, unless something was skipped.
    for folder in copied_sources.iter().rev() {
        let _ = delete_one(folder, &job).await;
    }
    finish(&job, &outcome, mode, &sources, &target, record);
    outcome
}

fn finish(
    job: &Job,
    outcome: &Outcome,
    mode: Mode,
    sources: &[gio::File],
    target: &gio::File,
    record: bool,
) {
    job.finish();
    if !record {
        return;
    }
    match mode {
        Mode::Copy if !outcome.created.is_empty() => undo::record(Action::Copy {
            sources: sources.to_vec(),
            target: target.clone(),
            created: outcome.created.clone(),
        }),
        Mode::Move if !outcome.moved.is_empty() => undo::record(Action::Move {
            moves: outcome.moved.clone(),
        }),
        _ => {}
    }
}

/// Moves files to the Recycle Bin. Files on drives without one are offered
/// for permanent deletion instead.
pub async fn trash(parent: Option<gtk::Window>, files: Vec<gio::File>, record: bool) -> Outcome {
    let parent = parent.as_ref();
    let from = folder_name(files.first().and_then(|file| file.parent()));
    let job = Job::start("Deleting", &from, "");
    job.items_total.set(files.len() as u64);
    let mut outcome = Outcome::default();
    let mut trashed = Vec::new();
    let skip_all = Cell::new(false);
    let mut delete_rest: Option<bool> = None;
    for file in &files {
        job.checkpoint().await;
        if job.is_cancelled() {
            outcome.cancelled = true;
            break;
        }
        job.current.replace(name_of(file));
        loop {
            match trash_one(file, &job).await {
                Ok(()) => {
                    trashed.push(file.clone());
                    break;
                }
                Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {
                    outcome.cancelled = true;
                    break;
                }
                Err(error)
                    if error.matches(gio::IOErrorEnum::NotSupported)
                        || error.matches(gio::IOErrorEnum::Failed) =>
                {
                    let delete = match delete_rest {
                        Some(choice) => choice,
                        None => {
                            job.waiting(true);
                            let choice = dialogs::message(
                                parent,
                                "Delete File",
                                "warning",
                                &format!("\"{}\" can't be moved to the Recycle Bin.", name_of(file)),
                                "This location doesn't have a Recycle Bin. Do you want to permanently delete it instead?",
                                &[("_Yes", 0), ("Yes to _All", 1), ("_No", 2), ("Cancel", 3)],
                                2,
                            )
                            .await;
                            job.waiting(false);
                            match choice {
                                0 => true,
                                1 => {
                                    delete_rest = Some(true);
                                    true
                                }
                                2 => false,
                                _ => {
                                    outcome.cancelled = true;
                                    false
                                }
                            }
                        }
                    };
                    if delete {
                        let _ = delete_tree(file, &job).await;
                    }
                    break;
                }
                Err(error) => {
                    let heading = format!("Can't delete \"{}\".", name_of(file));
                    match ask_error(
                        parent,
                        &job,
                        "Error Deleting File or Folder",
                        &heading,
                        &error,
                        &skip_all,
                    )
                    .await
                    {
                        OnError::Retry => continue,
                        OnError::Skip => break,
                        OnError::Cancel => {
                            outcome.cancelled = true;
                            break;
                        }
                    }
                }
            }
        }
        if outcome.cancelled {
            break;
        }
        job.items_done.set(job.items_done.get() + 1);
    }
    job.finish();
    if record && !trashed.is_empty() {
        undo::record(Action::Trash { files: trashed });
    }
    outcome
}

/// Deletes a file, or a folder and everything in it.
async fn delete_tree(root: &gio::File, job: &Job) -> Result<(), glib::Error> {
    let mut items = Vec::new();
    scan(root, root, &mut items, job).await?;
    for item in items.iter().rev() {
        if job.is_cancelled() {
            return Err(glib::Error::new(gio::IOErrorEnum::Cancelled, "cancelled"));
        }
        job.current.replace(name_of(&item.source));
        delete_one(&item.source, job).await?;
        job.items_done.set(job.items_done.get() + 1);
    }
    Ok(())
}

/// Deletes files and folders permanently. The caller has already asked.
pub async fn delete(parent: Option<gtk::Window>, files: Vec<gio::File>, verb: &str) -> Outcome {
    let parent = parent.as_ref();
    let from = folder_name(files.first().and_then(|file| file.parent()));
    let job = Job::start(
        verb,
        if places::is_trash(&files.first().cloned().unwrap_or_else(places::home)) {
            ""
        } else {
            &from
        },
        "",
    );
    let mut outcome = Outcome::default();
    let skip_all = Cell::new(false);
    for file in &files {
        loop {
            match delete_tree(file, &job).await {
                Ok(()) => break,
                Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {
                    outcome.cancelled = true;
                    break;
                }
                Err(error) => {
                    let heading = format!("Can't delete \"{}\".", name_of(file));
                    match ask_error(
                        parent,
                        &job,
                        "Error Deleting File or Folder",
                        &heading,
                        &error,
                        &skip_all,
                    )
                    .await
                    {
                        OnError::Retry => continue,
                        OnError::Skip => break,
                        OnError::Cancel => {
                            outcome.cancelled = true;
                            break;
                        }
                    }
                }
            }
        }
        if outcome.cancelled {
            break;
        }
    }
    job.finish();
    outcome
}

/// Everything in the Recycle Bin.
pub async fn trash_contents() -> Vec<gio::File> {
    let trash = places::trash();
    let Ok(enumerator) = trash
        .enumerate_children_future(
            "standard::name",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            glib::Priority::DEFAULT,
        )
        .await
    else {
        return Vec::new();
    };
    let mut items = Vec::new();
    while let Ok(infos) = enumerator
        .next_files_future(200, glib::Priority::DEFAULT)
        .await
    {
        if infos.is_empty() {
            break;
        }
        items.extend(infos.iter().map(|info| trash.child(info.name())));
    }
    items
}

/// Empties the Recycle Bin, after asking.
pub async fn empty_trash(parent: Option<gtk::Window>) {
    let items = trash_contents().await;
    if items.is_empty() {
        return;
    }
    let question = if items.len() == 1 {
        "Are you sure you want to permanently delete this item?".to_owned()
    } else {
        format!(
            "Are you sure you want to permanently delete these {} items?",
            util::thousands(items.len() as u64)
        )
    };
    let choice = dialogs::message(
        parent.as_ref(),
        "Delete Multiple Items",
        "warning",
        &question,
        "",
        &[("_Yes", 0), ("_No", 1)],
        0,
    )
    .await;
    if choice == 0 {
        delete(parent, items, "Emptying the Recycle Bin").await;
    }
}

/// Puts items from the Recycle Bin back where they came from.
pub async fn restore(parent: Option<gtk::Window>, items: Vec<gio::File>) -> Outcome {
    let parent = parent.as_ref();
    let job = Job::start("Restoring", "Recycle Bin", "");
    job.items_total.set(items.len() as u64);
    let mut outcome = Outcome::default();
    let skip_all = Cell::new(false);
    for item in &items {
        job.current.replace(name_of(item));
        let Ok(info) = query(item, "trash::orig-path,standard::type").await else {
            continue;
        };
        let Some(original) = info.attribute_byte_string("trash::orig-path") else {
            continue;
        };
        let mut dest = gio::File::for_path(original.as_str());
        if let Some(folder) = dest.parent() {
            let _ = folder.make_directory_with_parents(gio::Cancellable::NONE);
            if exists(&dest).await {
                let name = dest
                    .basename()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let is_dir = info.file_type() == gio::FileType::Directory;
                let (stem, extension) = if is_dir {
                    (name.as_str(), "")
                } else {
                    util::split_extension(&name)
                };
                dest = folder.child(util::unique_name(&folder, stem, extension));
            }
        }
        loop {
            match copy_file(
                item,
                &dest,
                gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
                &job,
                true,
            )
            .await
            {
                Ok(()) => {
                    outcome.created.push(dest.clone());
                    break;
                }
                Err(error) => {
                    let heading = format!("Can't restore \"{}\".", name_of(item));
                    match ask_error(
                        parent,
                        &job,
                        "Error Restoring File or Folder",
                        &heading,
                        &error,
                        &skip_all,
                    )
                    .await
                    {
                        OnError::Retry => continue,
                        OnError::Skip => break,
                        OnError::Cancel => {
                            outcome.cancelled = true;
                            break;
                        }
                    }
                }
            }
        }
        if outcome.cancelled {
            break;
        }
        job.items_done.set(job.items_done.get() + 1);
    }
    job.finish();
    outcome
}

/// Restores the Recycle Bin items that were these files, most recent first
/// (for undoing a delete).
pub async fn restore_originals(parent: Option<gtk::Window>, originals: &[gio::File]) -> Outcome {
    let trash = places::trash();
    let mut found: Vec<(String, i64, gio::File)> = Vec::new();
    if let Ok(enumerator) = trash
        .enumerate_children_future(
            "standard::name,trash::orig-path,trash::deletion-date",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            glib::Priority::DEFAULT,
        )
        .await
    {
        while let Ok(infos) = enumerator
            .next_files_future(200, glib::Priority::DEFAULT)
            .await
        {
            if infos.is_empty() {
                break;
            }
            for info in infos {
                let Some(path) = info.attribute_byte_string("trash::orig-path") else {
                    continue;
                };
                let deleted = crate::filecell::deletion_date(&info)
                    .map(|d| d.to_unix())
                    .unwrap_or(0);
                found.push((path.to_string(), deleted, trash.child(info.name())));
            }
        }
    }
    let mut items = Vec::new();
    for original in originals {
        let Some(path) = original.path() else {
            continue;
        };
        let path = path.to_string_lossy().into_owned();
        if let Some((_, _, item)) = found
            .iter()
            .filter(|(p, _, _)| *p == path)
            .max_by_key(|(_, deleted, _)| *deleted)
        {
            items.push(item.clone());
        }
    }
    restore(parent, items).await
}

/// Renames a file, checking the name first. Returns the renamed file.
pub async fn rename(
    parent: Option<gtk::Window>,
    file: gio::File,
    name: String,
    record: bool,
) -> Option<gio::File> {
    let parent = parent.as_ref();
    let name = name.trim().to_owned();
    if name.is_empty() {
        dialogs::error(parent, "Rename", "You must type a file name.", "");
        return None;
    }
    if name.contains('/') || name == "." || name == ".." {
        dialogs::error(
            parent,
            "Rename",
            "A file name can't contain the character /",
            "Names \".\" and \"..\" aren't allowed either.",
        );
        return None;
    }
    let mut name = name;
    if let Some(folder) = file.parent() {
        let dest = folder.child(&name);
        let same_file = dest.equal(&file);
        let only_case = file
            .basename()
            .is_some_and(|old| old.to_string_lossy().to_lowercase() == name.to_lowercase());
        if !same_file && !only_case && exists(&dest).await {
            let is_dir = query(&file, "standard::type")
                .await
                .is_ok_and(|info| info.file_type() == gio::FileType::Directory);
            let (stem, extension) = if is_dir {
                (name.as_str(), "")
            } else {
                util::split_extension(&name)
            };
            let suggestion = util::unique_name(&folder, stem, extension);
            let choice = dialogs::message(
                parent,
                if is_dir {
                    "Rename Folder"
                } else {
                    "Rename File"
                },
                "question",
                &format!(
                    "There is already a {} with the same name in this location.",
                    if is_dir { "folder" } else { "file" }
                ),
                &format!(
                    "Would you like to rename \"{}\" to \"{suggestion}\"?",
                    name_of(&file)
                ),
                &[("_Yes", 0), ("_No", 1)],
                0,
            )
            .await;
            if choice != 0 {
                return None;
            }
            name = suggestion;
        }
    }
    let (sender, receiver) = dialogs::oneshot();
    file.set_display_name_async(
        &name,
        glib::Priority::DEFAULT,
        gio::Cancellable::NONE,
        move |result| sender.send(result),
    );
    match receiver.await {
        Ok(renamed) => {
            if record {
                undo::record(Action::Rename {
                    from: file,
                    to: renamed.clone(),
                });
            }
            Some(renamed)
        }
        Err(error) => {
            dialogs::error(
                parent,
                "Rename",
                &format!("Can't rename \"{}\".", name_of(&file)),
                &dialogs::describe(&error),
            );
            None
        }
    }
}

/// Makes "New folder" (or "New folder (2)"...) in `folder`.
pub async fn new_folder(
    parent: Option<gtk::Window>,
    folder: gio::File,
    record: bool,
) -> Option<gio::File> {
    let dest = folder.child(util::unique_name(&folder, "New folder", ""));
    match dest.make_directory_future(glib::Priority::DEFAULT).await {
        Ok(()) => {
            if record {
                undo::record(Action::Create {
                    file: dest.clone(),
                    folder: true,
                    template: None,
                });
            }
            Some(dest)
        }
        Err(error) => {
            dialogs::error(
                parent.as_ref(),
                "New Folder",
                "Can't make a new folder here.",
                &dialogs::describe(&error),
            );
            None
        }
    }
}

/// Makes a new file named `base` + `extension`, empty or from a template.
pub async fn new_file(
    parent: Option<gtk::Window>,
    folder: gio::File,
    base: &str,
    extension: &str,
    template: Option<gio::File>,
    record: bool,
) -> Option<gio::File> {
    let dest = folder.child(util::unique_name(&folder, base, extension));
    let result = match &template {
        Some(template) => {
            let (sender, receiver) = dialogs::oneshot();
            template.copy_async(
                &dest,
                gio::FileCopyFlags::NONE,
                glib::Priority::DEFAULT,
                gio::Cancellable::NONE,
                None,
                move |r| sender.send(r),
            );
            receiver.await
        }
        None => {
            let (sender, receiver) = dialogs::oneshot();
            dest.create_async(
                gio::FileCreateFlags::NONE,
                glib::Priority::DEFAULT,
                gio::Cancellable::NONE,
                move |r| sender.send(r),
            );
            match receiver.await {
                Ok(stream) => stream.close_future(glib::Priority::DEFAULT).await,
                Err(error) => Err(error),
            }
        }
    };
    match result {
        Ok(()) => {
            if record {
                undo::record(Action::Create {
                    file: dest.clone(),
                    folder: false,
                    template,
                });
            }
            Some(dest)
        }
        Err(error) => {
            dialogs::error(
                parent.as_ref(),
                "New",
                "Can't make a new file here.",
                &dialogs::describe(&error),
            );
            None
        }
    }
}

/// Makes shortcuts (symbolic links) to `sources` in `target`.
pub async fn link(
    parent: Option<gtk::Window>,
    sources: Vec<gio::File>,
    target: gio::File,
    record: bool,
) -> Outcome {
    let mut outcome = Outcome::default();
    for source in &sources {
        let Some(path) = source.path() else { continue };
        let name = source
            .basename()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let in_same_folder = source.parent().is_some_and(|folder| folder.equal(&target));
        let base = if in_same_folder {
            format!("{name} - Shortcut")
        } else {
            name
        };
        let dest = target.child(util::unique_name(&target, &base, ""));
        match dest.make_symbolic_link(&path, gio::Cancellable::NONE) {
            Ok(()) => outcome.created.push(dest),
            Err(error) => {
                dialogs::error(
                    parent.as_ref(),
                    "Create Shortcut",
                    &format!("Can't make a shortcut to \"{}\".", name_of(source)),
                    &dialogs::describe(&error),
                );
                break;
            }
        }
    }
    if record && !outcome.created.is_empty() {
        undo::record(Action::Link {
            sources,
            target,
            created: outcome.created.clone(),
        });
    }
    outcome
}

/// Runs an archive job on a worker thread, showing its progress.
async fn archive_job(
    verb: &str,
    from: &str,
    to: &str,
    work: impl FnOnce(&archive::Progress) -> std::io::Result<()> + Send + 'static,
) -> Result<(), String> {
    let job = Job::start(verb, from, to);
    job.items_total.set(1);
    let shared = archive::Progress::new();
    let worker = shared.clone();
    let poll = {
        let job = job.clone();
        let shared = shared.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            job.bytes_total
                .set(shared.total.load(AtomicOrdering::Relaxed));
            job.bytes_done
                .set(shared.done.load(AtomicOrdering::Relaxed));
            if job.is_cancelled() {
                shared.cancelled.store(true, AtomicOrdering::Relaxed);
            }
            glib::ControlFlow::Continue
        })
    };
    let result = gio::spawn_blocking(move || work(&worker)).await;
    poll.remove();
    job.finish();
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::Interrupted => Err(String::new()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("The operation stopped unexpectedly.".into()),
    }
}

/// "Compress to ZIP file": makes an archive beside the items. Returns it.
pub async fn compress(
    parent: Option<gtk::Window>,
    files: Vec<gio::File>,
    format: archive::Format,
) -> Option<gio::File> {
    let paths: Vec<_> = files.iter().filter_map(|file| file.path()).collect();
    let folder = files.first().and_then(|file| file.parent())?;
    if paths.len() != files.len() || paths.is_empty() {
        dialogs::error(
            parent.as_ref(),
            "Compressed (zipped) Folders",
            "Only files on this PC can be compressed.",
            "",
        );
        return None;
    }
    let first = files[0]
        .basename()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let is_dir = paths[0].is_dir();
    let stem = if is_dir {
        first.as_str()
    } else {
        util::split_extension(&first).0
    };
    let target = folder.child(util::unique_name(&folder, stem, format.extension()));
    let target_path = target.path()?;
    let name = name_of(&target);
    let from = name_of(&folder);
    let result = archive_job("Compressing", &from, &name, move |progress| {
        archive::compress(&paths, &target_path, format, progress)
    })
    .await;
    match result {
        Ok(()) => {
            undo::record(Action::Create {
                file: target.clone(),
                folder: false,
                template: None,
            });
            Some(target)
        }
        Err(message) if message.is_empty() => None,
        Err(message) => {
            dialogs::error(
                parent.as_ref(),
                "Compressed (zipped) Folders",
                "The archive could not be made.",
                &message,
            );
            None
        }
    }
}

/// "Extract All": unpacks an archive into `target`.
pub async fn extract(
    parent: Option<gtk::Window>,
    archive_file: gio::File,
    target: gio::File,
) -> bool {
    let (Some(archive_path), Some(target_path)) = (archive_file.path(), target.path()) else {
        dialogs::error(
            parent.as_ref(),
            "Extract Compressed (Zipped) Folders",
            "Only archives on this PC can be extracted.",
            "",
        );
        return false;
    };
    let from = name_of(&archive_file);
    let to = name_of(&target);
    let result = archive_job("Extracting", &from, &to, move |progress| {
        archive::extract(&archive_path, &target_path, progress)
    })
    .await;
    match result {
        Ok(()) => true,
        Err(message) if message.is_empty() => false,
        Err(message) => {
            dialogs::error(
                parent.as_ref(),
                "Extract Compressed (Zipped) Folders",
                "The archive could not be extracted.",
                &message,
            );
            false
        }
    }
}
