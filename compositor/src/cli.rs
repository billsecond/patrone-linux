//! Command-line options.

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

pub const USAGE: &str = "\
Usage: patrone-compositor [OPTION...]

  --backend auto|nested|drm Run as a window inside another desktop (nested) or
                            on the screen itself (drm). auto: nested when
                            WAYLAND_DISPLAY or DISPLAY is set
  --mode WIDTHxHEIGHT       Screen resolution for the drm backend
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
    "/../branding/wallpapers/patrone-back-02.png"
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
    pub backend: BackendChoice,
    /// The screen resolution the drm backend uses, instead of choosing one.
    pub mode: Option<(u16, u16)>,
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
            backend: BackendChoice::Auto,
            mode: None,
            terminal: None,
            wallpaper: PathBuf::from(DEFAULT_WALLPAPER),
            modifier: Modifier::Super,
            screenshot: None,
            screenshot_after: DEFAULT_SCREENSHOT_AFTER,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackendChoice {
    Auto,
    /// A window inside another desktop
    Nested,
    /// The screen itself, through DRM/KMS
    Drm,
}

impl BackendChoice {
    /// Nested when there's a desktop to be a window in.
    pub fn nested(self) -> bool {
        match self {
            BackendChoice::Nested => true,
            BackendChoice::Drm => false,
            BackendChoice::Auto => {
                std::env::var_os("WAYLAND_DISPLAY").is_some()
                    || std::env::var_os("DISPLAY").is_some()
            }
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
            "--backend" => options.backend = backend(&value()?)?,
            "--mode" => options.mode = Some(mode(&value()?)?),
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

fn backend(value: &str) -> Result<BackendChoice, String> {
    match value {
        "auto" => Ok(BackendChoice::Auto),
        "nested" | "winit" => Ok(BackendChoice::Nested),
        "drm" | "tty" => Ok(BackendChoice::Drm),
        _ => Err(format!(
            "--backend must be auto, nested or drm, not {value}"
        )),
    }
}

fn mode(value: &str) -> Result<(u16, u16), String> {
    value
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .ok_or_else(|| format!("--mode needs WIDTHxHEIGHT, such as 1920x1080, not {value}"))
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
                .ends_with("patrone-back-02.png")
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
    fn backend_and_mode() {
        let options = options(&["--backend", "drm", "--mode=1920x1080"]);
        assert_eq!(options.backend, BackendChoice::Drm);
        assert_eq!(options.mode, Some((1920, 1080)));
        assert!(BackendChoice::Nested.nested());
        assert!(!BackendChoice::Drm.nested());
        assert!(parse_args(&["--backend", "x11"]).is_err());
        assert!(parse_args(&["--mode", "1920"]).is_err());
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
