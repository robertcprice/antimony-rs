//! Explicit file-I/O adapter for the deliberately narrow native compiler slice.
use antimony::{Diagnostic, Limits, SourceFile, SourceId, analyze, parse, write_sbml};
use std::{
    env, fs,
    io::{self, Write},
    process::ExitCode,
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
fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if !(1..=2).contains(&args.len()) {
        eprintln!("stage=input code=usage message=usage: export_sbml INPUT.ant [OUTPUT.xml]");
        return ExitCode::from(1);
    }
    let limits = Limits::default();
    // Bound reads, not just parsing. Core code never opens files.
    let read = || -> io::Result<String> {
        use std::io::Read;
        let file = fs::File::open(&args[0])?;
        let mut text = String::new();
        file.take((limits.limit(antimony::LimitKind::SourceBytes) as u64) + 1)
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
    let document = match write_sbml(&model) {
        Ok(document) => document,
        Err(d) => return diagnostic("export", d),
    };
    let result = if let Some(output) = args.get(1) {
        fs::write(output, document.xml())
    } else {
        io::stdout().lock().write_all(document.xml().as_bytes())
    };
    if let Err(e) = result {
        eprintln!("stage=input code=io message={e}");
        return ExitCode::from(1);
    }
    for warning in document.warnings() {
        eprintln!(
            "stage=resolve severity={:?} code={} message={}",
            warning.severity(),
            warning.code().as_str(),
            warning.message()
        );
    }
    ExitCode::SUCCESS
}
