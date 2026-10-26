//! .deb packages: what one is (dpkg-deb), whether it's installed already
//! (dpkg-query), and what installing it would change (apt-get's simulation,
//! which needs no administrator).

use std::path::Path;
use std::process::Command;

use crate::util;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Deb {
    pub package: String,
    pub version: String,
    pub architecture: String,
    pub maintainer: String,
    pub installed_size_kb: Option<u64>,
    /// The description's first line.
    pub summary: String,
    /// The rest of the description, as paragraphs.
    pub description: String,
    pub homepage: Option<String>,
    pub section: String,
}

/// Reads the package's control fields.
pub fn read(path: &Path) -> Result<Deb, String> {
    let path = path.to_string_lossy();
    let text = util::output("dpkg-deb", &["--field", &path])
        .ok_or_else(|| "This file isn't a .deb package dpkg can read.".to_string())?;
    parse_control(&text)
}

/// Parses control fields ("Name: value", with continuation lines).
pub fn parse_control(text: &str) -> Result<Deb, String> {
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some((_, value)) = fields.last_mut() {
                value.push('\n');
                value.push_str(line);
            }
        } else if let Some((name, value)) = line.split_once(':') {
            fields.push((name.trim().to_lowercase(), value.trim().to_string()));
        }
    }
    let field = |name: &str| {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.clone())
    };
    let package = field("package").ok_or("The package has no name.")?;
    let description = field("description").unwrap_or_default();
    let (summary, long) = description.split_once('\n').unwrap_or((&description, ""));
    Ok(Deb {
        package,
        version: field("version").unwrap_or_default(),
        architecture: field("architecture").unwrap_or_default(),
        maintainer: field("maintainer").unwrap_or_default(),
        installed_size_kb: field("installed-size").and_then(|size| size.parse().ok()),
        summary: summary.trim().to_string(),
        description: paragraphs(long),
        homepage: field("homepage"),
        section: field("section").unwrap_or_default(),
    })
}

/// A long description as text: lines of a paragraph joined, " ." as the
/// break between paragraphs, and lines indented further kept as they are.
fn paragraphs(long: &str) -> String {
    let mut out = String::new();
    let mut joining = false;
    for line in long.lines() {
        let line = line.strip_prefix(' ').unwrap_or(line);
        if line.trim() == "." {
            out.push_str("\n\n");
            joining = false;
        } else if line.starts_with(' ') || line.starts_with('\t') {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(line.trim_end());
            out.push('\n');
            joining = false;
        } else {
            if joining {
                out.push(' ');
            }
            out.push_str(line.trim());
            joining = true;
        }
    }
    out.trim().to_string()
}

/// The package against what's installed.
#[derive(Debug, Clone, PartialEq)]
pub enum Installed {
    No,
    /// This version is installed.
    Same,
    /// An older version (given) is installed: this updates it.
    Older(String),
    /// A newer version (given) is installed: this goes back.
    Newer(String),
}

pub fn installed(deb: &Deb) -> Installed {
    let Some(text) = util::output(
        "dpkg-query",
        &["-W", "-f", "${db:Status-Status} ${Version}\n", &deb.package],
    ) else {
        return Installed::No;
    };
    let Some(version) = text
        .lines()
        .find_map(|line| line.strip_prefix("installed "))
        .map(str::to_string)
    else {
        return Installed::No;
    };
    let compare = |relation: &str| {
        Command::new("dpkg")
            .args(["--compare-versions", &version, relation, &deb.version])
            .status()
            .is_ok_and(|status| status.success())
    };
    if compare("lt") {
        Installed::Older(version)
    } else if compare("gt") {
        Installed::Newer(version)
    } else {
        Installed::Same
    }
}

/// What else installing the package changes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changes {
    /// Packages it needs that aren't installed.
    pub installs: Vec<String>,
    /// Installed packages updated (or changed) along with it.
    pub upgrades: Vec<String>,
    /// Installed packages it removes.
    pub removes: Vec<String>,
}

/// Asks apt what installing the package would do, without doing it.
pub fn simulate(path: &Path, package: &str) -> Result<Changes, String> {
    let output = Command::new("apt-get")
        .args(["--simulate", "--quiet", "install"])
        .arg(path)
        .env("LC_ALL", "C.UTF-8")
        .output()
        .map_err(|error| format!("apt-get couldn't run: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(parse_simulation(&stdout, package))
    } else {
        Err(simulation_error(&stdout, &stderr))
    }
}

/// Reads apt-get's simulation: "Inst NAME [OLD] (NEW ...)", "Remv NAME [OLD]".
pub fn parse_simulation(text: &str, package: &str) -> Changes {
    let mut changes = Changes::default();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let (Some(action), Some(name)) = (words.next(), words.next()) else {
            continue;
        };
        let name = name.split(':').next().unwrap_or(name).to_string();
        let upgrade = words.next().is_some_and(|word| word.starts_with('['));
        match action {
            "Inst" if name == package => {}
            "Inst" if upgrade => changes.upgrades.push(name),
            "Inst" => changes.installs.push(name),
            "Remv" => changes.removes.push(name),
            _ => {}
        }
    }
    changes
}

/// Why apt says the package can't be installed, in its words: its errors,
/// and the packages it can't find or fit.
pub fn simulation_error(stdout: &str, stderr: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut unmet = false;
    for line in stdout.lines() {
        if line.starts_with("The following packages have unmet dependencies") {
            unmet = true;
        } else if unmet && line.starts_with(' ') {
            lines.push(line.trim().to_string());
        } else {
            unmet = false;
        }
    }
    let unmet_listed = !lines.is_empty();
    for line in stderr.lines() {
        if let Some(error) = line.strip_prefix("E: ") {
            // After the unmet dependencies, apt's last word (and its
            // solver's reasoning, which follows) says nothing new.
            let summary = error.starts_with("Unable to correct problems")
                || error.starts_with("Unable to satisfy dependencies");
            if !(summary && unmet_listed) {
                lines.push(error.trim().trim_end_matches(':').to_string());
            }
        }
    }
    if lines.is_empty() {
        "apt can't install this package.".into()
    } else {
        lines.join("\n")
    }
}

/// The package's apps, once it's installed: its launcher entries.
pub fn launchers(package: &str) -> Vec<String> {
    util::output("dpkg-query", &["--listfiles", package])
        .map(|text| {
            text.lines()
                .filter(|line| {
                    line.starts_with("/usr/share/applications/") && line.ends_with(".desktop")
                })
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTROL: &str = "\
Package: patrone-file-manager
Version: 0.2.4
Architecture: amd64
Maintainer: William Daugherty <31995458+billsecond@users.noreply.github.com>
Installed-Size: 2578
Depends: libc6 (>= 2.39), gvfs
Section: utils
Homepage: https://example.org/fm
Description: file manager for Patrone Linux
 Patrone File Manager is the file manager of Patrone Linux, in the image of
 Windows 11's File Explorer.
 .
 It works with the Recycle Bin.
 .
   verbatim line
";

    #[test]
    fn control_fields() {
        let deb = parse_control(CONTROL).unwrap();
        assert_eq!(deb.package, "patrone-file-manager");
        assert_eq!(deb.version, "0.2.4");
        assert_eq!(deb.architecture, "amd64");
        assert_eq!(deb.installed_size_kb, Some(2578));
        assert_eq!(deb.summary, "file manager for Patrone Linux");
        assert_eq!(deb.homepage.as_deref(), Some("https://example.org/fm"));
        assert_eq!(
            deb.description,
            "Patrone File Manager is the file manager of Patrone Linux, in the image of \
             Windows 11's File Explorer.\n\nIt works with the Recycle Bin.\n\n  verbatim line"
        );
        assert!(parse_control("Version: 1\n").is_err());
    }

    #[test]
    fn simulation() {
        let text = "\
NOTE: This is only a simulation!
The following NEW packages will be installed:
  libfoo1 hello
Inst libfoo1 (1.0 Ubuntu:26.04/resolute [amd64])
Inst libbar2 [2.0] (2.1 Ubuntu:26.04/resolute [amd64])
Remv oldthing [0.9]
Inst hello (2.10 local-deb [amd64])
Conf libfoo1 (1.0 Ubuntu:26.04/resolute [amd64])
Conf hello (2.10 local-deb [amd64])
";
        let changes = parse_simulation(text, "hello");
        assert_eq!(changes.installs, ["libfoo1"]);
        assert_eq!(changes.upgrades, ["libbar2"]);
        assert_eq!(changes.removes, ["oldthing"]);
    }

    #[test]
    fn simulation_errors() {
        let stdout = "\
Some packages could not be installed.
The following packages have unmet dependencies:
 hello : Depends: libmissing (>= 3) but it is not installable
Done
";
        let stderr = "E: Unable to correct problems, you have held broken packages.\n";
        assert_eq!(
            simulation_error(stdout, stderr),
            "hello : Depends: libmissing (>= 3) but it is not installable"
        );
        // apt 3's solver
        let stderr = "\
E: Unable to satisfy dependencies. Reached two conflicting assignments:
   1. hello:amd64=1.0 is selected for install
   2. hello:amd64 Depends libmissing
";
        assert_eq!(
            simulation_error(stdout, stderr),
            "hello : Depends: libmissing (>= 3) but it is not installable"
        );
        assert_eq!(
            simulation_error("", "E: Unsupported file /x.deb given on commandline\n"),
            "Unsupported file /x.deb given on commandline"
        );
    }
}
