//! Native-owner regressions for the bounded conditional foundation.
use antimony::{
    Diagnostic, DiagnosticCode, ExpressionId, ExpressionKind, LimitKind, Limits, LogicalOp,
    SourceFile, SourceId, ValidatedModel, analyze, format_antimony, parse,
};

fn compile(source: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let file =
        SourceFile::new(SourceId::new(307), "conditional regression", source, limits).unwrap();
    analyze(&parse(&file, limits)?)
}
fn model(source: &str) -> ValidatedModel {
    compile(source, &Limits::default()).unwrap()
}
fn value(m: &ValidatedModel, name: &str) -> Option<f64> {
    m.parameters()
        .iter()
        .find(|p| p.id() == name)
        .unwrap()
        .value()
}
fn root(m: &ValidatedModel) -> ExpressionId {
    m.initial_assignments()[0].expression()
}
fn rejects(source: &str, code: DiagnosticCode) {
    let e = compile(source, &Limits::default()).unwrap_err();
    assert_eq!(e.code(), code, "{source}: {e:?}");
    assert!(!e.labels().is_empty());
}

#[test]
fn boolean_alias_calls_are_exact_and_keep_leaf_identity() {
    for (name, b) in [
        ("True", true),
        ("TRUE", true),
        ("False", false),
        ("FALSE", false),
    ] {
        for gap in ["", " ", "/*ignored*/"] {
            let m = model(&format!("a={name}{gap}();"));
            assert!(
                matches!(m.expression(root(&m)).unwrap().kind(),ExpressionKind::Boolean(v) if *v==b)
            );
            assert_eq!(value(&m, "a"), Some(if b { 1.0 } else { 0.0 }));
        }
        for args in ["0", "false", "1,2"] {
            rejects(&format!("a={name}({args});"), DiagnosticCode::Syntax);
        }
    }
    for name in ["true", "false"] {
        rejects(&format!("a={name}();"), DiagnosticCode::Syntax);
    }
}

#[test]
fn finite_boolean_conversion_composes_with_numeric_functions() {
    let m =
        model("a=true+2;b=-false;c=sin(true);d=pow(true,2);e=not(-2);f=piecewise(true,0.5,false);");
    for (name, v) in [
        ("a", 3.0),
        ("b", 0.0),
        ("c", 1_f64.sin()),
        ("d", 1.0),
        ("e", 0.0),
        ("f", 1.0),
    ] {
        assert_eq!(value(&m, name), Some(v));
    }
}

#[test]
fn unknown_logical_operands_stop_without_guessing_or_inspecting_later_domains() {
    let m = model(
        "a=and(u,false);b=or(u,true);c=and(u,sqrt(-1));d=or(u,sqrt(-1));e=and(false,u);f=or(true,u);",
    );
    for n in ["a", "b", "c", "d"] {
        assert_eq!(value(&m, n), None);
    }
    assert_eq!(value(&m, "e"), Some(0.0));
    assert_eq!(value(&m, "f"), Some(1.0));
}

#[test]
fn comparisons_eagerly_demand_every_argument_despite_unknown_or_false_pair() {
    for s in [
        "a=lt(u,sqrt(-1));",
        "a=lt(2,1,sqrt(-1));",
        "a=eq(u,log(1,u));",
    ] {
        rejects(s, DiagnosticCode::DeclarationConflict);
    }
}

#[test]
fn piecewise_evaluates_only_ordered_demand_and_preserves_unknown() {
    let m = model(
        "a=piecewise(7,true,1/0);b=piecewise(sqrt(-1),false,8);c=piecewise(sqrt(-1),u,7);d=piecewise(7,false,8,u,9);e=piecewise(5);",
    );
    assert_eq!(value(&m, "a"), Some(7.0));
    assert_eq!(value(&m, "b"), Some(8.0));
    assert_eq!(value(&m, "c"), None);
    assert_eq!(value(&m, "d"), None);
    assert_eq!(value(&m, "e"), Some(5.0));
    rejects("a=piecewise(7,false);", DiagnosticCode::DeclarationConflict);
    assert!(compile("species S;J:=>S;piecewise(7,false);", &Limits::default()).is_ok());
}

#[test]
fn inactive_branches_still_have_static_dependencies_units_and_literal_checks() {
    rejects(
        "a=piecewise(a,false,1);",
        DiagnosticCode::InitializationCycle,
    );
    rejects(
        "x=1;x has second;y=2;y has metre;a=piecewise(u,false,x,true,y);",
        DiagnosticCode::UnitsInconsistency,
    );
    rejects(
        "a=piecewise(1,true,(1/0));",
        DiagnosticCode::UnsupportedFeature,
    );
    rejects(
        "a=piecewise(1,true,NaN);",
        DiagnosticCode::UnsupportedFeature,
    );
    let m = model("a=piecewise(u,false,1);");
    assert_eq!(value(&m, "a"), Some(1.0));
    assert_eq!(value(&m, "u"), None);
}

#[test]
fn logical_outer_infix_appends_but_outer_function_and_right_groups_do_not() {
    for (source, len) in [
        ("a=and(true,false)&&true;", 3),
        ("a=and(and(true,false),true);", 2),
        ("a=true&&(false&&true);", 2),
        ("a=and()&&true;", 1),
    ] {
        let m = model(source);
        assert!(
            matches!(m.expression(root(&m)).unwrap().kind(),ExpressionKind::Logical{op:LogicalOp::And,args} if args.len()==len)
        );
    }
    assert_eq!(value(&model("a=true||false&&false;"), "a"), Some(0.0));
}

#[test]
fn comparison_chain_guard_sees_normalized_wrappers_not_right_nesting() {
    for e in [
        "lt(1,2)<3",
        "+(1<2)<3",
        "-(-(1<2))<3",
        "and(lt(1,2),gt(3,2))<4",
    ] {
        rejects(&format!("a={e};"), DiagnosticCode::UnsupportedFeature);
    }
    for e in ["1<(2<3)", "lt(lt(1,2),3)", "and(lt(1,2))<4", "-(1<2)<3"] {
        model(&format!("a={e};"));
    }
}

#[test]
fn selective_token_fusion_removes_comments_and_roundtrips() {
    for e in [
        "1= /*a\nb*/ =1",
        "true& /*x*/ &false",
        "false|\t|true",
        "! !0",
    ] {
        let limits = Limits::default();
        let s = format!("a={e};");
        let file = SourceFile::new(SourceId::new(307), "layout", &s, &limits).unwrap();
        let p = parse(&file, &limits).unwrap();
        let before = analyze(&p).unwrap();
        let output = format_antimony(&p).unwrap();
        assert!(!output.contains("/*"));
        let afterfile = SourceFile::new(SourceId::new(308), "formatted", &output, &limits).unwrap();
        let afterparse = parse(&afterfile, &limits).unwrap();
        let after = analyze(&afterparse).unwrap();
        assert_eq!(output, format_antimony(&afterparse).unwrap());
        assert_eq!(value(&before, "a"), value(&after, "a"));
        #[cfg(feature = "sbml")]
        assert_eq!(
            antimony::write_sbml(&before).unwrap().xml(),
            antimony::write_sbml(&after).unwrap().xml()
        );
    }
    for e in ["1< =2", "1> =2", "1! =2", "1=\n=1", "true&\n&false"] {
        rejects(&format!("a={e};"), DiagnosticCode::Syntax);
    }
}

#[test]
fn fusion_charges_original_tokens_before_coalescing() {
    // a, =, 1, =, =, 1, ;, EOF: fusion must not hide an original token.
    let source = "a=1= =1;";
    assert!(compile(source, &Limits::default().with_limit(LimitKind::Tokens, 8)).is_ok());
    let e = compile(source, &Limits::default().with_limit(LimitKind::Tokens, 6)).unwrap_err();
    assert_eq!(e.code(), DiagnosticCode::ResourceLimit);
}

#[test]
fn wide_function_then_infix_append_has_one_retained_vector_and_bounded_nodes() {
    const WIDTH: usize = 5000;
    const APPENDS: usize = 64;
    let source = format!(
        "a=and({}){};",
        vec!["true"; WIDTH].join(","),
        "&&true".repeat(APPENDS)
    );
    let m = model(&source);
    let ExpressionKind::Logical { args, .. } = m.expression(root(&m)).unwrap().kind() else {
        panic!("logical root")
    };
    assert_eq!(args.len(), WIDTH + APPENDS);
    assert!(args.iter().all(|id| matches!(
        m.expression(*id).unwrap().kind(),
        ExpressionKind::Boolean(true)
    )));
    assert_eq!(value(&m, "a"), Some(1.0));
    let e = compile(
        &source,
        &Limits::default().with_limit(LimitKind::Tokens, WIDTH),
    )
    .unwrap_err();
    assert_eq!(e.code(), DiagnosticCode::ResourceLimit);
}

#[test]
fn textual_aliases_and_deferred_language_do_not_leak() {
    for s in [
        "a=true and false;",
        "a=not true;",
        "a=true OR false;",
        "a=xor(true,false);",
        "a=implies(true,false);",
    ] {
        rejects(s, DiagnosticCode::UnsupportedFeature);
    }
}

#[cfg(feature = "sbml")]
#[test]
fn piecewise_mathml_is_structural_not_an_apply_or_folded_constant() {
    let m = model("a=piecewise(7,true,9);b=not(false);c=lt(1,2,3);");
    let xml = antimony::write_sbml(&m).unwrap();
    let xml = xml.xml();
    assert!(xml.contains("<piecewise>"));
    assert!(xml.contains("<piece>"));
    assert!(xml.contains("<otherwise>"));
    assert!(!xml.contains("<apply><piecewise"));
    assert!(xml.contains("<true/>"));
    assert!(xml.contains("<not/>"));
}
