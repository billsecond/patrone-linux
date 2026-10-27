//! Task Manager's picture of what's running, renewed from each reading of
//! /proc: a row for every process, rows grouping them by app or service,
//! and a row for every signed-in account. Rows are kept from one reading to
//! the next, so lists keep their selection and what's expanded.
//!
//! Processes are grouped the way the desktop already groups them: every app
//! opened from the desktop runs in a systemd scope of its own, named after
//! its desktop entry (app-gnome-org.patrone.Terminal-1234.scope), and every
//! service in a unit of its own. A scope whose desktop entry is shown in
//! the app menus is an app; the account's other units are background
//! processes; everything else is a system process.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

use crate::format;
use crate::procs::{self, Process};
use crate::row::{self, APP, BACKGROUND, Row, SYSTEM};

/// Where a control group puts a process.
#[derive(Clone)]
pub struct Placement {
    /// The unit's control group: what the group is keyed by
    pub key: Rc<str>,
    /// Its unit (org.gnome.Shell@wayland.service)
    pub unit: String,
    /// The desktop entry an app scope names
    pub app_id: Option<String>,
    /// The account whose slice it's in (user-1000.slice)
    pub uid: Option<u32>,
    /// A service of the system's (system.slice/x.service) or an account's
    pub service: Option<(bool, String)>,
}

#[derive(Clone)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub icon: Option<gio::Icon>,
}

/// An account signed in, from logind.
#[derive(Clone, Default)]
pub struct Login {
    pub state: String,
    pub sessions: u32,
    pub remote: u32,
}

#[derive(Default, Clone, Copy)]
pub struct Totals {
    pub processes: usize,
    pub threads: u64,
}

pub struct Model {
    /// The Processes page: apps, background processes, system processes
    pub groups: gio::ListStore,
    /// The Details page: every process
    pub processes: gio::ListStore,
    /// The Users page
    pub users: gio::ListStore,
    /// The latest reading
    pub list: Vec<Process>,
    pub counts: [u32; 3],
    pub totals: Totals,
    /// Signed-in accounts, from logind (the Users page fills this in)
    pub logins: HashMap<u32, Login>,
    /// Each service's main process: (system, unit) → PID
    pub service_pids: HashMap<(bool, String), i32>,
    pub my_uid: u32,
    rows: HashMap<(i32, u64), Row>,
    group_rows: HashMap<Rc<str>, Row>,
    user_rows: HashMap<u32, Row>,
    placements: HashMap<Rc<str>, Placement>,
    apps: HashMap<String, Option<AppEntry>>,
}

impl Model {
    pub fn new(my_uid: u32) -> Self {
        Model {
            groups: gio::ListStore::new::<Row>(),
            processes: gio::ListStore::new::<Row>(),
            users: gio::ListStore::new::<Row>(),
            list: Vec::new(),
            counts: [0; 3],
            totals: Totals::default(),
            logins: HashMap::new(),
            service_pids: HashMap::new(),
            my_uid,
            rows: HashMap::new(),
            group_rows: HashMap::new(),
            user_rows: HashMap::new(),
            placements: HashMap::new(),
            apps: HashMap::new(),
        }
    }

    pub fn find(&self, pid: i32) -> Option<&Process> {
        self.list.iter().find(|process| process.pid == pid)
    }

    pub fn update(&mut self, list: Vec<Process>) {
        let mut threads = 0;

        // A row for every process
        let mut process_rows = Vec::with_capacity(list.len());
        for process in &list {
            threads += process.threads as u64;
            let row = self.rows.entry(process.key()).or_insert_with(|| {
                let row = Row::new(&format!("{}/{}", process.pid, process.start));
                row.set_pid(process.pid);
                row.set_start(process.start);
                row.set_pid_text(process.pid.to_string());
                row
            });
            fill_process(row, process);
            process_rows.push(row.clone());
        }
        let live: HashSet<(i32, u64)> = list.iter().map(Process::key).collect();
        self.rows.retain(|key, _| live.contains(key));

        // Groups, by unit
        let mut members: HashMap<Rc<str>, Vec<usize>> = HashMap::new();
        for (index, process) in list.iter().enumerate() {
            if process.kernel {
                continue;
            }
            let placement = self.place(&process.cgroup);
            members.entry(placement.key).or_default().push(index);
        }
        fold_spawned_scopes(&list, &mut members);
        let mut counts = [0u32; 3];
        let mut group_rows = Vec::with_capacity(members.len());
        let mut service_pids = HashMap::new();
        for (key, indexes) in &members {
            let placement = self
                .placements
                .get(key)
                .cloned()
                .unwrap_or_else(|| place(key));
            let app = placement.app_id.as_deref().and_then(|id| self.app(id));
            let kind = if app.is_some() {
                APP
            } else if placement.uid == Some(self.my_uid) {
                BACKGROUND
            } else {
                SYSTEM
            };
            counts[kind as usize] += 1;
            let row = self
                .group_rows
                .entry(key.clone())
                .or_insert_with(|| Row::parent(key))
                .clone();
            // The first process, by PID, names the group and is the
            // service's main process
            let main = indexes
                .iter()
                .map(|&i| &list[i])
                .min_by_key(|process| process.pid)
                .unwrap();
            if let Some(service) = &placement.service {
                service_pids.insert(service.clone(), main.pid);
            }
            let name = match &app {
                Some(app) => app.name.clone(),
                None => main.name.to_string(),
            };
            let icon = match &app {
                Some(app) => app.icon.clone(),
                None => Some(
                    gio::ThemedIcon::new(if kind == SYSTEM {
                        "emblem-system-symbolic"
                    } else {
                        "application-x-executable-symbolic"
                    })
                    .upcast(),
                ),
            };
            fill_group(&row, &list, indexes, &name, icon.as_ref(), kind);
            row.put_text(
                "app-id",
                app.as_ref().map(|app| app.id.as_str()).unwrap_or(""),
            );
            row.put_text("user", &main.user);
            row.set_pid(main.pid);
            let children: Vec<Row> = indexes
                .iter()
                .filter_map(|&i| self.rows.get(&list[i].key()).cloned())
                .collect();
            for child in &children {
                if child.kind() != kind {
                    child.set_kind(kind);
                }
                // An app's processes show its icon
                child.put_icon(icon.as_ref());
            }
            if let Some(store) = row.children() {
                row::sync_store(&store, &children);
            }
            group_rows.push(row);
        }
        self.group_rows.retain(|key, _| members.contains_key(key));
        row::sync_store(&self.groups, &group_rows);
        row::sync_store(&self.processes, &process_rows);
        self.counts = counts;
        self.service_pids = service_pids;
        self.totals = Totals {
            processes: list.len(),
            threads,
        };

        self.update_users(&list, &members);
        self.list = list;
    }

    /// A row for each account with a session, holding its groups.
    fn update_users(&mut self, list: &[Process], members: &HashMap<Rc<str>, Vec<usize>>) {
        let mut uids: Vec<u32> = self.logins.keys().copied().collect();
        if uids.is_empty() {
            // Before logind answers: the accounts with a slice of their own
            for key in members.keys() {
                if let Some(uid) = self.placements.get(key).and_then(|placement| placement.uid)
                    && !uids.contains(&uid)
                {
                    uids.push(uid);
                }
            }
        }
        let mut user_rows = Vec::new();
        for uid in uids {
            let row = self
                .user_rows
                .entry(uid)
                .or_insert_with(|| {
                    let row = Row::parent(&uid.to_string());
                    let (login, full) = procs::account(uid);
                    row.set_name(if full.is_empty() {
                        login.clone()
                    } else {
                        format!("{full} ({login})")
                    });
                    row.set_user(login);
                    // Not its PID: an account row stands for no process
                    row.set_extra_value(uid as f64);
                    row.set_icon(Some(
                        &gio::ThemedIcon::new("avatar-default-symbolic").upcast::<gio::Icon>(),
                    ));
                    row
                })
                .clone();
            let mine: Vec<&Process> = list
                .iter()
                .filter(|process| process.uid == uid && !process.kernel)
                .collect();
            fill_totals(&row, &mine);
            let login = self.logins.get(&uid).cloned().unwrap_or_default();
            row.put_text(
                "status",
                match login.state.as_str() {
                    "active" => "Active",
                    "online" => "Signed in",
                    "lingering" => "Running in the background",
                    "closing" => "Signing out",
                    _ => "",
                },
            );
            let sessions = match (login.sessions, login.remote) {
                (0, _) => String::new(),
                (n, 0) => format!("{n} here"),
                (n, r) if n == r => format!("{r} remote"),
                (n, r) => format!("{} here, {r} remote", n - r),
            };
            row.put_text("description", &sessions);
            let groups: Vec<Row> = members
                .keys()
                .filter(|key| {
                    self.placements
                        .get(*key)
                        .and_then(|placement| placement.uid)
                        == Some(uid)
                })
                .filter_map(|key| self.group_rows.get(key).cloned())
                .collect();
            if let Some(store) = row.children() {
                row::sync_store(&store, &groups);
            }
            user_rows.push(row);
        }
        let shown: HashSet<String> = user_rows.iter().map(|row| row.key()).collect();
        self.user_rows.retain(|_, row| shown.contains(&row.key()));
        row::sync_store(&self.users, &user_rows);
    }

    fn place(&mut self, cgroup: &Rc<str>) -> Placement {
        if let Some(placement) = self.placements.get(cgroup) {
            return placement.clone();
        }
        let placement = place(cgroup);
        // The group's key maps to itself too, for looking up by key
        self.placements
            .entry(placement.key.clone())
            .or_insert_with(|| placement.clone());
        self.placements.insert(cgroup.clone(), placement.clone());
        placement
    }

    /// The app a desktop entry names, if it's one people open (shown in the
    /// app menus).
    pub fn app(&mut self, id: &str) -> Option<AppEntry> {
        if let Some(app) = self.apps.get(id) {
            return app.clone();
        }
        let app = crate::desktop::find(id)
            .filter(|entry| entry.shown)
            .map(|entry| AppEntry {
                id: id.to_string(),
                name: entry.name,
                icon: entry.icon,
            });
        self.apps.insert(id.to_string(), app.clone());
        app
    }
}

/// Terminals start each shell in a scope of its own beside theirs
/// (vte-spawn-UUID.scope): those processes join the group of the process
/// that started them, so a terminal's shells and what runs in them show
/// under the terminal.
fn fold_spawned_scopes(list: &[Process], members: &mut HashMap<Rc<str>, Vec<usize>>) {
    let mut group_of: HashMap<i32, Rc<str>> = HashMap::new();
    for (key, indexes) in members.iter() {
        for &index in indexes {
            group_of.insert(list[index].pid, key.clone());
        }
    }
    let spawned: Vec<Rc<str>> = members
        .keys()
        .filter(|key| key.rsplit('/').next().is_some_and(|unit| unit.starts_with("vte-spawn-")))
        .cloned()
        .collect();
    for key in spawned {
        let Some(indexes) = members.get(&key) else { continue };
        let Some(first) = indexes.iter().map(|&index| &list[index]).min_by_key(|process| process.pid) else {
            continue;
        };
        let Some(parent) = group_of.get(&first.ppid).cloned() else { continue };
        if parent == key || !members.contains_key(&parent) {
            continue;
        }
        let moved = members.remove(&key).unwrap_or_default();
        members.entry(parent).or_default().extend(moved);
    }
}

fn fill_process(row: &Row, process: &Process) {
    row.put_text("name", &process.name);
    row.put_text("user", &process.user);
    row.put_text(
        "command",
        if process.cmdline.is_empty() {
            &process.name
        } else {
            &process.cmdline
        },
    );
    row.put_text(
        "status",
        match process.state {
            b'T' | b't' => "Suspended",
            b'Z' => "Ended",
            _ => "",
        },
    );
    row.put_text(
        "description",
        match process.state {
            b'T' | b't' => "Suspended",
            b'Z' => "Ended",
            b'D' => "Waiting",
            b'I' => "Idle",
            _ => "Running",
        },
    );
    row.put_number("cpu", process.cpu);
    row.put_text("cpu-text", &format::percent(process.cpu));
    row.put_text("cpu-detail", &format::percent_padded(process.cpu));
    row.put_number("memory", process.memory as f64);
    row.put_text(
        "memory-text",
        &if process.kernel {
            String::new()
        } else {
            format::megabytes(process.memory as f64)
        },
    );
    row.put_text("memory-detail", &format::kilobytes(process.memory));
    row.put_number("disk", process.disk.unwrap_or(0.0));
    row.put_text("disk-text", &format::disk_rate(process.disk));
    row.put_number("network", process.network.unwrap_or(0.0));
    row.put_text("network-text", &format::network_rate(process.network));
    if row.threads() != process.threads {
        row.set_threads(process.threads);
    }
    if !row.leaf() {
        row.set_leaf(true);
    }
}

fn fill_group(
    row: &Row,
    list: &[Process],
    indexes: &[usize],
    name: &str,
    icon: Option<&gio::Icon>,
    kind: u32,
) {
    let processes: Vec<&Process> = indexes.iter().map(|&i| &list[i]).collect();
    let name = if processes.len() > 1 {
        format!("{name} ({})", processes.len())
    } else {
        name.to_string()
    };
    row.put_text("name", &name);
    row.put_icon(icon);
    if row.kind() != kind {
        row.set_kind(kind);
    }
    let leaf = processes.len() < 2;
    if row.leaf() != leaf {
        row.set_leaf(leaf);
    }
    let suspended = processes
        .iter()
        .all(|process| matches!(process.state, b'T' | b't'));
    row.put_text("status", if suspended { "Suspended" } else { "" });
    fill_totals(row, &processes);
    let mut pids: Vec<(i32, u64)> = processes.iter().map(|process| process.key()).collect();
    pids.sort();
    row.set_processes(pids);
}

/// A group's or an account's processes, added up.
fn fill_totals(row: &Row, processes: &[&Process]) {
    let cpu: f64 = processes.iter().map(|process| process.cpu).sum();
    let memory: u64 = processes.iter().map(|process| process.memory).sum();
    let readable = |values: Vec<Option<f64>>| -> Option<f64> {
        let known: Vec<f64> = values.into_iter().flatten().collect();
        (!known.is_empty()).then(|| known.iter().sum())
    };
    let disk = readable(processes.iter().map(|process| process.disk).collect());
    let network = readable(processes.iter().map(|process| process.network).collect());
    row.put_number("cpu", cpu);
    row.put_text("cpu-text", &format::percent(cpu.min(100.0)));
    row.put_number("memory", memory as f64);
    row.put_text("memory-text", &format::megabytes(memory as f64));
    row.put_number("disk", disk.unwrap_or(0.0));
    row.put_text("disk-text", &format::disk_rate(disk));
    row.put_number("network", network.unwrap_or(0.0));
    row.put_text("network-text", &format::network_rate(network));
    let threads: u32 = processes.iter().map(|process| process.threads).sum();
    if row.threads() != threads {
        row.set_threads(threads);
    }
}

/// Where a control group puts a process: the last unit in its path
/// (`…/app.slice/app-gnome-org.patrone.Terminal-1234.scope`).
pub fn place(cgroup: &str) -> Placement {
    let parts: Vec<&str> = cgroup.split('/').collect();
    let last_unit = parts
        .iter()
        .rposition(|part| part.ends_with(".scope") || part.ends_with(".service"));
    let (key, unit) = match last_unit {
        Some(index) => (parts[..=index].join("/"), parts[index].to_string()),
        None => (
            cgroup.to_string(),
            parts.last().copied().unwrap_or("").to_string(),
        ),
    };
    let uid = parts.iter().find_map(|part| {
        part.strip_prefix("user-")
            .and_then(|rest| rest.strip_suffix(".slice"))
            .and_then(|uid| uid.parse::<u32>().ok())
    });
    let service = unit.strip_suffix(".service").map(|_| {
        let system = parts.get(1) == Some(&"system.slice");
        (system, unit.clone())
    });
    Placement {
        key: Rc::from(key.as_str()),
        app_id: app_id(&unit),
        unit,
        uid,
        service,
    }
}

/// The desktop entry an app's unit is named after:
/// `app[-<launcher>]-<id>-<random>.scope`, `app[-<launcher>]-<id>[@<random>].service`
/// (dashes in the id escaped as `\x2d`), or a snap's
/// `snap.<snap>.<app>-<random>.scope`.
pub fn app_id(unit: &str) -> Option<String> {
    if let Some(stem) = unit
        .strip_prefix("app-")
        .and_then(|rest| rest.strip_suffix(".scope"))
    {
        let parts: Vec<&str> = stem.split('-').collect();
        return (parts.len() >= 2).then(|| unescape(parts[parts.len() - 2]));
    }
    if let Some(stem) = unit
        .strip_prefix("app-")
        .and_then(|rest| rest.strip_suffix(".service"))
    {
        let stem = stem.split('@').next().unwrap_or(stem);
        return stem.rsplit('-').next().map(unescape);
    }
    if let Some(stem) = unit
        .strip_prefix("snap.")
        .and_then(|rest| rest.strip_suffix(".scope"))
    {
        let (snap, rest) = stem.split_once('.')?;
        let app = rest.split('-').next()?;
        return Some(format!("{snap}_{app}"));
    }
    None
}

/// systemd's escapes in unit names: `\x2d` is '-'.
pub fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && index + 3 < bytes.len()
            && bytes[index + 1] == b'x'
            && let Ok(value) = u8::from_str_radix(&text[index + 2..index + 4], 16)
        {
            out.push(value);
            index += 4;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The section headers: "Apps (5)".
pub fn section_title(kind: u32, counts: &[u32; 3]) -> String {
    let name = match kind {
        APP => "Apps",
        BACKGROUND => "Background processes",
        _ => "System processes",
    };
    format!(
        "{name} ({})",
        counts.get(kind as usize).copied().unwrap_or(0)
    )
}

/// A group's name without its count of processes, for messages:
/// "File Manager (3)" → "File Manager".
pub fn plain_name(row: &Row) -> String {
    let name = row.name();
    match name.rsplit_once(" (") {
        Some((base, count))
            if count
                .trim_end_matches(')')
                .chars()
                .all(|c| c.is_ascii_digit())
                && row.children().is_some() =>
        {
            base.to_string()
        }
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_scopes() {
        assert_eq!(
            app_id("app-gnome-org.patrone.Terminal-324259.scope").as_deref(),
            Some("org.patrone.Terminal")
        );
        assert_eq!(
            app_id(r"app-gnome-gnome\x2dpatrone\x2dpanel-325676.scope").as_deref(),
            Some("gnome-patrone-panel")
        );
        assert_eq!(
            app_id("app-flatpak-org.example.App-1234.scope").as_deref(),
            Some("org.example.App")
        );
        assert_eq!(
            app_id(r"app-org.gnome.Evolution\x2dalarm\x2dnotify@autostart.service").as_deref(),
            Some("org.gnome.Evolution-alarm-notify")
        );
        assert_eq!(
            app_id("snap.chromium.chromium-0b1c2d3e-aaaa-bbbb-cccc-0123456789ab.scope").as_deref(),
            Some("chromium_chromium")
        );
        assert_eq!(app_id("session-819.scope"), None);
        assert_eq!(app_id("NetworkManager.service"), None);
    }

    #[test]
    fn placements() {
        let placement = place(
            "/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-org.patrone.Terminal-1.scope",
        );
        assert_eq!(placement.uid, Some(1000));
        assert_eq!(placement.app_id.as_deref(), Some("org.patrone.Terminal"));
        assert_eq!(
            &*placement.key,
            "/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-org.patrone.Terminal-1.scope"
        );

        let placement = place("/system.slice/NetworkManager.service");
        assert_eq!(placement.uid, None);
        assert_eq!(
            placement.service,
            Some((true, "NetworkManager.service".to_string()))
        );

        let placement = place(
            "/user.slice/user-1000.slice/user@1000.service/session.slice/org.gnome.Shell@wayland.service",
        );
        assert_eq!(
            placement.service,
            Some((false, "org.gnome.Shell@wayland.service".to_string()))
        );

        let placement = place("/");
        assert_eq!(&*placement.key, "/");
    }
}
