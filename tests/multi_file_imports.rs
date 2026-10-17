//! Integration tests for multi-file `import "other.ant"` resolution (`parse_with_imports` and `load_file`).

use antimony::{
    DiagnosticCode, LimitKind, Limits, SimulationConfig, SourceFile, SourceId, analyze, load_file,
    parse_with_imports,
};
use std::collections::BTreeMap;
use std::fs;

#[test]
fn multi_file_imports_nested_diamond_and_flat_main() {
    let mut vfs = BTreeMap::<String, String>::new();
    vfs.insert(
        "lib/kinetics.ant".to_owned(),
        "function mm(S, Vm, Km)\n  Vm * S / (Km + S);\nend\n".to_owned(),
    );
    vfs.insert(
        "lib/step.ant".to_owned(),
        "import \"kinetics.ant\";\nmodule enzyme_step(S, P)\n  Vm = 2.0;\n  Km = 0.5;\n  J: S -> P; mm(S, Vm, Km);\nend\n".to_owned(),
    );

    let main_src = "\
import \"lib/kinetics.ant\";
import \"lib/step.ant\";

module *cascade()
  species A = 4.0, B = 0.0, C = 0.0;
  S1: enzyme_step(A, B);
  S2: enzyme_step(B, C);
end
";
    let limits = Limits::default();
    let sf = SourceFile::new(SourceId::new(0), "main.ant", main_src, &limits).unwrap();
    let parsed = parse_with_imports(&sf, &limits, |rel_path| {
        vfs.get(rel_path)
            .cloned()
            .ok_or_else(|| format!("missing virtual file {rel_path}"))
    })
    .expect("multi-file diamond import should parse");
    let model = analyze(&parsed).expect("multi-file diamond import should analyze");
    assert_eq!(model.id(), "cascade");
    assert_eq!(model.species().len(), 3);
    assert_eq!(model.reactions().len(), 2);

    // Also test flat top-level main statements after importing a submodel module.
    let flat_main_src = "\
import \"lib/step.ant\";
species X = 5.0, Y = 0.0;
M1: enzyme_step(X, Y);
";
    let sf_flat =
        SourceFile::new(SourceId::new(0), "flat_main.ant", flat_main_src, &limits).unwrap();
    let parsed_flat = parse_with_imports(&sf_flat, &limits, |rel_path| {
        vfs.get(rel_path)
            .cloned()
            .ok_or_else(|| format!("missing virtual file {rel_path}"))
    })
    .expect("flat main with import should parse");
    let model_flat = analyze(&parsed_flat).expect("flat main with import should analyze");
    assert_eq!(model_flat.id(), "__main");
    let traj = model_flat
        .simulate(&SimulationConfig::new(0.0, 5.0, 10))
        .expect("flat main should simulate");
    let x_end = traj.value_at(10, "X").unwrap();
    let y_end = traj.value_at(10, "Y").unwrap();
    assert!((x_end + y_end - 5.0).abs() < 1e-9);
    assert!(y_end > 4.0);
}

#[test]
fn load_file_resolves_disk_imports_and_rejects_cycles_and_traversal() {
    let limits = Limits::default();
    let tmp_dir =
        std::env::temp_dir().join(format!("antimony_rs_import_test_{}", std::process::id()));
    let sub_dir = tmp_dir.join("sub");
    fs::create_dir_all(&sub_dir).unwrap();

    let sub_file = sub_dir.join("unit_step.ant");
    fs::write(
        &sub_file,
        "module lin_step(S, P)\n  k = 0.25;\n  S -> P; k * S;\nend\n",
    )
    .unwrap();

    let main_file = tmp_dir.join("main.ant");
    fs::write(
        &main_file,
        "import \"sub/unit_step.ant\";\nmodel *main()\n  species A = 8, B = 0;\n  M: lin_step(A, B);\nend\n",
    )
    .unwrap();

    let model = load_file(&main_file, &limits).expect("load_file should resolve relative import");
    let traj = model
        .simulate(&SimulationConfig::new(0.0, 4.0, 8))
        .expect("loaded model should simulate");
    let expected_a = 8.0 * (-1.0_f64).exp();
    let actual_a = traj.value_at(8, "A").unwrap();
    assert!((actual_a - expected_a).abs() < 1e-6);

    let _ = fs::remove_dir_all(&tmp_dir);

    // Reject import cycles
    let mut cyc = BTreeMap::new();
    cyc.insert(
        "a.ant".to_owned(),
        "import \"b.ant\";\nmodel a() x = 1; end\n".to_owned(),
    );
    cyc.insert(
        "b.ant".to_owned(),
        "import \"a.ant\";\nmodel b() y = 1; end\n".to_owned(),
    );
    let sf_cyc = SourceFile::new(
        SourceId::new(0),
        "root.ant",
        "import \"a.ant\";\nmodel *m() z = 1; end\n",
        &limits,
    )
    .unwrap();
    let err_cyc = parse_with_imports(&sf_cyc, &limits, |p| {
        cyc.get(p).cloned().ok_or_else(|| "missing".to_owned())
    })
    .unwrap_err();
    assert_eq!(err_cyc.code(), DiagnosticCode::DeclarationConflict);

    // Reject path traversal (`..`) and absolute paths (`/etc/passwd`)
    for bad_import in [
        "import \"../secret.ant\";\nx = 1;\n",
        "import \"/etc/passwd\";\nx = 1;\n",
        "import \"C:\\\\secret.ant\";\nx = 1;\n",
    ] {
        let sf_bad = SourceFile::new(SourceId::new(0), "bad.ant", bad_import, &limits).unwrap();
        let err = parse_with_imports(
            &sf_bad,
            &limits,
            |_| Ok("model s() x = 1; end\n".to_owned()),
        )
        .unwrap_err();
        assert_eq!(err.code(), DiagnosticCode::UnsupportedFeature);
    }

    // Enforce cumulative SourceBytes across imported files
    let tight_limits = Limits::default().with_limit(LimitKind::SourceBytes, 60);
    let sf_tight = SourceFile::new(
        SourceId::new(0),
        "tight.ant",
        "import \"big.ant\";\nmodel *m() x = 1; end\n",
        &tight_limits,
    )
    .unwrap();
    let err_tight = parse_with_imports(&sf_tight, &tight_limits, |_| {
        Ok("model big()\n  species S1 = 1, S2 = 2, S3 = 3;\nend\n".to_owned())
    })
    .unwrap_err();
    assert_eq!(err_tight.code(), DiagnosticCode::ResourceLimit);
}
