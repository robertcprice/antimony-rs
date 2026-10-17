//! Explicit file-I/O adapter for decompiling SBML Level 3 Version 2 Core into Antimony.
use antimony::{Diagnostic, LimitKind, Limits, sbml_to_antimony};
use std::{
    env, fs,
    io::{self, Read, Write},
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
        eprintln!("stage=input code=usage message=usage: import_sbml INPUT.xml [OUTPUT.ant]");
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
    let xml = match read() {
        Ok(text) => text,
        Err(e) => {
            eprintln!("stage=input code=io message={e}");
            return ExitCode::from(1);
        }
    };
    let antimony_src = match sbml_to_antimony(&xml, &limits) {
        Ok(src) => src,
        Err(d) => return diagnostic("import", d),
    };
    let result = if let Some(output) = args.get(1) {
        fs::write(output, &antimony_src)
    } else {
        io::stdout().lock().write_all(antimony_src.as_bytes())
    };
    if let Err(e) = result {
        eprintln!("stage=input code=io message={e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
