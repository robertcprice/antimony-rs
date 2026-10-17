//! Independent post-rational conditional contract and adversarial regressions.
use antimony::{
    ComparisonOp, Diagnostic, DiagnosticCode, DimensionalStatus, ExpressionId, ExpressionKind,
    LimitKind, Limits, LogicalOp, SourceFile, SourceId, ValidatedModel, analyze, format_antimony,
    parse,
};
use std::collections::HashSet;

fn compile(text: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic> {
    let source =
        SourceFile::new(SourceId::new(943), "independent conditionals", text, limits).unwrap();
    analyze(&parse(&source, limits)?)
}
fn model(text: &str) -> ValidatedModel {
    compile(text, &Limits::default()).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}
fn root(m: &ValidatedModel, name: &str) -> ExpressionId {
    m.initial_assignments()
        .iter()
        .find(|x| x.symbol() == name)
        .unwrap()
        .expression()
}
fn value(m: &ValidatedModel, name: &str) -> Option<f64> {
    m.parameters()
        .iter()
        .find(|p| p.id() == name)
        .unwrap()
        .value()
}
fn rejected(text: &str, code: DiagnosticCode) {
    let error = compile(text, &Limits::default()).expect_err(text);
    assert_eq!(error.code(), code, "{text}: {error:?}");
    assert!(!error.labels().is_empty(), "unlocated error for {text}");
}
fn roundtrip(text: &str) -> ValidatedModel {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(943), "conditional layout", text, &limits).unwrap();
    let parsed = parse(&source, &limits).unwrap();
    let before = analyze(&parsed).unwrap();
    let output = format_antimony(&parsed).unwrap();
    let source2 = SourceFile::new(SourceId::new(944), "formatted", &output, &limits).unwrap();
    let parsed2 = parse(&source2, &limits).unwrap();
    let after = analyze(&parsed2).unwrap();
    assert_eq!(output, format_antimony(&parsed2).unwrap());
    assert_eq!(
        before
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value(), p.constant()))
            .collect::<Vec<_>>(),
        after
            .parameters()
            .iter()
            .map(|p| (p.id(), p.value(), p.constant()))
            .collect::<Vec<_>>()
    );
    #[cfg(feature = "sbml")]
    assert_eq!(
        antimony::write_sbml(&before).unwrap().xml(),
        antimony::write_sbml(&after).unwrap().xml()
    );
    before
}
fn logical(m: &ValidatedModel, id: ExpressionId, op: LogicalOp, len: usize) -> Vec<ExpressionId> {
    let ExpressionKind::Logical { op: actual, args } = m.expression(id).unwrap().kind() else {
        panic!("not logical")
    };
    assert_eq!(*actual, op);
    assert_eq!(args.len(), len);
    args.clone()
}

#[test]
fn booleans_keep_typed_nodes_and_numeric_getters_without_invented_symbols() {
    for (alias, expected) in [
        ("true", true),
        ("True", true),
        ("TRUE", true),
        ("false", false),
        ("False", false),
        ("FALSE", false),
    ] {
        let m = roundtrip(&format!("a={alias};"));
        assert!(
            matches!(m.expression(root(&m,"a")).unwrap().kind(),ExpressionKind::Boolean(v) if *v==expected)
        );
        assert_eq!(value(&m, "a"), Some(if expected { 1.0 } else { 0.0 }));
        assert_eq!(m.parameters().len(), 1);
        assert!(
            matches!(m.expression_dimensions(root(&m,"a")),Some(DimensionalStatus::Known(d)) if d.substance()==0&&d.time()==0&&d.length()==0)
        );
    }
    let m = model("a=TrUe;");
    assert_eq!(value(&m, "a"), None);
    assert_eq!(m.parameters().len(), 2);
}

#[test]
fn finite_context_conversion_never_constant_folds_away_boolean_math() {
    for (expr, expected) in [
        ("true+2", 3.0),
        ("false-2", -2.0),
        ("true^false", 1.0),
        ("sin(true)", 1.0_f64.sin()),
        ("!0", 1.0),
        ("!(-2)", 0.0),
        ("not((1/2))", 0.0),
        ("and(2,3)", 1.0),
        ("or(0,-2)", 1.0),
        ("eq(true,1,true)", 1.0),
        ("lt(false,0.5,true)", 1.0),
        ("neq(false,0)", 0.0),
        ("piecewise(true,2,9)", 1.0),
        ("piecewise(7,0,9)", 9.0),
        ("piecewise(7,-2,9)", 7.0),
    ] {
        let m = roundtrip(&format!("a={expr};"));
        assert_eq!(value(&m, "a"), Some(expected), "{expr}");
        assert!(
            !matches!(
                m.expression(root(&m, "a")).unwrap().kind(),
                ExpressionKind::Number { .. }
            ),
            "folded {expr}"
        );
    }
    let m = model("a=true+2;");
    let ExpressionKind::Binary { left, .. } = m.expression(root(&m, "a")).unwrap().kind() else {
        panic!()
    };
    assert!(matches!(
        m.expression(*left).unwrap().kind(),
        ExpressionKind::Boolean(true)
    ));
}

#[test]
fn equal_precedence_logic_and_power_tighter_than_not_are_preserved() {
    for (expr, expected) in [
        ("true||false&&false", 0.0),
        ("true||(false&&false)", 1.0),
        ("!2^0", 0.0),
        ("2^!0", 2.0),
        ("!1<2", 1.0),
        ("-!0", -1.0),
        ("!!2", 1.0),
    ] {
        assert_eq!(
            value(&roundtrip(&format!("a={expr};")), "a"),
            Some(expected),
            "{expr}"
        );
    }
    let m = model("a=true||false&&false;");
    let args = logical(&m, root(&m, "a"), LogicalOp::And, 2);
    logical(&m, args[0], LogicalOp::Or, 2);
    let m = model("a=!!true;");
    let ExpressionKind::Not { child } = m.expression(root(&m, "a")).unwrap().kind() else {
        panic!()
    };
    assert!(matches!(
        m.expression(*child).unwrap().kind(),
        ExpressionKind::Not { .. }
    ));
}

#[test]
fn outer_operator_append_differs_from_functional_and_right_nested_construction() {
    for expr in [
        "and(true,false)&&true",
        "(true&&false)&&true",
        "+(true&&false)&&true",
        "-(-(true&&false))&&true",
    ] {
        let m = roundtrip(&format!("a={expr};"));
        logical(&m, root(&m, "a"), LogicalOp::And, 3);
    }
    for expr in ["and(and(true,false),true)", "and(true&&false,true)"] {
        let m = model(&format!("a={expr};"));
        let args = logical(&m, root(&m, "a"), LogicalOp::And, 2);
        logical(&m, args[0], LogicalOp::And, 2);
    }
    for expr in ["true&&(false&&true)", "true&&and(false,true)"] {
        let m = model(&format!("a={expr};"));
        let args = logical(&m, root(&m, "a"), LogicalOp::And, 2);
        logical(&m, args[1], LogicalOp::And, 2);
    }
    let m = model("a=and()&&true;");
    logical(&m, root(&m, "a"), LogicalOp::And, 1);
    for (expr, op, expected) in [("and()", LogicalOp::And, 1.0), ("or()", LogicalOp::Or, 0.0)] {
        let m = model(&format!("a={expr};"));
        logical(&m, root(&m, "a"), op, 0);
        assert_eq!(value(&m, "a"), Some(expected));
    }
}

#[test]
fn relational_functions_retain_nary_and_nested_numeric_coercion() {
    for (expr, op, len, expected) in [
        ("lt(1,2,3)", ComparisonOp::Less, 3, 1.0),
        ("leq(1,1,2)", ComparisonOp::LessEqual, 3, 1.0),
        ("gt(3,2,1)", ComparisonOp::Greater, 3, 1.0),
        ("geq(3,3,1)", ComparisonOp::GreaterEqual, 3, 1.0),
        ("equals(true,1,true)", ComparisonOp::Equal, 3, 1.0),
        ("neq(1,2)", ComparisonOp::NotEqual, 2, 1.0),
        ("lt(lt(1,2),3)", ComparisonOp::Less, 2, 1.0),
        ("1<(2<3)", ComparisonOp::Less, 2, 0.0),
    ] {
        let m = roundtrip(&format!("a={expr};"));
        let ExpressionKind::Compare { op: actual, args } =
            m.expression(root(&m, "a")).unwrap().kind()
        else {
            panic!("{expr}")
        };
        assert_eq!(*actual, op);
        assert_eq!(args.len(), len);
        assert_eq!(value(&m, "a"), Some(expected));
    }
}

#[test]
fn deferred_chain_predicate_sees_normalized_relations_and_all_relation_and() {
    for left in [
        "1<2",
        "lt(1,2)",
        "lt(1,2,3)",
        "and(1<2,2<3)",
        "and(lt(1,2),geq(3,2))",
        "and(1<2,2<3,3<4)",
        "+(1<2)",
        "-(-(1<2))",
        "+and(1<2,2<3)",
        "-(-and(1<2,2<3))",
    ] {
        rejected(
            &format!("a=({left})<4;"),
            DiagnosticCode::UnsupportedFeature,
        );
    }
    for left in [
        "true",
        "and()",
        "and(1<2)",
        "and(1<2,true)",
        "or(1<2,2<3)",
        "!(1<2)",
        "!!(1<2)",
        "-(1<2)",
        "(1<2)+0",
        "piecewise(1<2)",
        "sin(1<2)",
    ] {
        model(&format!("a=({left})<4;"));
    }
    model("a=lt(and(1<2,2<3),4);");
}

#[test]
fn piecewise_order_otherwise_and_mixed_value_types_remain_explicit() {
    let m = roundtrip("a=piecewise(1,false,2,true,3);b=piecewise(7);c=piecewise(true,true,9);");
    let ExpressionKind::Piecewise {
        branches,
        otherwise,
    } = m.expression(root(&m, "a")).unwrap().kind()
    else {
        panic!()
    };
    assert_eq!(branches.len(), 2);
    assert!(otherwise.is_some());
    assert!(matches!(
        m.expression(branches[0].condition).unwrap().kind(),
        ExpressionKind::Boolean(false)
    ));
    assert!(matches!(
        m.expression(branches[1].condition).unwrap().kind(),
        ExpressionKind::Boolean(true)
    ));
    assert_eq!(value(&m, "a"), Some(2.0));
    assert_eq!(value(&m, "b"), Some(7.0));
    assert_eq!(value(&m, "c"), Some(1.0));
    let ExpressionKind::Piecewise {
        branches,
        otherwise,
    } = m.expression(root(&m, "b")).unwrap().kind()
    else {
        panic!()
    };
    assert!(branches.is_empty());
    assert!(otherwise.is_some());
    let rate = model("species S;J:=>S;piecewise(1,false);");
    let ExpressionKind::Piecewise {
        branches,
        otherwise,
    } = rate.expression(rate.reactions()[0].rate()).unwrap().kind()
    else {
        panic!()
    };
    assert_eq!(branches.len(), 1);
    assert!(otherwise.is_none());
}

#[test]
fn demanded_piecewise_conditions_and_selected_values_are_lazy() {
    for (expr, expected) in [
        ("piecewise(7,true,sqrt(-1))", 7.0),
        ("piecewise(sqrt(-1),false,7)", 7.0),
        ("piecewise(7,true,8,sqrt(-1),9)", 7.0),
        ("piecewise(7,true,1/0)", 7.0),
        ("piecewise(u,false,7)", 7.0),
    ] {
        assert_eq!(
            value(&model(&format!("a={expr};")), "a"),
            Some(expected),
            "{expr}"
        );
    }
    for expr in [
        "piecewise(1,false)",
        "piecewise(1,false,2,false)",
        "piecewise(sqrt(-1),true,7)",
        "piecewise(1/0,true,7)",
        "piecewise(1,sqrt(-1),2)",
    ] {
        rejected(&format!("a={expr};"), DiagnosticCode::DeclarationConflict);
    }
}

#[test]
fn first_unknown_selection_stays_none_without_evaluating_possible_branches() {
    for expr in [
        "piecewise(sqrt(-1),u,7)",
        "piecewise(7,u,7)",
        "piecewise(7,time>0,9)",
        "piecewise(1,u,2,true,3)",
        "piecewise(7,u,8,sqrt(-1),9)",
    ] {
        let m = model(&format!("a={expr};"));
        assert_eq!(value(&m, "a"), None, "{expr}");
        assert!(
            m.parameters()
                .iter()
                .find(|p| p.id() == "a")
                .unwrap()
                .initial_expression()
                .is_some()
        );
    }
}

#[test]
fn logical_demand_stops_on_decisive_or_first_unknown_operand() {
    for (expr, expected) in [
        ("and(false,sqrt(-1))", 0.0),
        ("or(true,sqrt(-1))", 1.0),
        ("and(false,u)", 0.0),
        ("or(true,u)", 1.0),
        ("false&&sqrt(-1)", 0.0),
        ("true||sqrt(-1)", 1.0),
    ] {
        assert_eq!(
            value(&model(&format!("a={expr};")), "a"),
            Some(expected),
            "{expr}"
        );
    }
    for expr in [
        "and(u,false)",
        "or(u,true)",
        "and(u,sqrt(-1))",
        "or(u,1/0)",
        "u&&false",
        "u||true",
    ] {
        assert_eq!(value(&model(&format!("a={expr};")), "a"), None, "{expr}");
    }
    for expr in [
        "and(sqrt(-1),false)",
        "or(1/0,true)",
        "true&&sqrt(-1)",
        "false||sqrt(-1)",
    ] {
        rejected(&format!("a={expr};"), DiagnosticCode::DeclarationConflict);
    }
}

#[test]
fn comparison_and_numeric_arguments_remain_eager_despite_false_or_unknown() {
    for expr in [
        "lt(u,sqrt(-1))",
        "eq(sqrt(-1),u)",
        "lt(3,2,sqrt(-1))",
        "lt(u,1/0)",
        "true+sqrt(-1)",
        "min(u,sqrt(-1))",
    ] {
        rejected(&format!("a={expr};"), DiagnosticCode::DeclarationConflict);
    }
    assert_eq!(value(&model("a=lt(u,1);"), "a"), None);
}

#[test]
fn static_cycles_and_nonfinite_literal_boundaries_survive_lazy_branches() {
    for source in [
        "a=piecewise(a,false,1);",
        "a=and(false,b);b=a;",
        "a=or(true,a);",
        "a=piecewise(b,true,1);b=a;",
    ] {
        rejected(source, DiagnosticCode::InitializationCycle);
    }
    for expr in [
        "piecewise(1,true,(1/0))",
        "and(false,(0/0))",
        "or(true,(1/0))",
    ] {
        rejected(&format!("a={expr};"), DiagnosticCode::UnsupportedFeature);
    }
}

#[test]
fn all_branch_units_and_boolean_contexts_are_checked_statically() {
    for source in [
        "x=2;x has second;y=3;y has metre;a=piecewise(x,true,y);",
        "x=2;x has second;a=piecewise(7,x,9);",
        "x=2;x has second;a=and(false,x);",
        "x=2;x has second;y=3;y has metre;a=lt(u,x,y);",
        "x=2;x has second;y=3;y has metre;a=piecewise(u,true,x,false,y);",
        "x=2;x has second;a=not(x);",
        "x=2;x has second;a=piecewise(true,true,x);",
    ] {
        rejected(source, DiagnosticCode::UnitsInconsistency);
    }
    let m = model("x=2;x has second;y=3;y has second;a=piecewise(x,true,y);");
    assert_eq!(
        m.expression_dimensions(root(&m, "a")),
        m.symbol_dimensions("x")
    );
    let m = model("a=u<1;");
    assert!(matches!(
        m.expression_dimensions(root(&m, "a")),
        Some(DimensionalStatus::Known(_))
    ));
    let m = model("unit time_unit=dimensionless;a=piecewise(1,time,2);");
    assert_eq!(value(&m, "a"), None);
}

#[test]
fn inactive_references_still_infer_symbols_modifiers_and_mutability() {
    let m = roundtrip("species A=1,B=0,E;A=>B;piecewise(1,true,E);");
    assert_eq!(m.reactions()[0].modifiers(), ["E"]);
    let m = model("A=>B;piecewise(1,true,_J0);");
    assert_eq!(m.reactions()[0].id(), "_J1");
    assert!(
        m.parameters()
            .iter()
            .any(|p| p.id() == "_J0" && p.value().is_none())
    );
    let m = model("var formula u;a=piecewise(u,false,7);");
    let a = m.parameters().iter().find(|p| p.id() == "a").unwrap();
    assert_eq!(a.value(), Some(7.0));
    assert!(!a.constant());
}

#[test]
fn operator_trivia_coalescing_is_selective_and_actual_newlines_reject() {
    for op in ["==", "&&", "||"] {
        for sep in [" ", "\t", "/*c*/", "/*c\nβ*/"] {
            let bytes = op.as_bytes();
            roundtrip(&format!(
                "a=1{}{sep}{}2;",
                bytes[0] as char, bytes[1] as char
            ));
        }
    }
    for op in ["<=", ">=", "!="] {
        for sep in [" ", "\t", "/*c*/", "/*c\nβ*/"] {
            let b = op.as_bytes();
            rejected(
                &format!("a=1{}{sep}{}2;", b[0] as char, b[1] as char),
                DiagnosticCode::Syntax,
            );
        }
    }
    for op in ["<=", ">=", "!=", "==", "&&", "||"] {
        let b = op.as_bytes();
        for sep in ["\n", " //c\n"] {
            rejected(
                &format!("a=1{}{sep}{}2;", b[0] as char, b[1] as char),
                DiagnosticCode::Syntax,
            );
        }
    }
    for expr in ["! !0", "!/*x\ny*/!0", "not/*x\ny*/(false)", "not (true)"] {
        roundtrip(&format!("a={expr};"));
    }
}

#[test]
fn unsupported_aliases_chains_and_unrelated_features_do_not_silently_accept() {
    for expr in [
        "true and false",
        "true or false",
        "not false",
        "xor(true,false)",
        "implies(true,false)",
        "1<2<3",
    ] {
        rejected(&format!("a={expr};"), DiagnosticCode::UnsupportedFeature);
    }
    for expr in [
        "not(1/2)",
        "not (1/2)",
        "eq()",
        "lt(1)",
        "neq(1,2,3)",
        "not()",
        "not(1,2)",
        "piecewise()",
        "and(true,)",
        "piecewise(1;true;2)",
    ] {
        rejected(&format!("a={expr};"), DiagnosticCode::Syntax);
    }
    for name in [
        "true",
        "False",
        "TRUE",
        "and",
        "or",
        "not",
        "lt",
        "equals",
        "piecewise",
    ] {
        assert!(
            compile(&format!("formula {name}=2;"), &Limits::default()).is_err(),
            "{name}"
        );
    }
}

#[test]
fn wide_piecewise_preserves_owned_node_count_and_missing_else_distinction() {
    let count = 300;
    let source = format!(
        "a=piecewise({},7);",
        std::iter::repeat_n("1,false", count)
            .collect::<Vec<_>>()
            .join(",")
    );
    let m = model(&source);
    assert_eq!(value(&m, "a"), Some(7.0));
    let mut pending = vec![root(&m, "a")];
    let mut seen = HashSet::new();
    let mut pairs = 0;
    while let Some(id) = pending.pop() {
        assert!(seen.insert(id), "duplicated/shared/cyclic id");
        match m.expression(id).unwrap().kind() {
            ExpressionKind::Piecewise {
                branches,
                otherwise,
            } => {
                pairs += branches.len();
                for b in branches {
                    pending.push(b.value);
                    pending.push(b.condition);
                }
                pending.extend(*otherwise);
            }
            ExpressionKind::Boolean(_) | ExpressionKind::Number { .. } => (),
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(pairs, count);
    assert_eq!(seen.len(), 2 * count + 2);
    let low = Limits::default().with_limit(LimitKind::Tokens, 100);
    assert_eq!(
        compile(&source, &low).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}

#[test]
fn inactive_branch_inference_and_normalized_depth_remain_resource_bounded() {
    let source = "a=piecewise(1,true,u0+u1+u2+u3+u4+u5+u6+u7+u8+u9);";
    let low = Limits::default().with_limit(LimitKind::ModelEntities, 5);
    assert_eq!(
        compile(source, &low).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
    let source = format!(
        "a={};",
        std::iter::repeat_n("piecewise(1,true,2)", 300)
            .collect::<Vec<_>>()
            .join("+")
    );
    let high = Limits::default().with_limit(LimitKind::ExpressionDepth, 10_000);
    assert_eq!(
        compile(&source, &high).unwrap_err().code(),
        DiagnosticCode::ResourceLimit
    );
}

#[cfg(feature = "sbml")]
#[test]
fn conditional_mathml_keeps_booleans_order_and_omitted_otherwise() {
    let m = model("a=true+2;b=piecewise(true,false,2,true,3);species S;J:=>S;piecewise(1,false);");
    let xml = antimony::write_sbml(&m).unwrap();
    assert!(xml.xml().contains("<true/>"));
    assert!(xml.xml().contains("<false/>"));
    assert!(xml.xml().contains("<piecewise><piece>"));
    assert!(
        xml.xml()
            .contains("<otherwise><cn type=\"integer\">3</cn></otherwise>")
    );
    assert_eq!(xml.xml().matches("<otherwise>").count(), 1);
    assert_eq!(xml.xml().matches("<piece>").count(), 3);
}

#[test]
fn raised_limit_conditional_depth_is_stack_safe_in_an_isolated_process() {
    const CHILD: &str = "ANTIMONY_INDEPENDENT_CONDITIONAL_DEPTH_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let n = 3000;
        let sources = [
            format!("a={}false;", "!".repeat(n)),
            format!("a={}false{};", "not(".repeat(n), ")".repeat(n)),
            format!("a={}7{};", "piecewise(1,false,".repeat(n), ")".repeat(n)),
            format!("a={}false{};", "true&&(".repeat(n), ")".repeat(n)),
            format!("a={}2{};", "lt(1,".repeat(n), ")".repeat(n)),
        ];
        let limits = Limits::default().with_limit(LimitKind::ExpressionDepth, 10_000);
        for text in sources {
            assert_eq!(
                compile(&text, &limits).unwrap_err().code(),
                DiagnosticCode::ResourceLimit
            );
        }
        let text = format!("a={}7{};", "piecewise(1,false,".repeat(96), ")".repeat(96));
        assert_eq!(value(&compile(&text, &limits).unwrap(), "a"), Some(7.0));
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "raised_limit_conditional_depth_is_stack_safe_in_an_isolated_process",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn boolean_call_aliases_distinguish_valid_capitals_from_lowercase_and_null_math() {
    for (name, expected) in [
        ("True", true),
        ("TRUE", true),
        ("False", false),
        ("FALSE", false),
    ] {
        for gap in ["", " ", "/* β\ncomment */"] {
            let m = roundtrip(&format!("a={name}{gap}();"));
            assert!(
                matches!(m.expression(root(&m,"a")).unwrap().kind(),ExpressionKind::Boolean(v) if *v==expected)
            );
            assert_eq!(value(&m, "a"), Some(if expected { 1.0 } else { 0.0 }));
        }
        // Upstream loads these but omits math; this is not conformance credit.
        for args in ["1", "1,2"] {
            rejected(&format!("a={name}({args});"), DiagnosticCode::Syntax);
        }
    }
    for name in ["true", "false"] {
        for args in ["", "1", "1,2"] {
            rejected(&format!("a={name}({args});"), DiagnosticCode::Syntax);
        }
    }
    let m = model("formula TrUe=2;a=TrUe;");
    assert_eq!(value(&m, "a"), Some(2.0));
    for name in ["TrUe", "FaLsE"] {
        assert!(compile(&format!("a={name}();"), &Limits::default()).is_err());
    }
}
