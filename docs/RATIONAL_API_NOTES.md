# Rational Atoms vs. Division in `math-core-v0.2.1-slice`

> **Status:** Independently verified bounded `math-core-v0.2.1-slice`; see the [corrective evidence](RATIONAL_CALLS_EVIDENCE.md). Successful parsing or analysis is structural only and makes no runtime, solver, biological, or simulation-readiness claim.

## Why `(1/2)` Is a Rational Atom While `1/2` Is Division

Pinned upstream Antimony 3.2.0 formats numeric tokens at precision 15 and passes formula strings to libSBML's L3 parser:

- **Unparenthesized `1/2`** parses as ordinary binary division (`BinaryOp::Divide`, MathML `<divide/>`).
- **Parenthesized `(1/2)`** (optional single `-` on each integer-formatted component after precision-15 default formatting, enclosed directly in `(...)`) is lexed upstream as a single rational-notation token, including its outer parentheses. It produces an exact, unreduced rational leaf (`ExpressionKind::Rational { numerator, denominator }`, MathML `<cn type="rational">...<sep/>...</cn>`). Common factors are never cancelled (e.g., `(2/-4)` keeps `numerator: 2, denominator: -4`), and denominator signs are not normalized.

## Why `sin(1/2)` Is a Syntax Error and How Alternatives Differ

Because libSBML's lexer consumes `(1/2)`—including its parentheses—as one rational token, writing `sin(1/2)` leaves `sin` without a separate opening-parenthesis token for its argument list. Upstream rejects `sin(1/2)` (and `sin(1.0/2)`, `cos(1/2)`, `sqrt(1/2)`, etc.) as a syntax error, and this slice matches that rejection.

Two valid spellings produce **different AST structures** (neither is universally "better," and algebraically equal rational atoms and division subtrees are **not** structurally interchangeable):

| Source | Parsed / Exported Structure |
|---|---|
| `sin((1/2))` | `Call` (`Builtin::Sin`) whose argument is `ExpressionKind::Rational { numerator: 1, denominator: 2 }` |
| `sin((1)/2)` | `Call` (`Builtin::Sin`) whose argument is `ExpressionKind::Binary { op: BinaryOp::Divide, .. }` |

## Inspecting `ExpressionKind::Rational` (Core Public API)

`Diagnostic` does not implement `std::error::Error`; use `.expect(...)` or explicit pattern matching rather than `?` with `Box<dyn std::error::Error>`. The snippet below uses only the default-free core API (`--no-default-features`):

```rust
use antimony::{BinaryOp, ExpressionKind, Limits, SourceFile, SourceId, analyze, parse};

fn main() {
    let limits = Limits::default();
    let src = "a = (2/-4); b = 1/2;";
    let file = SourceFile::new(SourceId::new(0), "example.ant", src, &limits)
        .expect("fixed source within limits");
    let doc = parse(&file, &limits).expect("valid candidate syntax");
    let model = analyze(&doc).expect("valid candidate model");

    let a_id = model.parameters()[0]
        .initial_expression()
        .expect("parameter a has an initial expression");
    match model.expression(a_id).expect("arena node exists").kind() {
        ExpressionKind::Rational {
            numerator,
            denominator,
        } => {
            assert_eq!((*numerator, *denominator), (2, -4));
        }
        other => panic!("expected Rational, got {other:?}"),
    }

    let b_id = model.parameters()[1]
        .initial_expression()
        .expect("parameter b has an initial expression");
    assert!(matches!(
        model.expression(b_id).expect("arena node exists").kind(),
        ExpressionKind::Binary {
            op: BinaryOp::Divide,
            ..
        }
    ));
}
```

When compiled with the optional `sbml` feature (`antimony::write_sbml`), `ExpressionKind::Rational` lowers to MathML `<cn type="rational">numerator<sep/>denominator</cn>`, whereas `BinaryOp::Divide` lowers to `<apply><divide/>...</apply>`.

## Bounds, Caveats, and Pre-Release API Notice

- **Signed 32-bit component bounds after unary normalization:** Unary minus folds into a rational leaf's numerator (`-(1/2)` becomes `(-1/2)`; `-(-2147483648/2)` rejects as a syntax error because `+2147483648` exceeds `i32::MAX`). Valid final `numerator` and `denominator` values must each fit within `[i32::MIN, i32::MAX]`; the public `i64` fields exist only to avoid intermediate overflow during normalization, not to widen the supported integer range.
- **Zero denominators excluded:** A zero rational denominator (such as `(1/0)`) is rejected as `DiagnosticCode::UnsupportedFeature` in every context because this finite-literal slice excludes nonfinite `INF`/`NaN` rational leaves.
- **Pre-release enum and API expansion:** `antimony` is pre-1.0 (`0.x`). Public enums such as [`ExpressionKind`](../src/model.rs) and [`DiagnosticCode`](../src/lib.rs) may gain variants as additional slices are verified; downstream `match` arms should include a wildcard fallback if forward source compatibility across minor slice updates is desired.

## Authoritative References

- [Rational-token compatibility repair discovery](RATIONAL_CALLS.md)
- [Supplemental rational-call compatibility investigation](RATIONAL_CALLS_EVIDENCE.md)
- [Compatibility and release verification plan](VALIDATION.md)
