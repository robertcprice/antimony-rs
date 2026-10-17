use crate::syntax::error;
use crate::syntax::lexer::{Kind, Token, lex};
use crate::{Diagnostic, DiagnosticCode, LimitKind, ParsedDocument, SourceFile, analyze, parse};

/// Validate and canonically lay out a parsed document without changing its syntax.
///
/// Statements and tokens retain source order, including number spellings, explicit
/// parentheses, unary signs, reaction arrows, boundary markers, unit juxtaposition,
/// and named versus anonymous model envelopes. This deliberately does **not** sort
/// equivalent models, simplify expressions, invent declarations, or reconstruct
/// source from normalized SBML metadata. Comments and original whitespace are
/// discarded. Each statement ends with `;` and LF; named bodies use two spaces.
///
/// Semantic analysis succeeds before any text is returned. The result is parsed
/// and analyzed again using the original document's limits. Canonical expansion
/// must fit its source-byte and token ceilings; otherwise `ResourceLimit` is
/// returned, with a location in the **original** source. Successful results are
/// byte-idempotent when parsed and formatted again with the same limits.
///
/// The core function has no filesystem, process, network, or printing effects.
/// Warnings remain available through [`analyze`]; formatting does not certify
/// dimensional completeness or acceptance by the full Antimony language.
///
/// ```
/// use antimony::{Limits, SourceFile, SourceId, parse, format_antimony};
/// let limits = Limits::default();
/// let source = SourceFile::new(SourceId::new(0), "label", "k=(+2);", &limits).unwrap();
/// let parsed = parse(&source, &limits).unwrap();
/// assert_eq!(format_antimony(&parsed).unwrap(), "k = (+2);\n");
/// ```
pub fn format_antimony(document: &ParsedDocument) -> Result<String, Diagnostic> {
    // The syntax wrapper is opaque but not semantically checked. Never erase an
    // unsupported/invalid construct by attempting a best-effort formatting pass.
    analyze(document)?;
    let tokens = lex(document.source(), &document.limits)?;
    let mut writer = Writer {
        document,
        output: String::new(),
    };
    let mut reaction_starts = document
        .statements
        .iter()
        .filter_map(|statement| {
            if let crate::syntax::Statement::Reaction { start, .. } = statement {
                Some(start.start())
            } else {
                None
            }
        })
        .collect::<std::collections::BTreeSet<_>>();
    for module in &document.modules {
        for statement in &module.statements {
            if let crate::syntax::Statement::Reaction { start, .. } = statement {
                reaction_starts.insert(start.start());
            }
        }
    }
    let mut pos = skip_separators(&tokens, 0);
    while tokens[pos].kind != Kind::Eof {
        if word(&tokens[pos], "function") {
            pos = format_function_block(&mut writer, &tokens, pos)?;
        } else if word(&tokens[pos], "model") || word(&tokens[pos], "module") {
            pos = format_model_block(&mut writer, &tokens, pos, &reaction_starts)?;
        } else {
            pos = format_statement_line(&mut writer, &tokens, pos, false, &reaction_starts)?;
        }
    }
    let output = writer.output;
    // Check the actual generated text, not an estimate of its token count. Map
    // all generated locations back to the original source: callers own source
    // IDs, and no generated SourceFile is exposed by this API.
    let generated = SourceFile::new(
        document.source().id(),
        "canonical output (internal)",
        &output,
        &document.limits,
    )
    .map_err(|e| format_error(document, DiagnosticCode::ResourceLimit, e.to_string()))?;
    let parsed =
        parse(&generated, &document.limits).map_err(|d| generated_error(document, "parse", d))?;
    analyze(&parsed).map_err(|d| generated_error(document, "analysis", d))?;
    Ok(output)
}

fn format_model_block(
    writer: &mut Writer<'_>,
    tokens: &[Token],
    pos: usize,
    reaction_starts: &std::collections::BTreeSet<usize>,
) -> Result<usize, Diagnostic> {
    let star_offset = usize::from(tokens[pos + 1].kind == Kind::Star);
    let mut header_end = pos + 2 + star_offset;
    if tokens[header_end].kind == Kind::LParen {
        while tokens[header_end].kind != Kind::RParen {
            header_end += 1;
        }
        header_end += 1;
    }
    writer.tokens(&tokens[pos..header_end])?;
    writer.push("\n")?;
    let mut body_pos = skip_separators(tokens, header_end);
    while tokens[body_pos].kind != Kind::Eof && !word(&tokens[body_pos], "end") {
        body_pos = format_statement_line(writer, tokens, body_pos, true, reaction_starts)?;
    }
    debug_assert!(word(&tokens[body_pos], "end"));
    writer.push("end\n")?;
    Ok(skip_separators(tokens, body_pos + 1))
}

fn format_statement_line(
    writer: &mut Writer<'_>,
    tokens: &[Token],
    mut pos: usize,
    indent: bool,
    reaction_starts: &std::collections::BTreeSet<usize>,
) -> Result<usize, Diagnostic> {
    if indent {
        writer.push("  ")?;
    }
    let start = pos;
    while !boundary(&tokens[pos]) {
        pos += 1;
    }
    writer.tokens(&tokens[start..pos])?;
    if reaction_starts.contains(&tokens[start].span.start()) {
        // A parsed reaction has one mandatory internal semicolon. Empty
        // separator runs after it are trivia; its rate stays on this line.
        writer.push("; ")?;
        pos = skip_separators(tokens, pos);
        let rate_start = pos;
        while !boundary(&tokens[pos]) {
            pos += 1;
        }
        writer.tokens(&tokens[rate_start..pos])?;
    }
    writer.push(";\n")?;
    Ok(skip_separators(tokens, pos))
}

fn format_function_block(
    writer: &mut Writer<'_>,
    tokens: &[Token],
    pos: usize,
) -> Result<usize, Diagnostic> {
    let mut header_end = pos + 2;
    while tokens[header_end].kind != Kind::RParen {
        header_end += 1;
    }
    header_end += 1;
    writer.tokens(&tokens[pos..header_end])?;
    writer.push("\n  ")?;
    let mut body_pos = skip_separators(tokens, header_end);
    let body_start = body_pos;
    while !boundary(&tokens[body_pos]) {
        body_pos += 1;
    }
    writer.tokens(&tokens[body_start..body_pos])?;
    writer.push(";\n")?;
    body_pos = skip_separators(tokens, body_pos);
    debug_assert!(word(&tokens[body_pos], "end"));
    writer.push("end\n")?;
    Ok(skip_separators(tokens, body_pos + 1))
}

fn word(token: &Token, expected: &str) -> bool {
    matches!(&token.kind, Kind::Ident(s) if s == expected)
}
fn separator(token: &Token) -> bool {
    matches!(token.kind, Kind::Sep | Kind::Newline)
}
fn skip_separators(tokens: &[Token], mut pos: usize) -> usize {
    while separator(&tokens[pos]) {
        pos += 1;
    }
    pos
}
fn boundary(token: &Token) -> bool {
    separator(token) || token.kind == Kind::Eof || word(token, "end")
}
fn format_error(
    document: &ParsedDocument,
    code: DiagnosticCode,
    message: impl Into<String>,
) -> Diagnostic {
    error(
        code,
        document.source().span(0, 0).expect("valid source start"),
        message,
    )
}
fn generated_error(document: &ParsedDocument, stage: &str, diagnostic: Diagnostic) -> Diagnostic {
    let code = if diagnostic.code() == DiagnosticCode::ResourceLimit {
        DiagnosticCode::ResourceLimit
    } else {
        DiagnosticCode::Export
    };
    format_error(
        document,
        code,
        format!("canonical output {stage} failed: {}", diagnostic.message()),
    )
    .with_note("The diagnostic location refers to the original source; no generated artifact was returned.")
}
struct Writer<'a> {
    document: &'a ParsedDocument,
    output: String,
}
impl Writer<'_> {
    fn push(&mut self, text: &str) -> Result<(), Diagnostic> {
        self.document
            .limits
            .checked_add(LimitKind::SourceBytes, self.output.len(), text.len())
            .map_err(|e| {
                format_error(self.document, DiagnosticCode::ResourceLimit, e.to_string())
            })?;
        self.output.push_str(text);
        Ok(())
    }
    fn tokens(&mut self, tokens: &[Token]) -> Result<(), Diagnostic> {
        let mut previous: Option<&Kind> = None;
        let mut previous_unary = false;
        for token in tokens {
            let unary = matches!(token.kind, Kind::Plus | Kind::Minus | Kind::Not)
                && !matches!(
                    previous,
                    Some(Kind::Ident(_) | Kind::Number(_) | Kind::RParen)
                );
            let tight = matches!(
                token.kind,
                Kind::RParen | Kind::Comma | Kind::Colon | Kind::PrimeEqual | Kind::Dot
            ) || matches!(previous, Some(Kind::LParen | Kind::Dollar | Kind::Dot))
                || (previous == Some(&Kind::Star)
                    && matches!(token.kind, Kind::Ident(_))
                    && tokens
                        .first()
                        .is_some_and(|first| word(first, "model") || word(first, "module")))
                || previous_unary
                || (token.kind == Kind::LParen
                    && matches!(
                        previous,
                        Some(Kind::Ident(s))
                            if !matches!(s.as_str(), "at" | "after" | "is" | "in" | "has" | "delete")
                    ));
            if previous.is_some()
                && (!tight || (previous == Some(&Kind::Minus) && token.kind == Kind::Minus))
            {
                self.push(" ")?;
            }
            // Merged operator spans can contain removed comments; emit the
            // canonical symbolic token rather than copying that raw span.
            let operator = match token.kind {
                Kind::ColonEqual => Some(":="),
                Kind::PrimeEqual => Some("' ="),
                Kind::Compare(crate::ComparisonOp::Equal) => Some("=="),
                Kind::Compare(crate::ComparisonOp::NotEqual) => Some("!="),
                Kind::Compare(crate::ComparisonOp::Less) => Some("<"),
                Kind::Compare(crate::ComparisonOp::LessEqual) => Some("<="),
                Kind::Compare(crate::ComparisonOp::Greater) => Some(">"),
                Kind::Compare(crate::ComparisonOp::GreaterEqual) => Some(">="),
                Kind::Logical(crate::LogicalOp::And) => Some("&&"),
                Kind::Logical(crate::LogicalOp::Or) => Some("||"),
                _ => None,
            };
            self.push(operator.unwrap_or_else(|| {
                self.document
                    .source()
                    .slice(token.span)
                    .expect("retained source token")
            }))?;
            previous = Some(&token.kind);
            previous_unary = unary;
        }
        Ok(())
    }
}
