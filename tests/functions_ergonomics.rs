//! Ordinary authoring and adversarial integration tests for `functions-core-v0.5-slice`:
//! public `FunctionDefinition` IR, `SymbolKind::FunctionParameter`, `ExpressionKind::UserCall`,
//! lexical isolation of formal parameters (`<bvar>`), declaration-order non-recursive call
//! resolution, initial-value evaluation across nested user calls, conservative call-site
//! substitution for unit/dimension checks, explicit rejections, and formatter/SBML roundtrip.
use antimony::{
    Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionKind, IMPLEMENTED_PROFILE, Limits,
    ParsedDocument, RuleKind, SourceFile, SourceId, SymbolKind, ValidatedModel, analyze,
    format_antimony, parse,
};

fn parsed(text: &str) -> Result<ParsedDocument, Diagnostic> {
    let limits = Limits::default();
    let source =
        SourceFile::new(SourceId::new(701), "functions_ergonomics", text, &limits).unwrap();
    parse(&source, &limits)
}

fn compile_err_code(text: &str) -> DiagnosticCode {
    match parsed(text) {
        Err(diag) => diag.code(),
        Ok(doc) => analyze(&doc)
            .expect_err("expected compilation failure")
            .code(),
    }
}

fn check_roundtrip(text: &str) -> (ValidatedModel, String) {
    let limits = Limits::default();
    let doc = parsed(text).expect("source should parse");
    let model = analyze(&doc).expect("source should analyze");
    let formatted = format_antimony(&doc).expect("source should format");

    let reparsed_source = SourceFile::new(
        SourceId::new(702),
        "functions_ergonomics_formatted",
        &formatted,
        &limits,
    )
    .unwrap();
    let reparsed = parse(&reparsed_source, &limits).expect("formatted output should parse");
    let reanalyzed = analyze(&reparsed).expect("formatted output should analyze");
    let reformatted = format_antimony(&reparsed).expect("formatted output should reformat");

    assert_eq!(reformatted, formatted, "formatter must be byte-idempotent");
    assert_eq!(model.id(), reanalyzed.id());

    let snap_fns = |m: &ValidatedModel| {
        m.function_definitions()
            .iter()
            .map(|f| {
                (
                    f.id().to_owned(),
                    f.parameters().to_vec(),
                    format!("{:?}", m.expression(f.body()).unwrap().kind()),
                )
            })
            .collect::<Vec<_>>()
    };
    let snap_params = |m: &ValidatedModel| {
        m.parameters()
            .iter()
            .map(|p| {
                (
                    p.id().to_owned(),
                    p.value(),
                    p.initial_expression().is_some(),
                    p.constant(),
                )
            })
            .collect::<Vec<_>>()
    };
    let snap_rules = |m: &ValidatedModel| {
        m.rules()
            .iter()
            .map(|r| {
                (
                    r.kind(),
                    r.target().to_owned(),
                    format!("{:?}", m.expression(r.expression()).unwrap().kind()),
                )
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(snap_fns(&model), snap_fns(&reanalyzed));
    assert_eq!(snap_params(&model), snap_params(&reanalyzed));
    assert_eq!(snap_rules(&model), snap_rules(&reanalyzed));

    #[cfg(feature = "sbml")]
    {
        let xml1 = antimony::write_sbml(&model).unwrap().xml().to_owned();
        let xml2 = antimony::write_sbml(&reanalyzed).unwrap().xml().to_owned();
        assert_eq!(xml1, xml2);
        assert!(xml1.contains("<listOfFunctionDefinitions>"));
        assert!(xml1.contains("<lambda>"));
    }

    (model, formatted)
}

#[test]
fn public_function_definition_ir_and_user_call_nodes() {
    let source = "\
function base_rate() 0.25 end
function scale_add(x, y) base_rate() + 2 * x + y end
model fn_demo
  k = scale_add(3, 1.5);
  y := scale_add(k, 0.5);
end";
    let (model, formatted) = check_roundtrip(source);
    assert_eq!(IMPLEMENTED_PROFILE, "modular-core-v0.8-slice");
    assert_eq!(model.profile_id(), "modular-core-v0.8-slice");
    assert!(formatted.contains("function base_rate()"));
    assert!(formatted.contains("function scale_add(x, y)"));
    assert!(formatted.contains("model fn_demo"));

    let fns = model.function_definitions();
    assert_eq!(fns.len(), 2);
    assert_eq!(fns[0].id(), "base_rate");
    assert!(fns[0].parameters().is_empty());
    assert!(!fns[0].source_span().is_empty());

    assert_eq!(fns[1].id(), "scale_add");
    assert_eq!(fns[1].parameters(), &["x".to_owned(), "y".to_owned()]);
    assert!(!fns[1].source_span().is_empty());

    // Inspect `<bvar>` formal parameter symbol resolution inside function body.
    let mut saw_param_x = false;
    let mut saw_zero_arg_call = false;
    let mut stack = vec![fns[1].body()];
    while let Some(id) = stack.pop() {
        let node = model.expression(id).unwrap();
        match node.kind() {
            ExpressionKind::Symbol { name, kind } => {
                if name == "x" {
                    assert_eq!(*kind, SymbolKind::FunctionParameter);
                    saw_param_x = true;
                }
            }
            ExpressionKind::UserCall { name, args } => {
                if name == "base_rate" {
                    assert!(args.is_empty());
                    saw_zero_arg_call = true;
                }
            }
            ExpressionKind::Unary { child, .. } => stack.push(*child),
            ExpressionKind::Binary { left, right, .. } => {
                stack.push(*left);
                stack.push(*right);
            }
            _ => (),
        }
    }
    assert!(saw_param_x, "expected FunctionParameter symbol in body");
    assert!(saw_zero_arg_call, "expected UserCall to base_rate()");

    // Inspect call-site ExpressionKind::UserCall in initial assignment and rule.
    let ia = &model.initial_assignments()[0];
    assert_eq!(ia.symbol(), "k");
    assert!(matches!(
        model.expression(ia.expression()).unwrap().kind(),
        ExpressionKind::UserCall { name, args } if name == "scale_add" && args.len() == 2
    ));

    let rule = &model.rules()[0];
    assert_eq!(rule.kind(), RuleKind::Assignment);
    assert_eq!(rule.target(), "y");
    assert!(matches!(
        model.expression(rule.expression()).unwrap().kind(),
        ExpressionKind::UserCall { name, args } if name == "scale_add" && args.len() == 2
    ));
}

#[test]
fn formal_parameter_shadowing_does_not_leak_or_create_model_parameters() {
    let source = "\
function mm(S, Vmax, Km) Vmax * S / (Km + S) end
species S = 4;
Vmax = 2;
Km = 1;
J0: S -> ; mm(S, Vmax, Km);";
    let (model, _) = check_roundtrip(source);
    assert_eq!(model.function_definitions().len(), 1);
    assert_eq!(model.species().len(), 1);
    assert_eq!(model.species()[0].id(), "S");
    assert_eq!(
        model
            .parameters()
            .iter()
            .map(|p| p.id())
            .collect::<Vec<_>>(),
        vec!["Km", "Vmax"]
    );

    // Formal parameters inside a function body that never appear outside do not create parameters.
    let (isolated, _) = check_roundtrip("function f(alpha, beta) alpha + beta end\na = f(1, 2);");
    assert_eq!(
        isolated
            .parameters()
            .iter()
            .map(|p| p.id())
            .collect::<Vec<_>>(),
        vec!["a"]
    );
}

#[test]
fn unit_checking_propagates_through_user_defined_functions() {
    let valid = "\
unit s_u = second;
unit per_sec = 1 / second;
function phase(w, t) sin(w * t) end
w = 2; w has per_sec;
t0 = 0.5; t0 has s_u;
a = phase(w, t0);";
    let (model, _) = check_roundtrip(valid);
    assert!(matches!(
        model.symbol_dimensions("w"),
        Some(DimensionalStatus::Known(_))
    ));

    // Dimension mismatch inside function body when called with incompatible arguments.
    assert_eq!(
        compile_err_code(
            "unit s_u = second; unit m_u = metre; function add2(x, y) x + y end; t0 = 1; t0 has s_u; x0 = 2; x0 has m_u; a = add2(t0, x0);"
        ),
        DiagnosticCode::UnitsInconsistency
    );
    // Dimensioned argument passed to trig builtin inside function body.
    assert_eq!(
        compile_err_code(
            "unit s_u = second; function s(x) sin(x) end; t0 = 1; t0 has s_u; a = s(t0);"
        ),
        DiagnosticCode::UnitsInconsistency
    );
    // Return dimension of function call incompatible with target symbol unit.
    assert_eq!(
        compile_err_code(
            "unit s_u = second; unit m_u = metre; function dbl(x) x + x end; t0 = 1; t0 has s_u; a = 1; a has m_u; a := dbl(t0);"
        ),
        DiagnosticCode::UnitsInconsistency
    );
}

#[test]
fn adversarial_function_rejections_preserve_diagnostic_categories() {
    // Syntax rejections
    for src in [
        "function () 1 end; a = 1;",
        "function f(x) x + 1",
        "function f(x,) x end; a = f(1);",
        "function f(x) x; x + 1 end; a = f(1);",
        "function f(x) y = x end; a = 1;",
        "function f(x) x + 1 end; a = f;",
        "function f(x) x + 1 end; f = 2;",
        "function f(x) x + 1 end; f := 2;",
        "function liter(x) x end; a = 1;",
        "function f(true) 1 end; a = 1;",
    ] {
        assert_eq!(
            compile_err_code(src),
            DiagnosticCode::Syntax,
            "expected Syntax for {src:?}"
        );
    }

    // Declaration conflict rejections
    for src in [
        "function f(x) x end; function f(x) x + 1 end; a = f(1);",
        "function f(x) x end; function f(x, y) x + y end; a = f(1);",
        "function f(x, x) x end; a = f(1, 2);",
        "function f(f) f end; a = 1;",
        "function f(x) x + g end; a = f(1);",
        "g = 2; function f(x) x + g end; a = f(1);",
        "function f(x) x + time end; a = f(1);",
        "function f(x) f(x) end; a = f(1);",
        "function f(x, y) x + y end; a = f(1);",
        "function f(x) x end; a = f(1, 2);",
        "f = 1; function f(x) x end;",
        "function f(x) x end; species f = 1;",
        "function _J0(x) x + 1 end; species A = 1; A -> ; 0.1 * A;",
        "function f(x) 1 / x end; a = f(0);",
        "function f(x) ln(x) end; a = f(-1);",
    ] {
        assert_eq!(
            compile_err_code(src),
            DiagnosticCode::DeclarationConflict,
            "expected DeclarationConflict for {src:?}"
        );
    }

    // Initialization cycle rejections through function call arguments
    for src in [
        "function id(x) x end; a = id(b); b = id(a);",
        "function id(x) x end; a := id(b); b := id(a);",
        "function id(x) x end; a := id(a);",
        "function id(x) x end; a := id(b); b := id(a); a := 1;",
    ] {
        assert_eq!(
            compile_err_code(src),
            DiagnosticCode::InitializationCycle,
            "expected InitializationCycle for {src:?}"
        );
    }

    // Compartment function calls are now supported in modular-core-v0.8-slice
    for src in [
        "function f(x) x end; compartment C = f(1);",
        "function f(x) x end; compartment C = 1; C := f(2);",
    ] {
        let _ = check_roundtrip(src);
    }

    // Unsupported feature rejections
    for src in [
        "a = unknown_fn(1);",
        "a = f(1); function f(x) x + 1 end",
        "function f(x) g(x) end; function g(x) x + 1 end; a = f(1);",
        "function f($x) x end; a = f(1);",
        "function sin(x) x end; a = 1;",
        "function f(pi) 1 end; a = 1;",
        "function f(time) 1 end; a = 1;",
        "function f(x) end; a = 1;",
        "function f(x) ; end; a = 1;",
        "function f(x) (1/0) * x end; a = 1;",
    ] {
        assert_eq!(
            compile_err_code(src),
            DiagnosticCode::UnsupportedFeature,
            "expected UnsupportedFeature for {src:?}"
        );
    }
}
