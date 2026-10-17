//! Explicit file adapter for source-preserving canonical Antimony formatting.
//!
//! Usage: `format_antimony INPUT.ant OUTPUT.ant`. Success prints a `stage=export
//! code=ok` status to stdout. Diagnostics/warnings use stderr. Exit 2 means a
//! syntax/resolve/export rejection; exit 1 means usage or file I/O failed. Output
//! is staged only after validation and replaced by rename after writing succeeds;
//! a rejected model never creates or overwrites the requested output.
use antimony::{
    Diagnostic, LimitKind, Limits, SourceFile, SourceId, analyze, format_antimony, parse,
};
use std::{
    env, fs,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

fn diagnostic(stage: &str, d: Diagnostic) -> ExitCode {
    eprintln!(
        "stage={stage} code={} message={}",
        d.code().as_str(),
        d.message()
    );
    for label in d.labels() {
        eprintln!(
            "source={} bytes={}..{} {}",
            label.span().source_id().get(),
            label.span().start(),
            label.span().end(),
            label.message()
        );
    }
    ExitCode::from(2)
}

// Keep any existing output intact on validation/write failure. The temporary file
// is in the same directory, avoiding cross-filesystem rename and partial output.
fn write_output(output: &Path, text: &str) -> io::Result<()> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..16 {
        let temporary = parent.join(format!(
            ".format-antimony-{}-{nonce}-{attempt}.tmp",
            std::process::id()
        ));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let result = file.write_all(text.as_bytes()).and_then(|()| file.flush());
        drop(file);
        let result = result.and_then(|()| fs::rename(&temporary, output));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        return result;
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "cannot reserve temporary output",
    ))
}

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("stage=input code=usage message=usage: format_antimony INPUT.ant OUTPUT.ant");
        return ExitCode::from(1);
    }
    let limits = Limits::default();
    let read = || -> io::Result<String> {
        let file = fs::File::open(&args[0])?;
        let mut text = String::new();
        file.take((limits.limit(LimitKind::SourceBytes) as u64) + 1)
            .read_to_string(&mut text)?;
        Ok(text)
    };
    let text = match read() {
        Ok(text) => text,
        Err(e) => {
            eprintln!("stage=input code=io message={e}");
            return ExitCode::from(1);
        }
    };
    let source = match SourceFile::new(SourceId::new(0), args[0].to_string_lossy(), &text, &limits)
    {
        Ok(source) => source,
        Err(e) => {
            eprintln!("stage=syntax code=resource_limit message={e}");
            return ExitCode::from(2);
        }
    };
    let parsed = match parse(&source, &limits) {
        Ok(parsed) => parsed,
        Err(d) => return diagnostic("syntax", d),
    };
    let model = match analyze(&parsed) {
        Ok(model) => model,
        Err(d) => return diagnostic("resolve", d),
    };
    let formatted = match format_antimony(&parsed) {
        Ok(formatted) => formatted,
        Err(d) => return diagnostic("export", d),
    };
    if let Err(e) = write_output(Path::new(&args[1]), &formatted) {
        eprintln!("stage=export code=io message={e}");
        return ExitCode::from(1);
    }
    for warning in model.warnings() {
        eprintln!(
            "stage=resolve severity={:?} code={} message={}",
            warning.severity(),
            warning.code().as_str(),
            warning.message()
        );
    }
    println!("stage=export code=ok message=canonical Antimony written");
    ExitCode::SUCCESS
}
