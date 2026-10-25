//! Command-line options.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

pub const USAGE: &str = "\
Usage: patrone-compositor [OPTION...]

  --terminal COMMAND        Start COMMAND (run by sh) instead of the default terminal
  --wallpaper PATH          Draw PATH as the background (PNG)
  --modifier super|alt      Key held for window shortcuts (default: super)
  --screenshot PATH         Save a rendered frame to PATH (PNG), for debugging
  --screenshot-after SECS   When to save it (default: 5)
  -h, --help                Show this help
  --version                 Show the version
";

/// The wallpaper in this repository, for running from a checkout.
const DEFAULT_WALLPAPER: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../branding/wallpapers/patrone-back-01.png"
);
const DEFAULT_SCREENSHOT_AFTER: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq)]
pub enum Command {
    Run(Options),
    Help,
    Version,
}

#[derive(Debug, PartialEq)]
pub struct Options {
    /// A shell command that replaces the default terminal.
    pub terminal: Option<String>,
    pub wallpaper: PathBuf,
    pub modifier: Modifier,
    pub screenshot: Option<PathBuf>,
    pub screenshot_after: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            terminal: None,
            wallpaper: PathBuf::from(DEFAULT_WALLPAPER),
            modifier: Modifier::Super,
            screenshot: None,
            screenshot_after: DEFAULT_SCREENSHOT_AFTER,
        }
    }
}

/// The key held down for the compositor's own shortcuts and mouse bindings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Modifier {
    Super,
    /// Inside GNOME, which keeps Super+drag for itself.
    Alt,
}

/// Parses `args` (including the program name).
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, String> {
    let mut options = Options::default();
    let mut args = args.into_iter().skip(1);
    while let Some(arg) = args.next() {
        let arg = arg.to_string_lossy().into_owned();
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_owned())),
            _ => (arg.as_str(), None),
        };
        let mut value = || {
            inline
                .clone()
                .or_else(|| args.next().map(|v| v.to_string_lossy().into_owned()))
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match name {
            "--terminal" => options.terminal = Some(value()?),
            "--wallpaper" => options.wallpaper = PathBuf::from(value()?),
            "--modifier" => options.modifier = modifier(&value()?)?,
            "--screenshot" => options.screenshot = Some(PathBuf::from(value()?)),
            "--screenshot-after" => options.screenshot_after = seconds(&value()?)?,
            "-h" | "--help" => return Ok(Command::Help),
            "--version" => return Ok(Command::Version),
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    Ok(Command::Run(options))
}

fn modifier(value: &str) -> Result<Modifier, String> {
    match value.to_ascii_lowercase().as_str() {
        "super" | "logo" => Ok(Modifier::Super),
        "alt" => Ok(Modifier::Alt),
        _ => Err(format!("--modifier must be super or alt, not {value}")),
    }
}

fn seconds(value: &str) -> Result<Duration, String> {
    value
        .parse::<f64>()
        .ok()
        .and_then(|secs| Duration::try_from_secs_f64(secs).ok())
        .ok_or_else(|| format!("--screenshot-after needs a number of seconds, not {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Command, String> {
        parse(
            std::iter::once("patrone-compositor")
                .chain(args.iter().copied())
                .map(OsString::from),
        )
    }

    fn options(args: &[&str]) -> Options {
        match parse_args(args) {
            Ok(Command::Run(options)) => options,
            other => panic!("expected options, got {other:?}"),
        }
    }

    #[test]
    fn no_arguments_uses_the_defaults() {
        assert_eq!(options(&[]), Options::default());
        assert!(
            Options::default()
                .wallpaper
                .ends_with("patrone-back-01.png")
        );
    }

    #[test]
    fn values_can_follow_or_use_equals() {
        let options = options(&[
            "--terminal",
            "foot -e htop",
            "--modifier=alt",
            "--screenshot",
            "/tmp/shot.png",
            "--screenshot-after=1.5",
        ]);
        assert_eq!(options.terminal.as_deref(), Some("foot -e htop"));
        assert_eq!(options.modifier, Modifier::Alt);
        assert_eq!(options.screenshot, Some(PathBuf::from("/tmp/shot.png")));
        assert_eq!(options.screenshot_after, Duration::from_millis(1500));
    }

    #[test]
    fn help_and_version() {
        assert_eq!(parse_args(&["--help"]), Ok(Command::Help));
        assert_eq!(parse_args(&["--version"]), Ok(Command::Version));
    }

    #[test]
    fn bad_arguments_are_errors() {
        assert!(parse_args(&["--terminal"]).is_err());
        assert!(parse_args(&["--modifier", "ctrl"]).is_err());
        assert!(parse_args(&["--screenshot-after", "-1"]).is_err());
        assert!(parse_args(&["--frobnicate"]).is_err());
    }
}
