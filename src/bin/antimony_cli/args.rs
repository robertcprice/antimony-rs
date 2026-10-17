//! Small OS-string argument parser, independent of source parsing and file I/O.
use std::ffi::{OsStr, OsString};

pub(super) const HELP: &str = "Usage: antimony [--json] check INPUT\n       antimony [--json] format INPUT [-o OUTPUT]\n       antimony [--json] export INPUT [-o OUTPUT]\n\nINPUT '-' reads stdin. Without -o, artifacts go to stdout; OUTPUT '-' also selects stdout.\nJSON format/export require -o with a real file path. JSON reports go to stdout;\nhuman diagnostics/status go to stderr. '--' ends option parsing for literal paths.\nOptions: --json, -o OUTPUT, --help, --version.\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Command {
    Check,
    Format,
    Export,
}
impl Command {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Format => "format",
            Self::Export => "export",
        }
    }
}
pub(super) enum Action {
    Help,
    Version,
    Run {
        command: Command,
        input: OsString,
        output: Option<OsString>,
    },
}
pub(super) struct Options {
    pub json: bool,
    pub command: Option<Command>,
    pub action: Action,
}
pub(super) struct ArgumentError {
    pub json: bool,
    pub command: Option<Command>,
    pub message: String,
}

// Recover report mode even if an earlier argument is invalid. Output-option
// values and operands after '--' are paths, never option requests.
fn requested_json(args: &[OsString]) -> bool {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if arg == "-o" {
            args.next();
        } else if arg == "--json" {
            return true;
        }
    }
    false
}

pub(super) fn parse(args: Vec<OsString>) -> Result<Options, ArgumentError> {
    let json = requested_json(&args);
    let mut command = None;
    let mut input = None;
    let mut output = None;
    let mut options = true;
    let mut json_seen = false;
    let mut help = false;
    let mut version = false;
    let error = |command, message: &str| ArgumentError {
        json,
        command,
        message: message.to_owned(),
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if options {
            if arg == "--" {
                options = false;
                continue;
            }
            if arg == "--json" {
                if json_seen {
                    return Err(error(command, "--json may be specified only once"));
                }
                json_seen = true;
                continue;
            }
            if arg == "--help" {
                if help {
                    return Err(error(command, "--help may be specified only once"));
                }
                help = true;
                continue;
            }
            if arg == "--version" {
                if version {
                    return Err(error(command, "--version may be specified only once"));
                }
                version = true;
                continue;
            }
            if arg == "-o" {
                if output.is_some() {
                    return Err(error(command, "-o may be specified only once"));
                }
                output = Some(
                    args.next()
                        .ok_or_else(|| error(command, "-o requires a path"))?,
                );
                continue;
            }
            if arg != "-" && arg.as_encoded_bytes().starts_with(b"-") {
                return Err(error(
                    command,
                    "unknown option; use -- before a literal input path",
                ));
            }
        }
        if command.is_none() {
            command = Some(match arg.to_str() {
                Some("check") => Command::Check,
                Some("format") => Command::Format,
                Some("export") => Command::Export,
                _ => return Err(error(None, "expected check, format or export subcommand")),
            });
        } else if input.replace(arg).is_some() {
            return Err(error(command, "exactly one input path is required"));
        }
    }
    if help || version {
        if (help && version) || input.is_some() || output.is_some() {
            return Err(error(
                command,
                "help/version cannot be combined with input, output or each other",
            ));
        }
        return Ok(Options {
            json,
            command,
            action: if help { Action::Help } else { Action::Version },
        });
    }
    let selected = command.ok_or_else(|| error(None, "a subcommand is required; use --help"))?;
    let input = input.ok_or_else(|| error(command, "an input path or '-' is required"))?;
    if selected == Command::Check && output.is_some() {
        return Err(error(command, "check does not accept an output path"));
    }
    if json && selected != Command::Check && output.as_deref().is_none_or(|p| p == OsStr::new("-"))
    {
        return Err(error(
            command,
            "JSON format/export requires -o with a file path, not stdout",
        ));
    }
    Ok(Options {
        json,
        command,
        action: Action::Run {
            command: selected,
            input,
            output,
        },
    })
}
