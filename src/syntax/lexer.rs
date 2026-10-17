use super::error;
use crate::{Diagnostic, DiagnosticCode, LimitKind, Limits, SourceFile, Span};
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Ident(String),
    Number(String),
    Sep,
    Newline,
    LParen,
    RParen,
    Comma,
    Colon,
    Prime,
    ColonEqual,
    PrimeEqual,
    Equal,
    Compare(crate::ComparisonOp),
    Logical(crate::LogicalOp),
    HalfLogical(crate::LogicalOp),
    Not,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Dollar,
    Dot,
    Arrow(bool),
    Unsupported,
    Eof,
}
#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub kind: Kind,
    pub span: Span,
}
pub(crate) fn lex(source: &SourceFile, limits: &Limits) -> Result<Vec<Token>, Diagnostic> {
    let text = source.text();
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let kind = match bytes[i] {
            b' ' | b'\t' => {
                i += 1;
                continue;
            }
            b'\r' => {
                i += 1;
                if bytes.get(i) == Some(&b'\n') {
                    i += 1;
                }
                Kind::Newline
            }
            b'\n' => {
                i += 1;
                Kind::Newline
            }
            b';' => {
                i += 1;
                Kind::Sep
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' && bytes[i] != b'\r' {
                    i += 1;
                }
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                    i += 1;
                }
                if i + 1 >= bytes.len() {
                    return Err(error(
                        DiagnosticCode::Syntax,
                        source.span(start, text.len()).expect("source boundaries"),
                        "unterminated block comment",
                    ));
                }
                i += 2;
                continue;
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                Kind::Ident(text[start..i].to_owned())
            }
            b'0'..=b'9' | b'.'
                if bytes[i].is_ascii_digit()
                    || bytes.get(i + 1).is_some_and(u8::is_ascii_digit) =>
            {
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if bytes.get(i) == Some(&b'.') {
                    i += 1;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if matches!(bytes.get(i), Some(b'e' | b'E')) {
                    i += 1;
                    if matches!(bytes.get(i), Some(b'+' | b'-')) {
                        i += 1;
                    }
                    let exponent_start = i;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i == exponent_start {
                        return Err(error(
                            DiagnosticCode::Syntax,
                            source.span(start, i).expect("ASCII boundaries"),
                            "numeric exponent requires digits",
                        ));
                    }
                }
                Kind::Number(text[start..i].to_owned())
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                return Err(error(
                    DiagnosticCode::Syntax,
                    source.span(start, start + 2).expect("ASCII token"),
                    "adjacent -- is not an arithmetic operator; separate unary minus tokens",
                ));
            }
            b'-' if bytes.get(i + 1) == Some(&b'>') => {
                i += 2;
                Kind::Arrow(true)
            }
            b'=' if bytes.get(i + 1) == Some(&b'>') => {
                i += 2;
                Kind::Arrow(false)
            }
            b'(' => {
                i += 1;
                Kind::LParen
            }
            b')' => {
                i += 1;
                Kind::RParen
            }
            b',' => {
                i += 1;
                Kind::Comma
            }
            b':' => {
                i += 1;
                Kind::Colon
            }
            b'\'' => {
                i += 1;
                Kind::Prime
            }
            b'<' | b'>' | b'=' | b'!' if bytes.get(i + 1) == Some(&b'=') => {
                let op = match bytes[i] {
                    b'<' => crate::ComparisonOp::LessEqual,
                    b'>' => crate::ComparisonOp::GreaterEqual,
                    b'=' => crate::ComparisonOp::Equal,
                    _ => crate::ComparisonOp::NotEqual,
                };
                i += 2;
                Kind::Compare(op)
            }
            b'<' | b'>' => {
                let op = if bytes[i] == b'<' {
                    crate::ComparisonOp::Less
                } else {
                    crate::ComparisonOp::Greater
                };
                i += 1;
                Kind::Compare(op)
            }
            b'!' => {
                i += 1;
                Kind::Not
            }
            b'&' | b'|' if bytes.get(i + 1) == Some(&bytes[i]) => {
                let op = if bytes[i] == b'&' {
                    crate::LogicalOp::And
                } else {
                    crate::LogicalOp::Or
                };
                i += 2;
                Kind::Logical(op)
            }
            b'&' | b'|' => {
                let op = if bytes[i] == b'&' {
                    crate::LogicalOp::And
                } else {
                    crate::LogicalOp::Or
                };
                i += 1;
                Kind::HalfLogical(op)
            }
            b'=' => {
                i += 1;
                Kind::Equal
            }
            b'+' => {
                i += 1;
                Kind::Plus
            }
            b'-' => {
                i += 1;
                Kind::Minus
            }
            b'*' => {
                i += 1;
                Kind::Star
            }
            b'/' => {
                i += 1;
                Kind::Slash
            }
            b'^' => {
                i += 1;
                Kind::Caret
            }
            b'$' => {
                i += 1;
                Kind::Dollar
            }
            b'.' => {
                i += 1;
                Kind::Dot
            }
            b'"' | b'[' | b']' | b'{' | b'}' | b'@' => {
                i += 1;
                Kind::Unsupported
            }
            _ => {
                let end = i + text[i..].chars().next().expect("in bounds").len_utf8();
                return Err(error(
                    DiagnosticCode::Syntax,
                    source.span(i, end).expect("character boundary"),
                    "unrecognized token; identifiers in this slice are ASCII",
                ));
            }
        };
        let span = source
            .span(start, i)
            .expect("lexer endpoints are UTF-8 boundaries");
        limits
            .checked_add(LimitKind::Tokens, out.len(), 1)
            .map_err(|e| error(DiagnosticCode::ResourceLimit, span, e.to_string()))?;
        out.push(Token { kind, span });
    }
    let span = source.span(i, i).expect("EOF boundary");
    limits
        .checked_add(LimitKind::Tokens, out.len(), 1)
        .map_err(|e| error(DiagnosticCode::ResourceLimit, span, e.to_string()))?;
    out.push(Token {
        kind: Kind::Eof,
        span,
    });
    // The reference reconstructs ==/&&/||/:=/'= from separate tokens across spaces
    // and removed block comments. It does NOT do so for <=/>=/!=. Count original
    // tokens above, before this bounded compaction; actual Newline tokens block it.
    let mut merged = Vec::with_capacity(out.len());
    let mut idx = 0;
    while idx < out.len() {
        let mut token = out[idx].clone();
        let next = out.get(idx + 1).map(|t| &t.kind);
        let after_next = out.get(idx + 2).map(|t| &t.kind);
        let combined = match (&token.kind, next) {
            (Kind::Colon, Some(Kind::Equal)) if after_next != Some(&Kind::Equal) => {
                Some(Kind::ColonEqual)
            }
            (Kind::Prime, Some(Kind::Equal)) if after_next != Some(&Kind::Equal) => {
                Some(Kind::PrimeEqual)
            }
            (Kind::Equal, Some(Kind::Equal)) => Some(Kind::Compare(crate::ComparisonOp::Equal)),
            (Kind::HalfLogical(a), Some(Kind::HalfLogical(b))) if a == b => Some(Kind::Logical(*a)),
            _ => None,
        };
        if let Some(kind) = combined {
            let end = out[idx + 1].span.end();
            token.span = source
                .span(token.span.start(), end)
                .expect("merged operator span");
            token.kind = kind;
            idx += 2;
        } else if matches!(token.kind, Kind::HalfLogical(_)) {
            return Err(error(
                DiagnosticCode::Syntax,
                token.span,
                "logical operators require doubled && or ||",
            ));
        } else if token.kind == Kind::Prime {
            return Err(error(
                DiagnosticCode::Syntax,
                token.span,
                "prime operator requires = to form a rate rule",
            ));
        } else {
            idx += 1;
        }
        merged.push(token);
    }
    Ok(merged)
}
