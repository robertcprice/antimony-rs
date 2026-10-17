//! Thin orchestration only: all language validation and serialization is delegated.
mod args;
mod io;
mod report;

use antimony::{LimitKind, Limits, SourceFile, SourceId, analyze, format_antimony, parse};
use args::{Action, Command};
use report::Report;
use serde_json::json;
use std::{
    ffi::{OsStr, OsString},
    io::Write,
    path::Path,
    process::ExitCode,
};

pub(super) fn main(arguments: Vec<OsString>) -> ExitCode {
    let options = match args::parse(arguments) {
        Ok(options) => options,
        Err(error) => {
            let mut report = Report::new(error.command);
            report.fail(1, "usage", "usage", error.message);
            return emit(report, error.json);
        }
    };
    let mut report = Report::new(options.command);
    match options.action {
        Action::Help => report.message("usage", "note", "help", args::HELP),
        Action::Version => report.message(
            "usage",
            "note",
            "version",
            concat!("antimony ", env!("CARGO_PKG_VERSION")),
        ),
        Action::Run {
            command,
            input,
            output,
        } => run(command, &input, output.as_deref(), &mut report),
    }
    emit(report, options.json)
}

fn run(command: Command, input: &OsStr, output: Option<&OsStr>, report: &mut Report) {
    report.source = report::display(input);
    let file_output = output.filter(|path| *path != OsStr::new("-"));
    if command != Command::Check {
        report.output = json!({"destination":if file_output.is_some(){"file"}else{"stdout"},
            "path":file_output.map(report::display),"written":false});
    }
    // A feature error precedes even input metadata/file access.
    #[cfg(not(feature = "sbml"))]
    if command == Command::Export {
        report.fail(
            1,
            "input",
            "feature_unavailable",
            "export requires a build with the sbml feature",
        );
        return;
    }
    let limits = Limits::default();
    let text = match io::read_source(input, limits.limit(LimitKind::SourceBytes)) {
        Ok(text) => text,
        Err(io::InputError::Io(error)) => {
            report.fail(1, "input", "io", error.to_string());
            return;
        }
        Err(io::InputError::ResourceLimit { maximum }) => {
            report.fail(
                2,
                "input",
                "resource_limit",
                format!("source exceeds {maximum} bytes"),
            );
            return;
        }
        Err(io::InputError::InvalidUtf8) => {
            report.fail(1, "input", "invalid_utf8", "input is not valid UTF-8");
            return;
        }
    };
    let source = match SourceFile::new(SourceId::new(0), input.to_string_lossy(), &text, &limits) {
        Ok(source) => source,
        Err(error) => {
            report.fail(2, "input", "resource_limit", error.to_string());
            return;
        }
    };
    let parsed = match parse(&source, &limits) {
        Ok(parsed) => parsed,
        Err(diagnostic) => {
            report.reject("syntax", &diagnostic, &source);
            return;
        }
    };
    let model = match analyze(&parsed) {
        Ok(model) => model,
        Err(diagnostic) => {
            report.reject("resolve", &diagnostic, &source);
            return;
        }
    };
    report.model(&model);
    let artifact = match command {
        Command::Check => return,
        Command::Format => match format_antimony(&parsed) {
            Ok(formatted) => formatted,
            Err(diagnostic) => {
                report.reject("format", &diagnostic, &source);
                return;
            }
        },
        Command::Export => {
            #[cfg(feature = "sbml")]
            {
                match antimony::write_sbml(&model) {
                    Ok(document) => document.xml().to_owned(),
                    Err(diagnostic) => {
                        report.reject("export", &diagnostic, &source);
                        return;
                    }
                }
            }
            #[cfg(not(feature = "sbml"))]
            unreachable!("export feature checked before input I/O")
        }
    };
    // Never open a destination or emit artifact bytes before the whole pipeline
    // succeeds. Staging protects files; stdout cannot offer rollback on I/O error.
    let result = match file_output {
        Some(path) => io::write_output(Path::new(path), artifact.as_bytes()),
        None => io::write_stdout(artifact.as_bytes()),
    };
    match result {
        Ok(()) => report.output["written"] = json!(true),
        Err(error) => report.fail(1, "output", "io", error.to_string()),
    }
}

fn emit(report: Report, json: bool) -> ExitCode {
    let code = report.exit_code;
    let result = if json {
        serde_json::to_vec(&report.value())
            .map_err(std::io::Error::other)
            .and_then(|mut bytes| {
                bytes.push(b'\n');
                io::write_stdout(&bytes)
            })
    } else {
        let mut stderr = std::io::stderr().lock();
        stderr
            .write_all(report.human().as_bytes())
            .and_then(|()| stderr.flush())
    };
    if let Err(error) = result {
        // A committed artifact is not rolled back if the report pipe breaks.
        // The fallback is best effort, also fallible; never panic while reporting.
        let message = format!(
            "error[io] (output): cannot write report: {}\n",
            report::escape(&error.to_string())
        );
        let _ = std::io::stderr().lock().write_all(message.as_bytes());
        return ExitCode::from(1);
    }
    ExitCode::from(code)
}
