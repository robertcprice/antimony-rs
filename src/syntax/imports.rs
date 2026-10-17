//! Safe, cycle-checked multi-file `import "other.ant"` resolution.
//!
//! Single-file [`crate::parse`] remains strictly sandboxed and rejects `import`
//! statements. [`crate::parse_with_imports`] and [`crate::load_file`] resolve
//! relative `import "file.ant"` directives through a caller-supplied closure
//! (or sandboxed relative directory reader) while enforcing cycle detection,
//! depth ceilings, path-traversal prevention, and cumulative [`Limits`].

use super::lexer::{Kind, lex};
use super::{ParsedDocument, error, parse_document};
use crate::{Diagnostic, DiagnosticCode, LimitKind, Limits, SourceFile, SourceId, Span};
use std::collections::BTreeSet;
use std::path::Path;

const MAX_IMPORT_DEPTH: usize = 16;

/// Parse an Antimony source file while resolving top-level `import "path.ant"` directives
/// via the caller-provided `resolver` callback.
///
/// Relative paths inside nested imports are normalized relative to the importing file's
/// directory. Absolute paths, parent traversal (`..`), cyclic imports, and cumulative
/// [`LimitKind::SourceBytes`] violations are rejected deterministically.
pub fn parse_with_imports<F>(
    source: &SourceFile,
    limits: &Limits,
    mut resolver: F,
) -> Result<ParsedDocument, Diagnostic>
where
    F: FnMut(&str) -> Result<String, String>,
{
    let origin = source.span(0, 0).expect("origin span");
    limits
        .check(LimitKind::SourceBytes, source.text().len())
        .map_err(|e| error(DiagnosticCode::ResourceLimit, origin, e.to_string()))?;

    let mut visited = BTreeSet::<String>::new();
    let mut active_stack = Vec::<String>::new();
    let mut total_bytes = source.text().len();

    let expanded = expand_source_imports(
        source,
        "",
        limits,
        &mut resolver,
        &mut visited,
        &mut active_stack,
        &mut total_bytes,
        0,
        false,
    )?;

    let combined_source =
        SourceFile::new(source.id(), source.name(), &expanded, limits).map_err(|e| {
            error(
                DiagnosticCode::ResourceLimit,
                origin,
                format!("combined imported source exceeds byte limit: {e}"),
            )
        })?;
    parse_document(&combined_source, limits)
}

/// Load, resolve relative `import "..."` directives, parse, and validate an Antimony file
/// from disk.
///
/// Imported paths are resolved strictly relative to `path.parent()` and may not traverse
/// above that base directory (`..`) or use absolute filesystem paths.
pub fn load_file(path: &Path, limits: &Limits) -> Result<crate::ValidatedModel, Diagnostic> {
    let dummy = SourceFile::new(
        SourceId::new(0),
        path.to_string_lossy(),
        "",
        &Limits::default(),
    )
    .expect("empty source");
    let origin = dummy.span(0, 0).expect("origin");
    let text = std::fs::read_to_string(path).map_err(|e| {
        error(
            DiagnosticCode::UnsupportedFeature,
            origin,
            format!("cannot read Antimony file {:?}: {e}", path.display()),
        )
    })?;
    let source = SourceFile::new(SourceId::new(0), path.to_string_lossy(), &text, limits)
        .map_err(|e| error(DiagnosticCode::ResourceLimit, origin, e.to_string()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let parsed = parse_with_imports(&source, limits, |rel_path| {
        let target = base_dir.join(rel_path);
        std::fs::read_to_string(&target).map_err(|e| format!("cannot read {rel_path:?}: {e}"))
    })?;
    crate::analyze(&parsed)
}

#[allow(clippy::too_many_arguments)]
fn expand_source_imports<F>(
    source: &SourceFile,
    current_dir: &str,
    limits: &Limits,
    resolver: &mut F,
    visited: &mut BTreeSet<String>,
    active_stack: &mut Vec<String>,
    total_bytes: &mut usize,
    depth: usize,
    is_imported_file: bool,
) -> Result<String, Diagnostic>
where
    F: FnMut(&str) -> Result<String, String>,
{
    let text = source.text();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut out = String::with_capacity(text.len());

    // Scan leading whitespace, comments, and `import "path"` directives at the top of the file.
    loop {
        while i < bytes.len() {
            match bytes[i] {
                b' ' | b'\t' | b'\r' | b'\n' | b';' => i += 1,
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    i += 2;
                    while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                        i += 1;
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    let start = i;
                    i += 2;
                    while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                        i += 1;
                    }
                    if i + 1 >= bytes.len() {
                        return Err(error(
                            DiagnosticCode::Syntax,
                            source.span(start, text.len()).expect("span"),
                            "unterminated block comment",
                        ));
                    }
                    i += 2;
                }
                _ => break,
            }
        }

        if i + 6 <= bytes.len()
            && &bytes[i..i + 6] == b"import"
            && !bytes
                .get(i + 6)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            let import_kw_span = source.span(i, i + 6).expect("import span");
            if depth >= MAX_IMPORT_DEPTH {
                return Err(error(
                    DiagnosticCode::ResourceLimit,
                    import_kw_span,
                    format!("import nesting depth exceeds maximum of {MAX_IMPORT_DEPTH}"),
                ));
            }
            i += 6;
            while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
                i += 1;
            }
            if bytes.get(i) != Some(&b'"') {
                let err_end = (i + 1).min(text.len());
                return Err(error(
                    DiagnosticCode::Syntax,
                    source.span(i, err_end).expect("span"),
                    "expected a quoted string path after 'import'",
                ));
            }
            let quote_start = i;
            i += 1;
            let path_start = i;
            while i < bytes.len() && bytes[i] != b'"' && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            if bytes.get(i) != Some(&b'"') {
                return Err(error(
                    DiagnosticCode::Syntax,
                    source.span(quote_start, i).expect("span"),
                    "unterminated string literal in import statement",
                ));
            }
            let raw_path = &text[path_start..i];
            i += 1;
            let path_span = source.span(quote_start, i).expect("path span");

            while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
                i += 1;
            }
            if i < bytes.len() {
                match bytes[i] {
                    b';' | b'\n' | b'\r' => {
                        i += 1;
                    }
                    b'/' if matches!(bytes.get(i + 1), Some(b'/' | b'*')) => {}
                    _ => {
                        let err_end = (i + 1).min(text.len());
                        return Err(error(
                            DiagnosticCode::Syntax,
                            source.span(i, err_end).expect("span"),
                            "expected ';' or newline after import path",
                        ));
                    }
                }
            }

            let norm_path = normalize_relative_import(current_dir, raw_path, path_span)?;
            if active_stack.contains(&norm_path) {
                let mut chain = active_stack.clone();
                chain.push(norm_path);
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    path_span,
                    format!("cyclic import chain detected: {}", chain.join(" -> ")),
                ));
            }

            if visited.insert(norm_path.clone()) {
                let imported_text = resolver(&norm_path).map_err(|msg| {
                    error(
                        DiagnosticCode::UnsupportedFeature,
                        path_span,
                        format!("cannot resolve import {raw_path:?}: {msg}"),
                    )
                })?;
                *total_bytes = limits
                    .checked_add(LimitKind::SourceBytes, *total_bytes, imported_text.len())
                    .map_err(|e| error(DiagnosticCode::ResourceLimit, path_span, e.to_string()))?;

                let imp_source = SourceFile::new(
                    SourceId::new(depth as u32 + 1),
                    norm_path.clone(),
                    &imported_text,
                    limits,
                )
                .map_err(|e| error(DiagnosticCode::ResourceLimit, path_span, e.to_string()))?;

                let next_dir = parent_dir(&norm_path);
                active_stack.push(norm_path);
                let expanded_imp = expand_source_imports(
                    &imp_source,
                    &next_dir,
                    limits,
                    resolver,
                    visited,
                    active_stack,
                    total_bytes,
                    depth + 1,
                    true,
                )?;
                active_stack.pop();

                out.push_str(&expanded_imp);
                if !expanded_imp.ends_with('\n') {
                    out.push('\n');
                }
            }
        } else {
            break;
        }
    }

    let body_text = &text[i..];
    let body_source =
        SourceFile::new(source.id(), source.name(), body_text, limits).map_err(|e| {
            error(
                DiagnosticCode::ResourceLimit,
                source.span(0, 0).expect("span"),
                e.to_string(),
            )
        })?;
    let tokens = lex(&body_source, limits)?;
    let mut block_depth = 0usize;
    for tok in &tokens {
        match &tok.kind {
            Kind::Eof | Kind::Sep | Kind::Newline => {}
            Kind::Ident(word) if word == "import" => {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    tok.span,
                    "import statements must appear at the top of the file before model or function definitions",
                ));
            }
            Kind::Ident(word) if word == "function" || word == "model" || word == "module" => {
                block_depth += 1;
            }
            Kind::Ident(word) if word == "end" => {
                block_depth = block_depth.saturating_sub(1);
            }
            _ => {
                if is_imported_file && block_depth == 0 {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        tok.span,
                        "imported Antimony files must contain only top-level 'model'/'module' or 'function' definitions",
                    ));
                }
            }
        }
    }

    out.push_str(body_text);
    Ok(out)
}

fn normalize_relative_import(
    current_dir: &str,
    raw_path: &str,
    span: Span,
) -> Result<String, Diagnostic> {
    if raw_path.is_empty() || raw_path.contains('\0') {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            span,
            "import path must be a non-empty relative path",
        ));
    }
    if raw_path.starts_with('/') || raw_path.starts_with('\\') || raw_path.contains(':') {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            span,
            format!("absolute or device import path {raw_path:?} is rejected for security"),
        ));
    }
    let mut parts: Vec<&str> = if current_dir.is_empty() {
        Vec::new()
    } else {
        current_dir.split('/').filter(|s| !s.is_empty()).collect()
    };
    for seg in raw_path.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    span,
                    format!("parent directory traversal ('..') in import {raw_path:?} is rejected"),
                ));
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            span,
            "import path resolves to an empty path",
        ));
    }
    Ok(parts.join("/"))
}

fn parent_dir(norm_path: &str) -> String {
    match norm_path.rfind('/') {
        Some(pos) => norm_path[..pos].to_owned(),
        None => String::new(),
    }
}
