//! Explicit, bounded CLI I/O. None of this module belongs to the language core.
use std::{
    ffi::OsStr,
    fmt, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub(super) enum InputError {
    Io(io::Error),
    ResourceLimit { maximum: usize },
    InvalidUtf8,
}
impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(f),
            Self::ResourceLimit { maximum } => {
                write!(f, "source exceeds the {maximum}-byte input limit")
            }
            Self::InvalidUtf8 => f.write_str("input source is not valid UTF-8"),
        }
    }
}
impl std::error::Error for InputError {}
impl From<io::Error> for InputError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Follow input symlinks only to regular files; `-` explicitly selects stdin.
/// Metadata checks avoid ordinary FIFO/device opens but are not a defense against
/// an attacker changing directory entries between checks and open/read.
pub(super) fn read_source(path: &OsStr, max_bytes: usize) -> Result<String, InputError> {
    if path == OsStr::new("-") {
        return read_bounded(io::stdin().lock(), max_bytes);
    }
    let path = Path::new(path);
    require_regular(&fs::metadata(path)?)?;
    let file = fs::File::open(path)?;
    require_regular(&file.metadata()?)?;
    read_bounded(file, max_bytes)
}
fn require_regular(metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.is_file() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input path must resolve to a regular file; use '-' for stdin",
        ))
    }
}
fn read_bounded(reader: impl Read, max_bytes: usize) -> Result<String, InputError> {
    let read_limit = max_bytes
        .checked_add(1)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "input limit is too large"))?;
    let mut bytes = Vec::new();
    reader.take(read_limit).read_to_end(&mut bytes)?;
    // Count bytes before decoding: a multibyte character split at the sentinel
    // byte is still an oversized input, not an invalid-UTF-8 classification.
    if bytes.len() > max_bytes {
        return Err(InputError::ResourceLimit { maximum: max_bytes });
    }
    String::from_utf8(bytes).map_err(|_| InputError::InvalidUtf8)
}

/// Replace a regular output file, or create a new file, after a complete staged
/// write. Existing symlinks (including dangling ones), nonregular and read-only
/// outputs reject. In-place writes are supported after callers finish reading.
///
/// Replacement detaches the selected hard-link path; peers retain their data.
/// Ordinary permissions are preserved for existing regular files. New Unix files
/// and all Unix staging files start private (0600, further restricted by umask).
/// Ownership, ACLs, extended attributes, hostile-directory race protection and
/// crash/directory-sync durability are deliberately not promised.
pub(super) fn write_output(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_staged(path, bytes, |file, data| {
        file.write_all(data)?;
        file.flush()
    })
}

/// Fallible artifact stdout write; a closed pipe never invokes a printing panic.
pub(super) fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()
}

fn output_permissions(path: &Path) -> io::Result<Option<fs::Permissions>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "output path must be a regular file, not a symlink or other file type",
                ));
            }
            let permissions = metadata.permissions();
            if permissions.readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "output file is read-only",
                ));
            }
            Ok(Some(permissions))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn write_staged(
    path: &Path,
    bytes: &[u8],
    write_body: impl FnOnce(&mut fs::File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let permissions = output_permissions(path)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut temporary = reserve_temporary(parent, nonce)?;
    let file = temporary.file.as_mut().expect("reserved temporary file");
    write_body(file, bytes)?;
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)?;
    }
    // Close before rename so Windows does not keep our own handle in the way.
    // Drop cleanup also closes before attempting removal on every error path.
    drop(temporary.file.take());
    fs::rename(&temporary.path, path)?;
    temporary.armed = false;
    Ok(())
}
fn reserve_temporary(parent: &Path, nonce: u128) -> io::Result<Temporary> {
    for attempt in 0..16 {
        let path = parent.join(format!(
            ".antimony-{}-{nonce}-{attempt}.tmp",
            std::process::id()
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => {
                return Ok(Temporary {
                    path,
                    file: Some(file),
                    armed: true,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "cannot reserve a temporary output after 16 attempts",
    ))
}
struct Temporary {
    path: PathBuf,
    file: Option<fs::File>,
    armed: bool,
}
impl Drop for Temporary {
    fn drop(&mut self) {
        drop(self.file.take());
        if self.armed {
            // Cleanup is best effort: permission changes or process termination
            // can prevent removal. Never remove/alter the final output here.
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "antimony-io-unit-{}-{}-{}",
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
    #[test]
    fn byte_limit_precedes_utf8_decoding_and_never_reads_past_sentinel() {
        let mut input = io::Cursor::new(b"abc\xf0\x9f\x92\xa1rest");
        assert!(matches!(
            read_bounded(&mut input, 3),
            Err(InputError::ResourceLimit { maximum: 3 })
        ));
        assert_eq!(input.position(), 4);
        assert!(matches!(
            read_bounded(&b"\xff"[..], 1),
            Err(InputError::InvalidUtf8)
        ));
        assert!(matches!(
            read_bounded(&b"x"[..], 0),
            Err(InputError::ResourceLimit { maximum: 0 })
        ));
        assert_eq!(read_bounded(&b""[..], 0).unwrap(), "");
        assert_eq!(read_bounded("α".as_bytes(), 2).unwrap(), "α");
    }
    #[test]
    fn read_errors_and_unrepresentable_limit_are_returned_not_panicked() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("injected read failure"))
            }
        }
        assert!(matches!(read_bounded(Broken, 32), Err(InputError::Io(_))));
        assert!(matches!(
            read_bounded(&b""[..], usize::MAX),
            Err(InputError::Io(_))
        ));
    }
    #[test]
    fn regular_file_input_is_checked_without_lossy_path_conversion() {
        let dir = Directory::new();
        let path = dir.join("source α.ant");
        fs::write(&path, "a=1;").unwrap();
        assert_eq!(read_source(path.as_os_str(), 4).unwrap(), "a=1;");
        assert!(matches!(
            read_source(dir.0.as_os_str(), 10),
            Err(InputError::Io(_))
        ));
    }
    #[test]
    fn staged_write_replaces_regular_content_and_supports_explicit_in_place() {
        let dir = Directory::new();
        let path = dir.join("model.ant");
        fs::write(&path, "a=1;").unwrap();
        let text = read_source(path.as_os_str(), 100).unwrap();
        write_output(&path, format!("{text}\n").as_bytes()).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"a=1;\n");
        dir.no_staging();
    }
    #[test]
    fn injected_partial_write_and_flush_failures_preserve_destination_and_cleanup() {
        let dir = Directory::new();
        let path = dir.join("out");
        fs::write(&path, b"KEEP").unwrap();
        for partial in [true, false] {
            let error = write_staged(&path, b"replacement", |file, bytes| {
                file.write_all(if partial { &bytes[..1] } else { bytes })?;
                Err(io::Error::other(if partial {
                    "injected write"
                } else {
                    "injected flush"
                }))
            })
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert_eq!(fs::read(&path).unwrap(), b"KEEP");
            dir.no_staging();
        }
    }
    #[test]
    fn injected_rename_failure_cleans_temporary_not_destination() {
        let dir = Directory::new();
        let path = dir.join("out");
        let result = write_staged(&path, b"data", |file, bytes| {
            file.write_all(bytes)?;
            fs::create_dir(&path)?;
            Ok(())
        });
        assert!(result.is_err());
        assert!(path.is_dir());
        dir.no_staging();
    }
    #[test]
    fn directories_readonly_and_missing_parent_fail_before_destination_mutation() {
        let dir = Directory::new();
        assert!(write_output(&dir.0, b"data").is_err());
        assert!(write_output(&dir.join("absent/out"), b"data").is_err());
        let path = dir.join("read-only");
        fs::write(&path, b"KEEP").unwrap();
        let old = fs::metadata(&path).unwrap().permissions();
        let mut readonly = old.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let result = write_output(&path, b"bad");
        fs::set_permissions(&path, old).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&path).unwrap(), b"KEEP");
        dir.no_staging();
    }
    #[test]
    fn existing_staging_name_collisions_never_overwrite_and_retry_is_bounded() {
        let dir = Directory::new();
        let nonce = 42;
        for attempt in 0..16 {
            fs::write(
                dir.join(&format!(
                    ".antimony-{}-{nonce}-{attempt}.tmp",
                    std::process::id()
                )),
                b"KEEP",
            )
            .unwrap();
        }
        assert_eq!(
            reserve_temporary(&dir.0, nonce).err().unwrap().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 16);
        for entry in fs::read_dir(&dir.0).unwrap() {
            assert_eq!(fs::read(entry.unwrap().path()).unwrap(), b"KEEP");
        }
    }
    #[cfg(unix)]
    #[test]
    fn unix_new_files_are_private_and_existing_permissions_are_preserved() {
        use std::os::unix::fs::PermissionsExt;
        let dir = Directory::new();
        let fresh = dir.join("fresh");
        write_output(&fresh, b"data").unwrap();
        assert_eq!(
            fs::metadata(&fresh).unwrap().permissions().mode() & 0o077,
            0
        );
        fs::set_permissions(&fresh, fs::Permissions::from_mode(0o640)).unwrap();
        write_output(&fresh, b"changed").unwrap();
        assert_eq!(
            fs::metadata(&fresh).unwrap().permissions().mode() & 0o777,
            0o640
        );
        dir.no_staging();
    }
    #[cfg(unix)]
    #[test]
    fn unix_symlink_outputs_reject_but_regular_symlink_inputs_are_allowed() {
        use std::os::unix::fs::symlink;
        let dir = Directory::new();
        let target = dir.join("target");
        let link = dir.join("link");
        fs::write(&target, b"a=1;").unwrap();
        symlink(&target, &link).unwrap();
        assert_eq!(read_source(link.as_os_str(), 4).unwrap(), "a=1;");
        assert!(write_output(&link, b"BAD").is_err());
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&target).unwrap(), b"a=1;");
        let dangling = dir.join("dangling");
        symlink(dir.join("absent"), &dangling).unwrap();
        assert!(write_output(&dangling, b"BAD").is_err());
        assert!(!dir.join("absent").exists());
        dir.no_staging();
    }
    #[cfg(unix)]
    #[test]
    fn unix_hardlink_replacement_detaches_only_selected_path() {
        use std::os::unix::fs::MetadataExt;
        let dir = Directory::new();
        let original = dir.join("original");
        let output = dir.join("output");
        fs::write(&original, b"KEEP").unwrap();
        fs::hard_link(&original, &output).unwrap();
        write_output(&output, b"NEW").unwrap();
        assert_eq!(fs::read(&original).unwrap(), b"KEEP");
        assert_eq!(fs::read(&output).unwrap(), b"NEW");
        assert_ne!(
            fs::metadata(&original).unwrap().ino(),
            fs::metadata(&output).unwrap().ino()
        );
        dir.no_staging();
    }
    #[cfg(unix)]
    #[test]
    fn unix_non_utf8_paths_are_opened_as_os_strings() {
        use std::os::unix::ffi::OsStringExt;
        let dir = Directory::new();
        let path = dir
            .0
            .join(std::ffi::OsString::from_vec(b"source-\xff.ant".to_vec()));
        if let Err(error) = fs::write(&path, b"a=1;") {
            // macOS filesystems may reject invalid UTF-8 filenames themselves.
            // Still require a returned I/O error rather than an args/path panic;
            // this branch is not evidence of successful non-UTF-8 file creation.
            #[cfg(target_os = "macos")]
            if error.raw_os_error() == Some(92) {
                assert!(matches!(
                    read_source(path.as_os_str(), 4),
                    Err(InputError::Io(_))
                ));
                assert!(write_output(&path, b"data").is_err());
                dir.no_staging();
                return;
            }
            panic!("unexpected failure creating non-UTF-8 path: {error}");
        }
        assert_eq!(read_source(path.as_os_str(), 4).unwrap(), "a=1;");
        write_output(&path, b"a = 1;\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"a = 1;\n");
        dir.no_staging();
    }
    #[cfg(unix)]
    #[test]
    fn unix_device_and_socket_inputs_are_rejected_without_reading() {
        use std::os::unix::net::UnixListener;
        let dir = Directory::new();
        let socket = dir.join("socket");
        let _listener = UnixListener::bind(&socket).unwrap();
        assert!(matches!(
            read_source(socket.as_os_str(), 10),
            Err(InputError::Io(_))
        ));
        assert!(matches!(
            read_source(OsStr::new("/dev/null"), 10),
            Err(InputError::Io(_))
        ));
    }
}
