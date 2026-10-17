//! Subprocess contracts for the optional adapter, not new language semantics.
#![cfg(feature = "cli")]
#[cfg(feature = "sbml")]
use antimony::analyze;
use antimony::{Limits, SourceFile, SourceId, format_antimony, parse};
use serde_json::Value;
use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "antimony-cli-owner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str, source: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, source).unwrap();
        path
    }
    fn run(&self, args: &[&OsStr]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_antimony"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn json(output: &Output, code: i32) -> Value {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "antimony-cli-report-v1");
    assert_eq!(report["exit_code"], code);
    assert_eq!(report["profile_id"], antimony::IMPLEMENTED_PROFILE);
    report
}
fn formatted(source: &str) -> String {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(0), "fixture", source, &limits).unwrap();
    format_antimony(&parse(&source, &limits).unwrap()).unwrap()
}

#[test]
fn json_check_exposes_model_counts_and_all_warning_fields() {
    let f = Fixture::new();
    f.file("m.ant", "model m\nformula a=2,b=a+1;end");
    let report = json(
        &f.run(&["--json".as_ref(), "check".as_ref(), "m.ant".as_ref()]),
        0,
    );
    assert_eq!(report["command"], "check");
    assert_eq!(report["status"], "success");
    assert_eq!(report["source"]["display"], "m.ant");
    assert_eq!(report["source"]["lossy"], false);
    assert_eq!(report["model"]["id"], "m");
    assert_eq!(report["model"]["parameters"], 2);
    assert_eq!(report["model"]["initial_assignments"], 1);
    assert!(report["output"].is_null());
    assert_eq!(report["diagnostics"][0]["code"], "unspecified_units");
    assert_eq!(report["diagnostics"][0]["labels"][0]["start"]["line"], 1);
    assert_eq!(report["diagnostics"][0]["notes"], serde_json::json!([]));
}

#[test]
fn human_check_uses_stderr_only_and_retains_warnings() {
    let f = Fixture::new();
    f.file("m.ant", "a=1;");
    let output = f.run(&["check".as_ref(), "m.ant".as_ref()]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("warning[unspecified_units]"));
    assert!(text.contains("m.ant:1:1"));
    assert!(text.contains("check: success"));
}

#[test]
fn raw_format_stdout_equals_library_and_has_no_status_contamination() {
    let f = Fixture::new();
    let source = "model m\nformula a=1,b=a+2;end";
    f.file("m.ant", source);
    for args in [vec!["format", "m.ant"], vec!["format", "m.ant", "-o", "-"]] {
        let args = args.iter().map(OsStr::new).collect::<Vec<_>>();
        let output = f.run(&args);
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), formatted(source));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("format: success")
        );
    }
}

#[test]
fn json_format_writes_only_file_and_reports_commit() {
    let f = Fixture::new();
    let source = "a=1;";
    f.file("m.ant", source);
    let report = json(
        &f.run(&[
            "format".as_ref(),
            "m.ant".as_ref(),
            "--json".as_ref(),
            "-o".as_ref(),
            "out.ant".as_ref(),
        ]),
        0,
    );
    assert_eq!(
        report["output"],
        serde_json::json!({"destination":"file","path":{"display":"out.ant","lossy":false},"written":true})
    );
    assert_eq!(
        fs::read_to_string(f.0.join("out.ant")).unwrap(),
        formatted(source)
    );
}

#[test]
fn json_artifact_stdout_conflicts_fail_before_input_io() {
    let f = Fixture::new();
    for command in ["format", "export"] {
        for suffix in [vec![], vec!["-o", "-"]] {
            let mut args = vec!["--json", command, "missing.ant"];
            args.extend(suffix);
            let report = json(&f.run(&args.iter().map(OsStr::new).collect::<Vec<_>>()), 1);
            assert_eq!(report["diagnostics"][0]["code"], "usage");
            assert!(report["source"].is_null());
            assert!(report["output"].is_null());
        }
    }
}

#[test]
fn syntax_rejection_retains_utf8_byte_and_character_positions() {
    let f = Fixture::new();
    let source = "/* β */ nonsense;";
    f.file("m.ant", source);
    let report = json(
        &f.run(&[
            "--json".as_ref(),
            "format".as_ref(),
            "m.ant".as_ref(),
            "-o".as_ref(),
            "out.ant".as_ref(),
        ]),
        2,
    );
    let d = &report["diagnostics"][0];
    let l = &d["labels"][0];
    assert_eq!(d["code"], "syntax");
    assert_eq!(d["stage"], "syntax");
    assert_eq!(l["start_byte"], 9);
    assert_eq!(l["end_byte"], 17);
    assert_eq!(
        l["start"],
        serde_json::json!({"line":1,"column":9,"byte_column":10})
    );
    assert_eq!(report["output"]["written"], false);
    assert!(!f.0.join("out.ant").exists());
}

#[test]
fn resolve_rejection_preserves_core_code_and_secondary_labels() {
    let f = Fixture::new();
    f.file("m.ant", "species a;compartment a=2;");
    let report = json(
        &f.run(&["--json".as_ref(), "check".as_ref(), "m.ant".as_ref()]),
        2,
    );
    assert_eq!(report["diagnostics"][0]["stage"], "resolve");
    assert_eq!(report["diagnostics"][0]["code"], "declaration_conflict");
    assert_eq!(report["diagnostics"][0]["labels"][1]["style"], "secondary");
    assert!(report["model"].is_null());
}

#[test]
fn output_io_failure_keeps_model_and_warnings_without_claiming_write() {
    let f = Fixture::new();
    f.file("m.ant", "a=1;");
    fs::create_dir(f.0.join("directory")).unwrap();
    let report = json(
        &f.run(&[
            "--json".as_ref(),
            "format".as_ref(),
            "m.ant".as_ref(),
            "-o".as_ref(),
            "directory".as_ref(),
        ]),
        1,
    );
    assert_eq!(report["status"], "error");
    assert_eq!(report["output"]["written"], false);
    assert_eq!(report["model"]["parameters"], 1);
    assert_eq!(report["diagnostics"][0]["severity"], "warning");
    assert_eq!(
        report["diagnostics"].as_array().unwrap().last().unwrap()["stage"],
        "output"
    );
}

#[test]
fn stdin_source_and_json_report_are_separate_streams() {
    let f = Fixture::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_antimony"))
        .args(["--json", "check", "-"])
        .current_dir(&f.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"a=1;").unwrap();
    let report = json(&child.wait_with_output().unwrap(), 0);
    assert_eq!(report["source"]["display"], "-");
}

#[test]
fn delimiter_protects_literal_option_shaped_input_and_output_values() {
    let f = Fixture::new();
    f.file("--json", "a=1;");
    let output = f.run(&["check".as_ref(), "--".as_ref(), "--json".as_ref()]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    f.file("m.ant", "a=1;");
    let output = f.run(&[
        "format".as_ref(),
        "m.ant".as_ref(),
        "-o".as_ref(),
        "--json".as_ref(),
    ]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_to_string(f.0.join("--json")).unwrap(), "a = 1;\n");
}

#[test]
fn usage_errors_are_structured_and_never_touch_output() {
    let f = Fixture::new();
    f.file("out.ant", "SENTINEL");
    for args in [
        vec!["--json"],
        vec!["--json", "bogus"],
        vec!["--json", "check"],
        vec!["--json", "check", "a", "b"],
        vec!["--json", "check", "a", "-o", "out.ant"],
        vec!["--json", "format", "a", "-o"],
        vec!["--json", "--json", "check", "a"],
        vec!["--bad", "--json", "check", "a"],
        vec!["--json", "format", "a", "-o", "out.ant", "-o", "other"],
    ] {
        let report = json(&f.run(&args.iter().map(OsStr::new).collect::<Vec<_>>()), 1);
        assert_eq!(report["diagnostics"][0]["code"], "usage");
    }
    assert_eq!(fs::read(f.0.join("out.ant")).unwrap(), b"SENTINEL");
}

#[test]
fn help_and_version_succeed_in_both_report_modes_without_input() {
    let f = Fixture::new();
    for (flag, code) in [("--help", "help"), ("--version", "version")] {
        let report = json(&f.run(&["--json".as_ref(), flag.as_ref()]), 0);
        assert_eq!(report["diagnostics"][0]["code"], code);
        assert!(report["command"].is_null());
        let human = f.run(&[flag.as_ref()]);
        assert!(human.status.success());
        assert!(human.stdout.is_empty());
        assert!(!human.stderr.is_empty());
    }
}

#[test]
fn invalid_utf8_and_oversized_input_have_distinct_exit_codes() {
    let f = Fixture::new();
    f.file("bad.ant", [0xff]);
    let report = json(
        &f.run(&["--json".as_ref(), "check".as_ref(), "bad.ant".as_ref()]),
        1,
    );
    assert_eq!(report["diagnostics"][0]["code"], "invalid_utf8");
    let maximum = Limits::default().limit(antimony::LimitKind::SourceBytes);
    let mut oversized = vec![b' '; maximum];
    oversized.extend_from_slice("β".as_bytes());
    f.file("large.ant", oversized);
    let report = json(
        &f.run(&["--json".as_ref(), "check".as_ref(), "large.ant".as_ref()]),
        2,
    );
    assert_eq!(report["diagnostics"][0]["code"], "resource_limit");
}

#[cfg(unix)]
#[test]
fn lossy_path_displays_are_marked_and_terminal_controls_are_escaped() {
    use std::os::unix::ffi::OsStrExt;
    let f = Fixture::new();
    let path = OsStr::from_bytes(b"missing-\xff.ant");
    let report = json(&f.run(&["--json".as_ref(), "check".as_ref(), path]), 1);
    assert_eq!(report["source"]["lossy"], true);
    f.file("control\n\u{1b}[31m.ant", "a=1;");
    let human = f.run(&["check".as_ref(), "control\n\u{1b}[31m.ant".as_ref()]);
    assert!(human.status.success());
    assert!(!human.stderr.contains(&0x1b));
    let text = String::from_utf8(human.stderr).unwrap();
    assert!(text.contains("control\\n\\u{1b}[31m.ant"));
}

#[cfg(feature = "sbml")]
#[test]
fn export_matches_library_xml_exactly_and_check_model_summary() {
    let f = Fixture::new();
    let text = "S=2;P=0;k=.1;J:S=>P;k*S;";
    f.file("m.ant", text);
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(0), "fixture", text, &limits).unwrap();
    let expected =
        antimony::write_sbml(&analyze(&parse(&source, &limits).unwrap()).unwrap()).unwrap();
    let output = f.run(&["export".as_ref(), "m.ant".as_ref()]);
    assert!(output.status.success());
    assert_eq!(output.stdout, expected.xml().as_bytes());
    let report = json(
        &f.run(&[
            "--json".as_ref(),
            "export".as_ref(),
            "m.ant".as_ref(),
            "-o".as_ref(),
            "out.xml".as_ref(),
        ]),
        0,
    );
    assert_eq!(report["model"]["species"], 2);
    assert_eq!(report["output"]["written"], true);
    assert_eq!(
        fs::read(f.0.join("out.xml")).unwrap(),
        expected.xml().as_bytes()
    );
}

#[cfg(not(feature = "sbml"))]
#[test]
fn disabled_export_rejects_before_opening_missing_input_or_output() {
    let f = Fixture::new();
    let report = json(
        &f.run(&[
            "--json".as_ref(),
            "export".as_ref(),
            "missing.ant".as_ref(),
            "-o".as_ref(),
            "out.xml".as_ref(),
        ]),
        1,
    );
    assert_eq!(report["diagnostics"][0]["code"], "feature_unavailable");
    assert_eq!(report["diagnostics"][0]["stage"], "input");
    assert!(!f.0.join("out.xml").exists());
}
