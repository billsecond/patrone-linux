//! The browsers the chooser offers, all from Flathub, which Patrone Software
//! installs from (it leaves out the Snap Store); finding them once they're
//! installed; and making one the default for the web.

use std::path::{Path, PathBuf};

use gtk::prelude::*;
use gio_unix::DesktopAppInfo;
use gtk::{gio, glib};

/// The link and file types a web browser is the default for: those GNOME
/// Settings sets for its default web browser. The taskbar's Web browser
/// slot opens the default for the first three.
pub const TYPES: &[&str] = &[
    "x-scheme-handler/https",
    "x-scheme-handler/http",
    "text/html",
    "application/xhtml+xml",
];

/// Where Flatpak keeps Flathub's catalog (the system-wide remote that
/// patrone-software adds): its list of apps, and their icons.
const CATALOG: &str = "/var/lib/flatpak/appstream/flathub";

pub struct Browser {
    pub name: &'static str,
    /// One line about it
    pub about: &'static str,
    /// Its app ID on Flathub
    pub id: &'static str,
    /// The desktop file IDs it can be installed as: Flathub's first, then
    /// Ubuntu's snaps and the makers' own .deb packages, for a browser
    /// installed some other way. /etc/xdg/mimeapps.list
    /// (patrone-default-apps) lists them too.
    pub desktop_ids: &'static [&'static str],
}

/// The browsers on offer, in the order shown: the user's three first
/// (2026-10-28: "Chromium, Fire Fox, Brave, etc."), then the other
/// well-known ones.
pub const BROWSERS: &[Browser] = &[
    Browser {
        name: "Chromium",
        about: "The open-source browser that Google Chrome, Microsoft Edge and Brave are built on.",
        id: "org.chromium.Chromium",
        desktop_ids: &[
            "org.chromium.Chromium.desktop",
            "chromium_chromium.desktop",
            "chromium.desktop",
        ],
    },
    Browser {
        name: "Firefox",
        about: "Mozilla's independent browser, with tracking protection turned on.",
        id: "org.mozilla.firefox",
        desktop_ids: &[
            "org.mozilla.firefox.desktop",
            "firefox_firefox.desktop",
            "firefox.desktop",
        ],
    },
    Browser {
        name: "Brave",
        about: "Built on Chromium, and blocks ads and trackers by itself.",
        id: "com.brave.Browser",
        desktop_ids: &[
            "com.brave.Browser.desktop",
            "brave_brave.desktop",
            "brave-browser.desktop",
        ],
    },
    Browser {
        name: "Google Chrome",
        about: "Google's browser, which syncs with your Google account.",
        id: "com.google.Chrome",
        desktop_ids: &["com.google.Chrome.desktop", "google-chrome.desktop"],
    },
    Browser {
        name: "Microsoft Edge",
        about: "Microsoft's browser, which syncs with your Microsoft account.",
        id: "com.microsoft.Edge",
        desktop_ids: &["com.microsoft.Edge.desktop", "microsoft-edge.desktop"],
    },
    Browser {
        name: "Vivaldi",
        about: "A browser you can arrange your own way, with tab stacks, notes and mail built in.",
        id: "com.vivaldi.Vivaldi",
        desktop_ids: &["com.vivaldi.Vivaldi.desktop", "vivaldi-stable.desktop"],
    },
    Browser {
        name: "Opera",
        about: "Built on Chromium, with a free VPN and messengers in its sidebar.",
        id: "com.opera.Opera",
        desktop_ids: &["com.opera.Opera.desktop", "opera.desktop"],
    },
    Browser {
        name: "LibreWolf",
        about: "Firefox with stricter privacy settings and no telemetry.",
        id: "io.gitlab.librewolf-community",
        desktop_ids: &["io.gitlab.librewolf-community.desktop", "librewolf.desktop"],
    },
    Browser {
        name: "Zen Browser",
        about: "A calm browser built on Firefox, with its tabs down the side.",
        id: "app.zen_browser.zen",
        desktop_ids: &["app.zen_browser.zen.desktop"],
    },
    Browser {
        name: "GNOME Web",
        about: "GNOME's own simple, lightweight browser.",
        id: "org.gnome.Epiphany",
        desktop_ids: &["org.gnome.Epiphany.desktop"],
    },
];

impl Browser {
    /// What Patrone Software's --install takes: the app's AppStream data ID
    /// on the system's Flathub remote (scope, bundle, origin, ID, branch).
    pub fn data_id(&self) -> String {
        format!("system/flatpak/flathub/{}/stable", self.id)
    }

    /// The browser's launcher, if it's installed. Hidden launchers don't
    /// count: Ubuntu's chromium-browser stub has one that only installs
    /// Chromium's snap.
    pub fn installed(&self) -> Option<DesktopAppInfo> {
        self.desktop_ids
            .iter()
            .filter_map(|id| DesktopAppInfo::new(id))
            .find(|app| app.should_show())
    }

    /// Its icon from Flathub's catalog, if Flatpak has fetched it.
    pub fn icon(&self) -> Option<PathBuf> {
        ["128x128", "64x64"]
            .iter()
            .map(|size| {
                Path::new(CATALOG)
                    .join(std::env::consts::ARCH)
                    .join("active/icons")
                    .join(size)
                    .join(format!("{}.png", self.id))
            })
            .find(|path| path.exists())
    }
}

/// Whether Flathub's catalog is on this machine. Patrone Software can't find
/// an app on Flathub without it, and a new system has none until Patrone
/// Software first refreshes, shortly after the first sign-in.
pub fn catalog_ready() -> bool {
    let active = Path::new(CATALOG)
        .join(std::env::consts::ARCH)
        .join("active");
    active.join("appstream.xml.gz").exists() || active.join("appstream.xml").exists()
}

/// Browsers installed some other way that aren't on the list: apps that say
/// they're a web browser.
pub fn others() -> Vec<DesktopAppInfo> {
    gio::AppInfo::all()
        .into_iter()
        .filter_map(|app| app.downcast::<DesktopAppInfo>().ok())
        .filter(|app| {
            app.should_show()
                && app
                    .categories()
                    .is_some_and(|categories| categories.split(';').any(|c| c == "WebBrowser"))
                && !app.id().is_some_and(|id| {
                    BROWSERS
                        .iter()
                        .any(|browser| browser.desktop_ids.contains(&id.as_str()))
                })
        })
        .collect()
}

/// Makes the app this account's default for web links and pages, as
/// Settings → Apps → Default Apps and the taskbar's Web browser menu do:
/// for https always, and for the other types if it says it opens them.
pub fn make_default(app: &gio::AppInfo) -> Result<(), glib::Error> {
    let supported = app.supported_types();
    for (i, content_type) in TYPES.iter().enumerate() {
        if i == 0 || supported.is_empty() || supported.iter().any(|t| t == content_type) {
            app.set_as_default_for_type(content_type)?;
        }
    }
    Ok(())
}

/// What a link is, in a few words: its site, or a file's name.
pub fn describe(uri: &str) -> String {
    if let Ok(parsed) = glib::Uri::parse(uri, glib::UriFlags::NONE)
        && let Some(host) = parsed.host().filter(|host| !host.is_empty())
    {
        return host.trim_start_matches("www.").to_string();
    }
    gio::File::for_uri(uri)
        .basename()
        .map(|name| name.display().to_string())
        .unwrap_or_else(|| uri.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_ids() {
        assert_eq!(
            BROWSERS[1].data_id(),
            "system/flatpak/flathub/org.mozilla.firefox/stable"
        );
    }

    #[test]
    fn flathub_launcher_first() {
        for browser in BROWSERS {
            assert_eq!(browser.desktop_ids[0], format!("{}.desktop", browser.id));
        }
    }

    #[test]
    fn links_described() {
        assert_eq!(describe("https://www.example.com/a?b=c"), "example.com");
        assert_eq!(describe("http://help.gnome.org/"), "help.gnome.org");
        assert_eq!(describe("file:///home/me/page.html"), "page.html");
    }
}
