//! Command-line options. `-e` and `-T` work like xterm's, which Debian
//! requires of the `x-terminal-emulator` alternative.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::window::Launch;

pub const USAGE: &str = "\
Usage: patrone-terminal [OPTION...] [-e COMMAND [ARG...]]

  -e, -x, -- COMMAND [ARG...]   Run COMMAND instead of your shell (must come last)
  -T, --title TITLE             Title for the new tab
  --working-directory DIR       Start in DIR
  -s, --standalone              Start a separate process instead of opening the
                                window in the one already running
  -h, --help                    Show this help
  --version                     Show the version
";

#[derive(Debug, PartialEq)]
pub enum Command {
    Open(Launch),
    Help,
    Version,
}

/// Whether `--standalone` comes before any command to run. GApplication has
/// to know before it starts, since by default it hands the command line to
/// an instance that's already running.
pub fn wants_standalone(args: impl IntoIterator<Item = OsString>) -> bool {
    args.into_iter()
        .skip(1)
        .take_while(|arg| !matches!(arg.to_str(), Some("-e" | "-x" | "--")))
        .any(|arg| matches!(arg.to_str(), Some("-s" | "--standalone")))
}

/// Parses `args` (including the program name). `cwd` is the directory the
/// command was run from, which new tabs start in unless told otherwise.
pub fn parse(args: &[OsString], cwd: Option<&Path>) -> Result<Command, String> {
    let mut launch = Launch {
        working_directory: cwd.map(Path::to_owned),
        ..Launch::default()
    };
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        let arg = arg.to_string_lossy();
        match arg.as_ref() {
            "-e" | "-x" | "--" => {
                let command: Vec<String> = args
                    .by_ref()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect();
                if command.is_empty() {
                    return Err(format!("{arg} needs a command to run"));
                }
                launch.command = Some(command);
            }
            "-T" | "--title" => launch.title = Some(value(&arg, args.next())?),
            "--working-directory" => {
                launch.working_directory = Some(resolve(cwd, &value(&arg, args.next())?));
            }
            "-s" | "--standalone" => {} // handled by wants_standalone
            "-h" | "--help" => return Ok(Command::Help),
            "--version" => return Ok(Command::Version),
            _ => {
                if let Some(title) = arg.strip_prefix("--title=") {
                    launch.title = Some(title.to_owned());
                } else if let Some(dir) = arg.strip_prefix("--working-directory=") {
                    launch.working_directory = Some(resolve(cwd, dir));
                } else {
                    return Err(format!("unknown option {arg}"));
                }
            }
        }
    }
    Ok(Command::Open(launch))
}

fn value(option: &str, value: Option<&OsString>) -> Result<String, String> {
    value
        .map(|v| v.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{option} needs a value"))
}

fn resolve(cwd: Option<&Path>, dir: &str) -> PathBuf {
    match cwd {
        Some(cwd) => cwd.join(dir),
        None => PathBuf::from(dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Command, String> {
        let args: Vec<OsString> = std::iter::once("patrone-terminal")
            .chain(args.iter().copied())
            .map(OsString::from)
            .collect();
        parse(&args, Some(Path::new("/home/user")))
    }

    fn launch(args: &[&str]) -> Launch {
        match parse_args(args) {
            Ok(Command::Open(launch)) => launch,
            other => panic!("expected a launch, got {other:?}"),
        }
    }

    #[test]
    fn no_arguments_opens_a_shell_where_it_was_run() {
        assert_eq!(
            launch(&[]),
            Launch {
                working_directory: Some("/home/user".into()),
                ..Launch::default()
            }
        );
    }

    #[test]
    fn dash_e_takes_the_rest_of_the_arguments() {
        let launch = launch(&[
            "-T",
            "Logs",
            "-e",
            "tail",
            "-f",
            "/var/log/syslog",
            "-T",
            "x",
        ]);
        assert_eq!(launch.title.as_deref(), Some("Logs"));
        assert_eq!(
            launch.command,
            Some(vec![
                "tail".into(),
                "-f".into(),
                "/var/log/syslog".into(),
                "-T".into(),
                "x".into()
            ])
        );
    }

    #[test]
    fn working_directory_is_relative_to_where_it_was_run() {
        assert_eq!(
            launch(&["--working-directory", "src"]).working_directory,
            Some("/home/user/src".into())
        );
        assert_eq!(
            launch(&["--working-directory=/tmp"]).working_directory,
            Some("/tmp".into())
        );
    }

    #[test]
    fn standalone_counts_only_before_the_command() {
        let args = |list: &[&str]| -> Vec<OsString> {
            std::iter::once("patrone-terminal")
                .chain(list.iter().copied())
                .map(OsString::from)
                .collect()
        };
        assert!(wants_standalone(args(&["--standalone"])));
        assert!(wants_standalone(args(&["-s", "-e", "htop"])));
        assert!(!wants_standalone(args(&["-e", "ls", "-s"])));
        assert!(!wants_standalone(args(&[])));
        assert_eq!(launch(&["-s"]), launch(&[]));
    }

    #[test]
    fn bad_arguments_are_errors() {
        assert!(parse_args(&["-e"]).is_err());
        assert!(parse_args(&["--title"]).is_err());
        assert!(parse_args(&["--frobnicate"]).is_err());
    }
}
