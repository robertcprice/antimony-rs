//! Independent filesystem/process contracts for the optional CLI adapter.
#![cfg(feature = "cli")]
use std::{
    ffi::OsStr,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "antimony-cli-review-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn input(&self, bytes: &[u8]) -> PathBuf {
        let input = self.join("input.ant");
        fs::write(&input, bytes).unwrap();
        input
    }
    fn no_staging(&self) {
        assert!(fs::read_dir(&self.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".antimony-")
        }));
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_antimony"))
}
fn run(args: &[&OsStr]) -> Output {
    command().args(args).output().unwrap()
}
fn formatted(input: &Path, output: &Path) -> Output {
    run(&[
        OsStr::new("format"),
        input.as_os_str(),
        OsStr::new("-o"),
        output.as_os_str(),
    ])
}
fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "invalid JSON: {e};stdout={:?};stderr={:?}",
            output.stdout, output.stderr
        )
    })
}
fn stdin_run(args: &[&str], bytes: &[u8]) -> Output {
    let mut child = command()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn unicode_file_paths_and_explicit_in_place_formatting_work() {
    let dir = Directory::new();
    let input = dir.join("source β.ant");
    let output = dir.join("result 💡.ant");
    fs::write(&input, b"model m;formula a=1,b=a+2;end").unwrap();
    assert_eq!(formatted(&input, &output).status.code(), Some(0));
    let expected = "model m\n  formula a = 1, b = a + 2;\nend\n";
    assert_eq!(fs::read_to_string(&output).unwrap(), expected);
    assert_eq!(formatted(&input, &input).status.code(), Some(0));
    assert_eq!(fs::read_to_string(&input).unwrap(), expected);
    dir.no_staging();
}
#[test]
fn source_and_semantic_rejection_preserve_existing_destination_bytes() {
    let dir = Directory::new();
    let output = dir.join("output.ant");
    for text in [b"unknown;".as_slice(), b"formula a=1,b=a/0;".as_slice()] {
        let input = dir.input(text);
        fs::write(&output, b"KEEP\0EXACT").unwrap();
        let result = formatted(&input, &output);
        assert_eq!(result.status.code(), Some(2));
        assert_eq!(fs::read(&output).unwrap(), b"KEEP\0EXACT");
        dir.no_staging();
        fs::remove_file(&output).unwrap();
        assert_eq!(formatted(&input, &output).status.code(), Some(2));
        assert!(!output.exists());
        dir.no_staging();
    }
}
#[test]
fn invalid_utf8_source_returns_input_failure_and_never_writes_artifact() {
    let dir = Directory::new();
    let input = dir.input(b"a=1;\xff");
    let output = dir.join("out");
    let result = run(&[
        OsStr::new("--json"),
        OsStr::new("format"),
        input.as_os_str(),
        OsStr::new("-o"),
        output.as_os_str(),
    ]);
    assert_eq!(result.status.code(), Some(1));
    json(&result);
    assert!(!output.exists());
    dir.no_staging();
}
#[test]
fn input_byte_cap_precedes_truncated_utf8_and_exact_cap_is_inclusive() {
    const CAP: usize = 4 * 1024 * 1024;
    let mut exact = vec![b' '; CAP - 4];
    exact.extend_from_slice(b"a=1;");
    let result = stdin_run(&["--json", "check", "-"], &exact);
    assert_eq!(result.status.code(), Some(0));
    json(&result);
    let mut oversized = vec![b' '; CAP - 1];
    oversized.extend_from_slice("💡".as_bytes());
    let result = stdin_run(&["--json", "check", "-"], &oversized);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stdout).contains("resource_limit"));
    json(&result);
}
#[test]
fn stdin_and_artifact_stdout_do_not_mix_reports_into_artifacts() {
    let result = stdin_run(&["format", "-"], b"a=(+2);");
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"a = (+2);\n");
    assert!(!result.stderr.is_empty());
    let result = stdin_run(&["--json", "check", "-"], b"a=1;");
    assert_eq!(result.status.code(), Some(0));
    json(&result);
}
#[test]
fn json_stdout_artifact_collision_fails_before_missing_input_is_opened() {
    for args in [
        vec!["--json", "format", "/no/such/input"],
        vec!["--json", "export", "/no/such/input", "-o", "-"],
    ] {
        let output = command().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        json(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("usage"));
    }
}
#[test]
fn nonregular_inputs_and_destinations_fail_without_creation_or_cleanup_damage() {
    let dir = Directory::new();
    let input = dir.input(b"a=1;");
    let result = run(&[OsStr::new("--json"), OsStr::new("check"), dir.0.as_os_str()]);
    assert_eq!(result.status.code(), Some(1));
    json(&result);
    assert_eq!(formatted(&input, &dir.0).status.code(), Some(1));
    assert!(dir.0.is_dir());
    assert_eq!(
        formatted(&input, &dir.join("missing/out")).status.code(),
        Some(1)
    );
    assert!(!dir.join("missing").exists());
    dir.no_staging();
}
#[test]
fn readonly_output_rejects_even_when_process_could_replace_its_directory_entry() {
    let dir = Directory::new();
    let input = dir.input(b"a=1;");
    let output = dir.join("readonly");
    fs::write(&output, b"KEEP").unwrap();
    let original = fs::metadata(&output).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&output, readonly).unwrap();
    let result = formatted(&input, &output);
    fs::set_permissions(&output, original).unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"KEEP");
    dir.no_staging();
}
#[test]
fn literal_dash_prefixed_filename_is_selected_only_after_argument_delimiter() {
    let dir = Directory::new();
    fs::write(dir.join("--json"), b"a=1;").unwrap();
    let output = command()
        .current_dir(&dir.0)
        .args(["format", "--", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"a = 1;\n");
    dir.no_staging();
}
#[test]
fn artifact_broken_pipe_is_an_io_exit_not_a_printing_panic() {
    let mut child = command()
        .args(["format", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take()); // Close before stdin EOF permits any artifact output.
    child.stdin.take().unwrap().write_all(b"a=1;").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
}
#[test]
fn report_broken_pipe_after_commit_does_not_claim_or_attempt_rollback() {
    let dir = Directory::new();
    let output = dir.join("committed.ant");
    let mut child = command()
        .args([
            OsStr::new("--json"),
            OsStr::new("format"),
            OsStr::new("-"),
            OsStr::new("-o"),
            output.as_os_str(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    child.stdin.take().unwrap().write_all(b"a=1;").unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"a = 1;\n");
    assert!(!String::from_utf8_lossy(&result.stderr).contains("panicked"));
    dir.no_staging();
}
#[cfg(unix)]
#[test]
fn unix_output_symlinks_reject_without_following_or_replacing_and_input_symlinks_work() {
    use std::os::unix::fs::symlink;
    let dir = Directory::new();
    let input = dir.input(b"a=1;");
    let link = dir.join("input-link");
    symlink(&input, &link).unwrap();
    let output = dir.join("formatted");
    assert_eq!(formatted(&link, &output).status.code(), Some(0));
    let destination = dir.join("output-link");
    symlink(&output, &destination).unwrap();
    let before = fs::read(&output).unwrap();
    assert_eq!(formatted(&input, &destination).status.code(), Some(1));
    assert!(
        fs::symlink_metadata(&destination)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&output).unwrap(), before);
    let dangling = dir.join("dangling");
    symlink(dir.join("absent"), &dangling).unwrap();
    assert_eq!(formatted(&input, &dangling).status.code(), Some(1));
    assert!(!dir.join("absent").exists());
    dir.no_staging();
}
#[cfg(unix)]
#[test]
fn unix_existing_permissions_survive_and_new_output_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Directory::new();
    let input = dir.input(b"a=1;");
    let output = dir.join("out");
    assert_eq!(formatted(&input, &output).status.code(), Some(0));
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o077,
        0
    );
    fs::set_permissions(&output, fs::Permissions::from_mode(0o640)).unwrap();
    assert_eq!(formatted(&input, &output).status.code(), Some(0));
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o640
    );
    dir.no_staging();
}
#[cfg(unix)]
#[test]
fn unix_output_hardlink_detaches_without_changing_peer_file() {
    let dir = Directory::new();
    let input = dir.input(b"a=1;");
    let peer = dir.join("peer");
    let output = dir.join("out");
    fs::write(&peer, b"KEEP").unwrap();
    fs::hard_link(&peer, &output).unwrap();
    assert_eq!(formatted(&input, &output).status.code(), Some(0));
    assert_eq!(fs::read(&peer).unwrap(), b"KEEP");
    assert_eq!(fs::read(&output).unwrap(), b"a = 1;\n");
    dir.no_staging();
}
#[cfg(unix)]
#[test]
fn unix_non_utf8_path_arguments_never_panic_and_follow_filesystem_capability() {
    use std::os::unix::ffi::OsStringExt;
    let dir = Directory::new();
    let input = dir
        .0
        .join(std::ffi::OsString::from_vec(b"model-\xff.ant".to_vec()));
    let output = dir.join("out");
    if let Err(error) = fs::write(&input, b"a=1;") {
        #[cfg(target_os = "macos")]
        if error.raw_os_error() == Some(92) {
            let result = run(&[OsStr::new("--json"), OsStr::new("check"), input.as_os_str()]);
            assert_eq!(result.status.code(), Some(1));
            json(&result);
            dir.no_staging();
            return;
        }
        panic!("unexpected non-UTF-8 fixture creation failure: {error}");
    }
    assert_eq!(formatted(&input, &output).status.code(), Some(0));
    assert_eq!(fs::read(&output).unwrap(), b"a = 1;\n");
    dir.no_staging();
}
#[cfg(unix)]
#[test]
fn unix_device_and_socket_inputs_reject_instead_of_attempting_stream_reads() {
    use std::os::unix::net::UnixListener;
    let dir = Directory::new();
    let socket = dir.join("socket");
    let _listener = UnixListener::bind(&socket).unwrap();
    for input in [socket.as_os_str(), OsStr::new("/dev/null")] {
        let result = run(&[OsStr::new("--json"), OsStr::new("check"), input]);
        assert_eq!(result.status.code(), Some(1));
        json(&result);
    }
}

#[test]
fn diagnostic_broken_stderr_returns_io_exit_without_panic_or_stdout_noise() {
    let mut child = command()
        .args(["check", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stderr.take());
    child.stdin.take().unwrap().write_all(b"unknown;").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}
