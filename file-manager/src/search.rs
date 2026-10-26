//! Searching a folder and its subfolders, as the search box does. Results
//! stream into the list as they're found. Names match anywhere, ignoring
//! case, and `*` and `?` work as wildcards; with "Always search file names
//! and contents" on, text files are searched inside too.

use std::collections::VecDeque;

use gtk::{gio, glib, prelude::*};

use crate::config;
use crate::folderview::ATTRIBUTES;

pub struct Options {
    pub subfolders: bool,
    pub hidden: bool,
    pub system_folders: bool,
    pub contents: bool,
    pub archives: bool,
}

impl Options {
    pub fn from_settings() -> Options {
        Options {
            subfolders: config::flag("search-subfolders"),
            hidden: config::flag("show-hidden"),
            system_folders: config::flag("search-system-folders"),
            contents: config::flag("search-contents"),
            archives: config::flag("search-archives"),
        }
    }
}

/// Folders that are never searched: they're not files, or they're huge.
const NEVER: &[&str] = &["/proc", "/sys", "/dev", "/run", "/snap", "/tmp/.X11-unix"];
/// Skipped unless "Include system folders" is on.
const SYSTEM: &[&str] = &[
    "/usr", "/etc", "/var", "/opt", "/boot", "/lib", "/lib64", "/bin", "/sbin", "/srv",
];

/// A query that matches names.
pub struct Matcher {
    words: Vec<String>,
    pattern: Option<String>,
}

impl Matcher {
    pub fn new(query: &str) -> Matcher {
        let query = query.trim().to_lowercase();
        if query.contains('*') || query.contains('?') {
            Matcher {
                words: Vec::new(),
                pattern: Some(query),
            }
        } else {
            Matcher {
                words: query.split_whitespace().map(str::to_owned).collect(),
                pattern: None,
            }
        }
    }

    pub fn matches(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        match &self.pattern {
            Some(pattern) => glob(pattern.as_bytes(), name.as_bytes()),
            None => {
                !self.words.is_empty() && self.words.iter().all(|word| name.contains(word.as_str()))
            }
        }
    }

    pub fn matches_text(&self, text: &str) -> bool {
        let text = text.to_lowercase();
        match &self.pattern {
            Some(pattern) => text.contains(pattern.trim_matches('*')),
            None => self.words.iter().all(|word| text.contains(word.as_str())),
        }
    }
}

/// `*` and `?` wildcards over the whole name.
fn glob(pattern: &[u8], name: &[u8]) -> bool {
    let (mut p, mut n) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            mark = n;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            n = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

/// Searches `root`, adding each match to `results`. Returns when done or
/// cancelled.
pub async fn run(
    root: gio::File,
    query: String,
    results: gio::ListStore,
    options: Options,
    cancellable: gio::Cancellable,
) {
    let matcher = Matcher::new(&query);
    let mut queue = VecDeque::from([root.clone()]);
    let root_is_system = root
        .path()
        .is_some_and(|path| path == std::path::Path::new("/"));
    while let Some(folder) = queue.pop_front() {
        if cancellable.is_cancelled() {
            return;
        }
        let Ok(enumerator) = folder
            .enumerate_children_future(
                ATTRIBUTES,
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                glib::Priority::LOW,
            )
            .await
        else {
            continue;
        };
        loop {
            if cancellable.is_cancelled() {
                return;
            }
            let Ok(infos) = enumerator.next_files_future(200, glib::Priority::LOW).await else {
                break;
            };
            if infos.is_empty() {
                break;
            }
            let mut found = Vec::new();
            for info in infos {
                let name = info.name();
                let child = folder.child(&name);
                info.set_attribute_object("standard::file", &child);
                info.set_attribute_boolean("pfm::search-result", true);
                let hidden = info.is_hidden() || info.is_backup();
                if hidden && !options.hidden {
                    continue;
                }
                let is_dir = info.file_type() == gio::FileType::Directory;
                let matches = matcher.matches(&info.display_name())
                    || (options.contents && !is_dir && in_text_file(&child, &info, &matcher).await)
                    || (options.archives && is_zip(&info) && zip_has_match(&child, &matcher).await);
                if matches {
                    found.push(info.clone());
                }
                if is_dir && options.subfolders && !info.is_symlink() {
                    if let Some(path) = child.path() {
                        let path = path.to_string_lossy();
                        if NEVER.iter().any(|never| path == *never) {
                            continue;
                        }
                        if root_is_system
                            && !options.system_folders
                            && SYSTEM.iter().any(|system| path == *system)
                        {
                            continue;
                        }
                    }
                    queue.push_back(child);
                }
            }
            if !found.is_empty() {
                results.extend_from_slice(&found);
            }
        }
    }
}

fn is_zip(info: &gio::FileInfo) -> bool {
    info.content_type()
        .is_some_and(|ct| ct == "application/zip")
}

async fn in_text_file(file: &gio::File, info: &gio::FileInfo, matcher: &Matcher) -> bool {
    let is_text = info
        .content_type()
        .is_some_and(|ct| gio::content_type_is_a(&ct, "text/plain"));
    if !is_text || info.size() > 4 * 1024 * 1024 {
        return false;
    }
    match file.load_contents_future().await {
        Ok((bytes, _)) => matcher.matches_text(&String::from_utf8_lossy(&bytes)),
        Err(_) => false,
    }
}

async fn zip_has_match(file: &gio::File, matcher: &Matcher) -> bool {
    let Some(path) = file.path() else {
        return false;
    };
    let names = gio::spawn_blocking(move || crate::archive::zip_entry_names(&path))
        .await
        .unwrap_or_default();
    names.iter().any(|name| {
        let base = name
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(name);
        matcher.matches(base)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_match_anywhere_ignoring_case() {
        let matcher = Matcher::new("tax 2026");
        assert!(matcher.matches("Taxes-2026.pdf"));
        assert!(!matcher.matches("Taxes-2025.pdf"));
    }

    #[test]
    fn wildcards_match_the_whole_name() {
        let matcher = Matcher::new("*.txt");
        assert!(matcher.matches("notes.TXT"));
        assert!(!matcher.matches("notes.txt.bak"));
        assert!(Matcher::new("file?.md").matches("file1.md"));
        assert!(!Matcher::new("file?.md").matches("file10.md"));
    }
}
