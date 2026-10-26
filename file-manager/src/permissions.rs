//! Properties' Permissions tab, Linux's take on Windows' Security tab: who
//! owns an item, what its owner, its group and everyone else may do with
//! it, whether a file may run as a program, the special bits, and for a
//! folder, the same for everything inside. A change the user can't make
//! themselves is made as administrator after the system asks for their
//! password (pkexec). With "System folder hints" on, the tab also shows the
//! commands that make the same changes.

use std::cell::{Cell, RefCell};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::{gio, glib, prelude::*};

use crate::hints;

const READ: u32 = 0o444;
const WRITE: u32 = 0o222;
const RUN: u32 = 0o111;
const SETUID: u32 = 0o4000;
const SETGID: u32 = 0o2000;
const STICKY: u32 = 0o1000;

pub struct Permissions {
    pub widget: gtk::Box,
    file: gio::File,
    path: Option<PathBuf>,
    is_dir: bool,
    /// The user running File Manager.
    me: Account,
    owner_uid: Cell<u32>,
    groups: Vec<Group>,
    owner: gtk::DropDown,
    group: gtk::DropDown,
    /// The nine Read, Write and Run boxes, with their bits.
    checks: Vec<(u32, gtk::CheckButton)>,
    run: Option<gtk::CheckButton>,
    special: Vec<(u32, gtk::CheckButton)>,
    recursive: Option<gtk::CheckButton>,
    mode_label: gtk::Label,
    notice: gtk::Label,
    commands: gtk::Label,
    hint_box: gtk::Box,
    /// What's on disk, after the last Apply.
    applied: Cell<u32>,
    applied_owner: RefCell<String>,
    applied_group: RefCell<String>,
    /// Set while boxes are ticked by code, so they don't echo.
    syncing: Cell<bool>,
}

struct Account {
    uid: u32,
    name: String,
    gid: u32,
}

struct Group {
    name: String,
    gid: u32,
    members: Vec<String>,
}

fn read_lines(path: &str) -> Vec<Vec<String>> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| line.split(':').map(str::to_owned).collect())
        .collect()
}

/// The accounts people sign in with, root, and the item's owner.
fn users(owner: &str) -> Vec<String> {
    let mut users: Vec<String> = read_lines("/etc/passwd")
        .into_iter()
        .filter(|fields| fields.len() > 2)
        .filter(|fields| {
            let uid: u32 = fields[2].parse().unwrap_or(u32::MAX);
            uid == 0 || (1000..65534).contains(&uid) || fields[0] == owner
        })
        .map(|fields| fields[0].clone())
        .collect();
    if !owner.is_empty() && !users.iter().any(|user| user == owner) {
        users.push(owner.to_owned());
    }
    users
}

fn groups() -> Vec<Group> {
    let mut groups: Vec<Group> = read_lines("/etc/group")
        .into_iter()
        .filter(|fields| fields.len() > 2)
        .map(|fields| Group {
            name: fields[0].clone(),
            gid: fields[2].parse().unwrap_or(u32::MAX),
            members: fields
                .get(3)
                .map(|members| {
                    members
                        .split(',')
                        .filter(|m| !m.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    groups
}

/// Who File Manager runs as: /proc/self belongs to the running process.
fn me() -> Account {
    let uid = std::fs::metadata("/proc/self")
        .map(|meta| meta.uid())
        .unwrap_or(u32::MAX);
    let account = read_lines("/etc/passwd")
        .into_iter()
        .find(|fields| fields.len() > 3 && fields[2].parse() == Ok(uid));
    Account {
        uid,
        name: account
            .as_ref()
            .map(|fields| fields[0].clone())
            .unwrap_or_else(|| glib::user_name().to_string_lossy().into_owned()),
        gid: account
            .and_then(|fields| fields[3].parse().ok())
            .unwrap_or(u32::MAX),
    }
}

/// `-rwxr-xr-x`, with the special bits where `ls` shows them.
pub fn symbolic(mode: u32, is_dir: bool) -> String {
    let mut text = String::from(if is_dir { "d" } else { "-" });
    for (shift, special, letter) in [(6, SETUID, 's'), (3, SETGID, 's'), (0, STICKY, 't')] {
        let bits = (mode >> shift) & 0o7;
        text.push(if bits & 4 != 0 { 'r' } else { '-' });
        text.push(if bits & 2 != 0 { 'w' } else { '-' });
        let run = bits & 1 != 0;
        text.push(match (mode & special != 0, run) {
            (true, true) => letter,
            (true, false) => letter.to_ascii_uppercase(),
            (false, true) => 'x',
            (false, false) => '-',
        });
    }
    text
}

/// Settings for everything inside a folder: Run only for folders and for
/// files that can already run, as `chmod`'s `X` does.
fn recursive_mode(mode: u32) -> String {
    let class = |shift: u32| {
        let bits = (mode >> shift) & 0o7;
        let mut text = String::new();
        if bits & 4 != 0 {
            text.push('r');
        }
        if bits & 2 != 0 {
            text.push('w');
        }
        if bits & 1 != 0 {
            text.push('X');
        }
        text
    };
    format!("u={},g={},o={}", class(6), class(3), class(0))
}

/// Paths and commands break anywhere, without Pango's added hyphens, which
/// would read as part of the name.
fn no_hyphens() -> gtk::pango::AttrList {
    let attributes = gtk::pango::AttrList::new();
    attributes.insert(gtk::pango::AttrInt::new_insert_hyphens(false));
    attributes
}

fn check_cell() -> gtk::CheckButton {
    let check = gtk::CheckButton::new();
    check.set_halign(gtk::Align::Center);
    check
}

fn key_label(text: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    label.add_css_class("property-key");
    label
}

fn dropdown(names: &[String], current: &str) -> gtk::DropDown {
    let strings: Vec<&str> = names.iter().map(String::as_str).collect();
    let dropdown = gtk::DropDown::from_strings(&strings);
    dropdown.set_enable_search(true);
    dropdown.set_expression(Some(gtk::PropertyExpression::new(
        gtk::StringObject::static_type(),
        None::<gtk::Expression>,
        "string",
    )));
    if let Some(index) = names.iter().position(|name| name == current) {
        dropdown.set_selected(index as u32);
    }
    dropdown.set_hexpand(true);
    dropdown
}

impl Permissions {
    pub fn new(info: &gio::FileInfo, file: &gio::File) -> Rc<Permissions> {
        let is_dir = crate::util::is_dir(info);
        let mode = info.attribute_uint32("unix::mode") & 0o7777;
        let owner_name = info
            .attribute_string("owner::user")
            .map(String::from)
            .unwrap_or_default();
        let group_name = info
            .attribute_string("owner::group")
            .map(String::from)
            .unwrap_or_default();
        let path = file.path();
        let users = users(&owner_name);
        let groups = groups();
        let group_names: Vec<String> = groups.iter().map(|g| g.name.clone()).collect();

        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        widget.add_css_class("property-page");
        widget.add_css_class("permissions-page");

        let names = gtk::Grid::builder()
            .row_spacing(8)
            .column_spacing(16)
            .build();
        let object = gtk::Label::builder()
            .label(
                path.as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file.uri().to_string()),
            )
            .xalign(0.0)
            .selectable(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::Char)
            .attributes(&no_hyphens())
            .width_chars(28)
            .max_width_chars(44)
            .hexpand(true)
            .build();
        names.attach(&key_label("Object name:"), 0, 0, 1, 1);
        names.attach(&object, 1, 0, 1, 1);
        let owner = dropdown(&users, &owner_name);
        let group = dropdown(&group_names, &group_name);
        // Only local files have owners that can be changed.
        owner.set_sensitive(path.is_some());
        group.set_sensitive(path.is_some());
        names.attach(&key_label("Owner:"), 0, 1, 1, 1);
        names.attach(&owner, 1, 1, 1, 1);
        names.attach(&key_label("Group:"), 0, 2, 1, 1);
        names.attach(&group, 1, 2, 1, 1);
        widget.append(&names);

        // Who may do what: a row each for the owner, the group and everyone
        // else, as Windows lists users and groups with Allow boxes.
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        frame.add_css_class("permission-table");
        let table = gtk::Grid::builder()
            .row_spacing(4)
            .column_spacing(12)
            .column_homogeneous(false)
            .build();
        let columns = if is_dir {
            ["List contents", "Add and delete", "Open"]
        } else {
            ["Read", "Write", "Run"]
        };
        let heading = gtk::Label::builder()
            .label("Who")
            .xalign(0.0)
            .hexpand(true)
            .build();
        heading.add_css_class("permission-heading");
        table.attach(&heading, 0, 0, 1, 1);
        for (column, title) in columns.iter().enumerate() {
            let label = gtk::Label::builder().label(*title).build();
            label.add_css_class("permission-heading");
            table.attach(&label, column as i32 + 1, 0, 1, 1);
        }
        let mut checks = Vec::new();
        for (index, title) in ["Owner", "Group", "Everyone else"].iter().enumerate() {
            let label = gtk::Label::builder().label(*title).xalign(0.0).build();
            table.attach(&label, 0, index as i32 + 1, 1, 1);
            for (column, base) in [0o4, 0o2, 0o1].iter().enumerate() {
                let bit = base << ((2 - index) * 3);
                let check = check_cell();
                check.set_active(mode & bit != 0);
                table.attach(&check, column as i32 + 1, index as i32 + 1, 1, 1);
                checks.push((bit, check));
            }
        }
        frame.append(&table);
        widget.append(&frame);

        let run = (!is_dir).then(|| {
            let run = gtk::CheckButton::with_mnemonic("Allow _running this file as a program");
            run.set_tooltip_text(Some(
                "Lets it run like an app or command (chmod +x). For scripts and programs.",
            ));
            widget.append(&run);
            run
        });

        // The special bits, out of the way like Windows' Advanced button.
        let special_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        special_box.set_margin_top(4);
        let mut special = Vec::new();
        let special_list: Vec<(u32, &str, &str)> = if is_dir {
            vec![
                (
                    SETGID,
                    "New items get this folder's group (set group ID)",
                    "Files and folders created inside belong to this folder's group, for shared folders.",
                ),
                (
                    STICKY,
                    "Only owners can delete or rename items inside (sticky)",
                    "As /tmp is: everyone may add items, but only an item's owner can remove it.",
                ),
            ]
        } else {
            vec![
                (
                    SETUID,
                    "Run as the file's owner (set user ID)",
                    "The program runs with its owner's rights, whoever starts it. Rarely wanted: it's how sudo works.",
                ),
                (
                    SETGID,
                    "Run with the file's group (set group ID)",
                    "The program runs with its group's rights, whoever starts it.",
                ),
            ]
        };
        for (bit, label, tip) in special_list {
            let check = gtk::CheckButton::with_label(label);
            check.set_tooltip_text(Some(tip));
            check.set_active(mode & bit != 0);
            special_box.append(&check);
            special.push((bit, check));
        }
        let expander = gtk::Expander::builder()
            .label("Special permissions")
            .child(&special_box)
            .expanded(mode & (SETUID | SETGID | STICKY) != 0)
            .build();
        widget.append(&expander);

        let recursive = is_dir.then(|| {
            let check = gtk::CheckButton::with_mnemonic(
                "_Apply to everything inside this folder (files run only if they already could)",
            );
            widget.append(&check);
            check
        });

        let mode_label = gtk::Label::builder().xalign(0.0).selectable(true).build();
        mode_label.add_css_class("permission-mode");
        widget.append(&mode_label);

        let notice = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .width_chars(28)
            .max_width_chars(60)
            .visible(false)
            .build();
        notice.add_css_class("permission-notice");
        widget.append(&notice);

        // The commands for the same changes, for "System folder hints".
        let hint_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        hint_box.add_css_class("permission-commands");
        let hint_heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let hint_title = gtk::Label::builder()
            .label("The same as commands")
            .xalign(0.0)
            .hexpand(true)
            .build();
        hint_title.add_css_class("hint-title");
        let copy = gtk::Button::with_label("Copy");
        copy.add_css_class("dialog-button");
        hint_heading.append(&hint_title);
        hint_heading.append(&copy);
        let commands = gtk::Label::builder()
            .xalign(0.0)
            .selectable(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::Char)
            .attributes(&no_hyphens())
            .width_chars(28)
            .max_width_chars(60)
            .build();
        commands.add_css_class("command-text");
        let explain = gtk::Label::builder()
            .label("sudo runs a command as administrator after asking for your password. File Manager asks the same way when you apply a change only an administrator can make.")
            .xalign(0.0)
            .wrap(true)
            .width_chars(28)
            .max_width_chars(60)
            .build();
        explain.add_css_class("hint-windows");
        hint_box.append(&hint_heading);
        hint_box.append(&commands);
        hint_box.append(&explain);
        widget.append(&hint_box);

        let owner_uid = info.attribute_uint32("unix::uid");
        let permissions = Rc::new(Permissions {
            widget,
            file: file.clone(),
            path,
            is_dir,
            me: me(),
            owner_uid: Cell::new(owner_uid),
            groups,
            owner,
            group,
            checks,
            run,
            special,
            recursive,
            mode_label,
            notice,
            commands,
            hint_box,
            applied: Cell::new(mode),
            applied_owner: RefCell::new(owner_name),
            applied_group: RefCell::new(group_name),
            syncing: Cell::new(false),
        });
        permissions.connect();
        {
            let label = permissions.commands.clone();
            copy.connect_clicked(move |button| {
                button.clipboard().set_text(&label.text());
            });
        }
        permissions.refresh();
        permissions
    }

    fn connect(self: &Rc<Self>) {
        let refresh = {
            let weak = Rc::downgrade(self);
            move || {
                if let Some(page) = weak.upgrade()
                    && !page.syncing.get()
                {
                    page.refresh();
                }
            }
        };
        for (_, check) in self.checks.iter().chain(&self.special) {
            let refresh = refresh.clone();
            check.connect_toggled(move |_| refresh());
        }
        if let Some(recursive) = &self.recursive {
            let refresh = refresh.clone();
            recursive.connect_toggled(move |_| refresh());
        }
        for dropdown in [&self.owner, &self.group] {
            let refresh = refresh.clone();
            dropdown.connect_selected_notify(move |_| refresh());
        }
        if let Some(run) = &self.run {
            let weak = Rc::downgrade(self);
            run.connect_toggled(move |run| {
                let Some(page) = weak.upgrade() else { return };
                if page.syncing.get() {
                    return;
                }
                // Whoever may read it may run it; the owner always may.
                let mode = page.mode();
                let wanted = if run.is_active() {
                    ((mode & READ) >> 2) | 0o100
                } else {
                    0
                };
                page.set_bits(RUN, wanted);
            });
        }
    }

    /// Ticks the boxes for the bits in `mask` to match `bits`.
    fn set_bits(&self, mask: u32, bits: u32) {
        self.syncing.set(true);
        for (bit, check) in &self.checks {
            if mask & bit != 0 {
                check.set_active(bits & bit != 0);
            }
        }
        self.syncing.set(false);
        self.refresh();
    }

    /// General's Read-only box: no one may write, or the owner may again.
    pub fn set_read_only(&self, read_only: bool) {
        if read_only {
            self.set_bits(WRITE, 0);
        } else if self.mode() & WRITE == 0 {
            self.set_bits(0o200, 0o200);
        }
    }

    /// The mode the boxes describe.
    pub fn mode(&self) -> u32 {
        self.checks
            .iter()
            .chain(&self.special)
            .filter(|(_, check)| check.is_active())
            .fold(0, |mode, (bit, _)| mode | bit)
    }

    fn selected(dropdown: &gtk::DropDown) -> String {
        dropdown
            .selected_item()
            .and_downcast::<gtk::StringObject>()
            .map(|item| item.string().to_string())
            .unwrap_or_default()
    }

    fn owner(&self) -> String {
        Self::selected(&self.owner)
    }

    fn group(&self) -> String {
        Self::selected(&self.group)
    }

    fn recursive(&self) -> bool {
        self.recursive
            .as_ref()
            .is_some_and(|check| check.is_active())
    }

    fn is_root(&self) -> bool {
        self.me.uid == 0
    }

    fn i_own_it(&self) -> bool {
        self.owner_uid.get() == self.me.uid
    }

    fn in_group(&self, name: &str) -> bool {
        self.groups.iter().any(|group| {
            group.name == name
                && (group.gid == self.me.gid || group.members.contains(&self.me.name))
        })
    }

    fn owner_changed(&self) -> bool {
        self.owner() != *self.applied_owner.borrow()
    }

    fn group_changed(&self) -> bool {
        self.group() != *self.applied_group.borrow()
    }

    /// Whether the change takes an administrator. Inside a folder there may
    /// be items owned by others: those show up when the change is tried.
    fn needs_admin(&self) -> bool {
        if self.is_root() {
            return false;
        }
        self.owner_changed()
            || !self.i_own_it()
            || (self.group_changed() && !self.in_group(&self.group()))
    }

    /// The commands that make the boxes true, as argument lists: what has
    /// changed, or with `all`, everything the tab shows (for the hints).
    fn command_lines(&self, all: bool) -> Vec<Vec<String>> {
        let Some(path) = &self.path else {
            return Vec::new();
        };
        let path = path.to_string_lossy().into_owned();
        let recursive = self.recursive();
        let mut lines = Vec::new();
        let ownership = if self.owner_changed() || (all && !self.i_own_it()) {
            Some(vec![
                "chown".to_owned(),
                format!("{}:{}", self.owner(), self.group()),
            ])
        } else if self.group_changed() || all {
            Some(vec!["chgrp".to_owned(), self.group()])
        } else {
            None
        };
        if let Some(mut line) = ownership {
            if recursive {
                line.insert(1, "-R".into());
            }
            line.extend(["--".into(), path.clone()]);
            lines.push(line);
        }
        let mode = self.mode();
        if recursive {
            lines.push(vec![
                "chmod".into(),
                "-R".into(),
                recursive_mode(mode),
                "--".into(),
                path.clone(),
            ]);
        }
        if all || recursive || mode != self.applied.get() {
            lines.push(vec![
                "chmod".into(),
                format!("{mode:04o}"),
                "--".into(),
                path,
            ]);
        }
        lines
    }

    fn quoted(line: &[String]) -> String {
        line.iter()
            .map(|arg| {
                if arg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_./:=,+".contains(c))
                {
                    arg.clone()
                } else {
                    glib::shell_quote(arg).to_string_lossy().into_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn refresh(&self) {
        let mode = self.mode();
        self.mode_label.set_label(&format!(
            "Numeric: {mode:04o}      Symbolic: {}",
            symbolic(mode, self.is_dir)
        ));
        if let Some(run) = &self.run {
            self.syncing.set(true);
            run.set_active(mode & 0o100 != 0);
            run.set_inconsistent(mode & RUN != 0 && mode & 0o100 == 0);
            self.syncing.set(false);
        }
        let notice = if self.path.is_none() {
            None
        } else if !self.i_own_it() && !self.is_root() {
            Some(format!(
                "{} owns this item, so changing it takes an administrator. File Manager will ask for your password when you apply.",
                self.applied_owner.borrow()
            ))
        } else if self.owner_changed() && !self.is_root() {
            Some(
                "Giving an item to someone else takes an administrator. File Manager will ask for your password when you apply."
                    .to_owned(),
            )
        } else {
            None
        };
        self.notice.set_visible(notice.is_some());
        self.notice.set_label(notice.as_deref().unwrap_or_default());

        let show_commands = hints::enabled() && self.path.is_some();
        self.hint_box.set_visible(show_commands);
        if show_commands {
            let sudo = if self.needs_admin() { "sudo " } else { "" };
            let text = self
                .command_lines(true)
                .iter()
                .map(|line| format!("{sudo}{}", Self::quoted(line)))
                .collect::<Vec<_>>()
                .join("\n");
            self.commands.set_label(&text);
        }
    }

    /// Makes the changes, as administrator when need be. Errors are for
    /// showing to the user.
    pub async fn apply(&self) -> Result<(), String> {
        let changed = self.owner_changed()
            || self.group_changed()
            || self.mode() != self.applied.get()
            || self.recursive();
        if !changed {
            return Ok(());
        }
        if self.path.is_none() {
            // A network place: its own permissions, through GIO.
            self.file
                .set_attribute_uint32(
                    "unix::mode",
                    self.mode(),
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                )
                .map_err(|error| error.message().to_owned())?;
            self.applied.set(self.mode());
            return Ok(());
        }
        let lines = self.command_lines(false);
        let mut result = Err(String::new());
        if !self.needs_admin() {
            result = run_all(&lines).await;
        }
        if result.is_err() {
            // Not allowed, or something inside belongs to someone else.
            let script = lines
                .iter()
                .map(|line| Self::quoted(line))
                .collect::<Vec<_>>()
                .join(" && ");
            let elevated = vec!["pkexec".to_owned(), "/bin/sh".into(), "-c".into(), script];
            result = run(&elevated)
                .await
                .map_err(|(status, error)| match status {
                    126 | 127 => {
                        "The change wasn't made: administrator rights weren't given.".to_owned()
                    }
                    _ if error.is_empty() => "The change couldn't be made.".to_owned(),
                    _ => error,
                });
        }
        result?;
        self.applied.set(self.mode());
        self.applied_owner.replace(self.owner());
        self.applied_group.replace(self.group());
        if let Some(path) = &self.path
            && let Ok(meta) = std::fs::symlink_metadata(path)
        {
            self.owner_uid.set(meta.uid());
        }
        if let Some(recursive) = &self.recursive {
            recursive.set_active(false);
        }
        self.refresh();
        Ok(())
    }
}

async fn run_all(lines: &[Vec<String>]) -> Result<(), String> {
    for line in lines {
        run(line).await.map_err(|(_, error)| error)?;
    }
    Ok(())
}

/// Runs a command; on failure, its exit status and what it said.
async fn run(argv: &[String]) -> Result<(), (i32, String)> {
    let argv: Vec<&std::ffi::OsStr> = argv.iter().map(|arg| arg.as_ref()).collect();
    let process = gio::Subprocess::newv(
        &argv,
        gio::SubprocessFlags::STDERR_PIPE | gio::SubprocessFlags::STDOUT_SILENCE,
    )
    .map_err(|error| (-1, error.message().to_owned()))?;
    let (_, error) = process
        .communicate_utf8_future(None)
        .await
        .map_err(|error| (-1, error.message().to_owned()))?;
    if process.is_successful() {
        Ok(())
    } else {
        let error = error.map(|text| text.trim().to_owned()).unwrap_or_default();
        Err((process.exit_status(), error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbolic_modes() {
        assert_eq!(symbolic(0o755, false), "-rwxr-xr-x");
        assert_eq!(symbolic(0o4755, false), "-rwsr-xr-x");
        assert_eq!(symbolic(0o1777, true), "drwxrwxrwt");
        assert_eq!(symbolic(0o2640, false), "-rw-r-S---");
        assert_eq!(recursive_mode(0o755), "u=rwX,g=rX,o=rX");
        assert_eq!(recursive_mode(0o640), "u=rw,g=r,o=");
    }
}
