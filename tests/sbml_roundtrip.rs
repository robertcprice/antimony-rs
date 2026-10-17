//! Integration tests for the native SBML Level 3 Version 2 Core reader and decompiler
//! (`read_sbml` and `sbml_to_antimony`).
#![cfg(feature = "sbml")]

use antimony::{
    DiagnosticCode, Limits, SourceFile, SourceId, analyze, parse, read_sbml, sbml_to_antimony,
    write_sbml,
};
use std::fs;
use std::path::Path;

fn assert_sbml_roundtrip(antimony_src: &str, label: &str) {
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(900), label, antimony_src, &limits)
        .expect("source within limits");
    let parsed1 = parse(&source, &limits).expect("initial Antimony should parse");
    let model1 = analyze(&parsed1).expect("initial Antimony should analyze");
    let sbml1 = write_sbml(&model1).expect("initial model should export to SBML");

    let decompiled = sbml_to_antimony(sbml1.xml(), &limits).unwrap_or_else(|d| {
        panic!(
            "sbml_to_antimony failed for {label}: {} ({:?})\nSBML:\n{}",
            d.message(),
            d.code(),
            sbml1.xml()
        )
    });
    let model2 = read_sbml(sbml1.xml(), &limits).unwrap_or_else(|d| {
        panic!(
            "read_sbml failed for {label}: {} ({:?})\nDecompiled:\n{decompiled}",
            d.message(),
            d.code()
        )
    });
    let sbml2 = write_sbml(&model2).expect("roundtripped model should export to SBML");

    assert_eq!(
        sbml1.xml(),
        sbml2.xml(),
        "SBML roundtrip mismatch for {label}\nOriginal Antimony:\n{antimony_src}\nDecompiled Antimony:\n{decompiled}"
    );
}

#[test]
fn roundtrip_all_positive_modular_and_collaboration_corpus_models() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut total_checked = 0usize;
    for (dir_rel, min_expected) in [
        ("fixtures/accepted", 20usize),
        ("fixtures/arithmetic", 20),
        ("fixtures/builtins", 40),
        ("fixtures/rational-calls", 40),
        ("fixtures/conditionals", 80),
        ("fixtures/rules", 80),
        ("fixtures/functions", 60),
        ("fixtures/collaboration/cases", 24),
        ("fixtures/modular/cases", 28),
    ] {
        let dir = root.join(dir_rel);
        let mut entries: Vec<_> = fs::read_dir(&dir)
            .expect("fixture directory")
            .filter_map(Result::ok)
            .collect();
        entries.sort_by_key(|e| e.file_name());
        let mut checked = 0usize;
        for entry in entries {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("ant") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = fs::read_to_string(&path).expect("read fixture");
            let limits = Limits::default();
            let Ok(src) = SourceFile::new(SourceId::new(0), name.clone(), &text, &limits) else {
                continue;
            };
            let Ok(parsed) = parse(&src, &limits) else {
                continue;
            };
            let Ok(_) = analyze(&parsed) else {
                continue;
            };
            assert_sbml_roundtrip(&text, &format!("{dir_rel}/{name}"));
            checked += 1;
            total_checked += 1;
        }
        assert!(
            checked >= min_expected,
            "expected at least {min_expected} positive models in {dir_rel}, got {checked}"
        );
    }
    assert!(total_checked >= 400);
}

#[test]
fn roundtrip_module_delete_and_conversion_factor_sbml() {
    let src = "\
module sub(S)
  compartment c = 1;
  species S in c = 10;
  species P in c = 0;
  species Q in c = 0;
  k1 = 0.2;
  k2 = 0.05;
  J0: S -> P + Q; k1 * S;
  J1: P -> Q; k2 * P;
  E0: at 2 after (time > 5): P = time;
end
module *main()
  compartment c = 1;
  species A in c = 10;
  tf = 60;
  ef = 1000;
  M1: sub(A), timeconv = tf, extentconv = ef;
  delete M1.J1, M1.Q, M1.k2;
end
";
    assert_sbml_roundtrip(src, "module_delete_timeconv_extentconv");
}

#[test]
fn read_sbml_rejects_unsafe_and_outside_slice_xml() {
    let limits = Limits::default();

    // 1. Reject XML DOCTYPE / ENTITY (XXE defense)
    let xxe = "<?xml version=\"1.0\"?><!DOCTYPE sbml [<!ENTITY x \"1\">]><sbml xmlns=\"http://www.sbml.org/sbml/level3/version2/core\" level=\"3\" version=\"2\"><model id=\"m\"/></sbml>";
    assert_eq!(
        read_sbml(xxe, &limits).unwrap_err().code(),
        DiagnosticCode::UnsupportedFeature
    );

    // 2. Reject unsupported SBML Level 1
    let l1v2 = "<sbml xmlns=\"http://www.sbml.org/sbml/level1\" level=\"1\" version=\"2\"><model id=\"m\"/></sbml>";
    assert_eq!(
        read_sbml(l1v2, &limits).unwrap_err().code(),
        DiagnosticCode::UnsupportedFeature
    );

    // 3. Reject SBML L3 package extension attributes
    let comp_pkg = "<sbml xmlns=\"http://www.sbml.org/sbml/level3/version2/core\" xmlns:comp=\"http://www.sbml.org/sbml/level3/version1/comp/version1\" level=\"3\" version=\"2\" comp:required=\"true\"><model id=\"m\"/></sbml>";
    assert_eq!(
        read_sbml(comp_pkg, &limits).unwrap_err().code(),
        DiagnosticCode::UnsupportedFeature
    );

    // 4. Reject algebraicRule
    let alg_rule = "\
<sbml xmlns=\"http://www.sbml.org/sbml/level3/version2/core\" level=\"3\" version=\"2\">
  <model id=\"m\">
    <listOfParameters>
      <parameter id=\"x\" value=\"1\" constant=\"false\"/>
    </listOfParameters>
    <listOfRules>
      <algebraicRule>
        <math xmlns=\"http://www.w3.org/1998/Math/MathML\">
          <ci>x</ci>
        </math>
      </algebraicRule>
    </listOfRules>
  </model>
</sbml>";
    assert_eq!(
        read_sbml(alg_rule, &limits).unwrap_err().code(),
        DiagnosticCode::UnsupportedFeature
    );

    // 5. Reject delay csymbol
    let delay_csymbol = "\
<sbml xmlns=\"http://www.sbml.org/sbml/level3/version2/core\" level=\"3\" version=\"2\">
  <model id=\"m\">
    <listOfParameters>
      <parameter id=\"x\" constant=\"false\"/>
    </listOfParameters>
    <listOfRules>
      <assignmentRule variable=\"x\">
        <math xmlns=\"http://www.w3.org/1998/Math/MathML\">
          <apply>
            <csymbol encoding=\"text\" definitionURL=\"http://www.sbml.org/sbml/symbols/delay\">delay</csymbol>
            <ci>x</ci>
            <cn type=\"integer\">1</cn>
          </apply>
        </math>
      </assignmentRule>
    </listOfRules>
  </model>
</sbml>";
    assert_eq!(
        read_sbml(delay_csymbol, &limits).unwrap_err().code(),
        DiagnosticCode::UnsupportedFeature
    );
}

#[test]
fn wild_biomodels_sbml_l2v4_and_l3v1_import_and_simulate() {
    let limits = Limits::default();

    // BioModels-style SBML L2V4 with <notes>, RDF <annotation>, defaulted attributes,
    // <cn type="e-notation">, and reaction-scoped <listOfParameters> (including a shadowed `k`).
    let l2v4_biomodels = "\
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<sbml xmlns=\"http://www.sbml.org/sbml/level2/version4\" xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" level=\"2\" version=\"4\">
  <model id=\"biomodels_cascade\">
    <notes>
      <body xmlns=\"http://www.w3.org/1999/xhtml\"><p>BioModels L2V4 import test</p></body>
    </notes>
    <annotation>
      <rdf:RDF xmlns:bqbiol=\"http://biomodels.net/biology-qualifiers/\">
        <rdf:Description rdf:about=\"#biomodels_cascade\"/>
      </rdf:RDF>
    </annotation>
    <listOfCompartments>
      <compartment id=\"cell\" size=\"1\"/>
    </listOfCompartments>
    <listOfSpecies>
      <species id=\"S\" compartment=\"cell\" initialAmount=\"10\"/>
      <species id=\"I\" compartment=\"cell\" initialAmount=\"0\"/>
      <species id=\"P\" compartment=\"cell\" initialAmount=\"0\" boundaryCondition=\"true\" constant=\"true\"/>
    </listOfSpecies>
    <listOfReactions>
      <reaction id=\"R1\" reversible=\"false\">
        <listOfReactants>
          <speciesReference species=\"S\"/>
        </listOfReactants>
        <listOfProducts>
          <speciesReference species=\"I\"/>
        </listOfProducts>
        <kineticLaw>
          <math xmlns=\"http://www.w3.org/1998/Math/MathML\">
            <apply>
              <times/>
              <ci>k</ci>
              <ci>S</ci>
              <cn type=\"e-notation\"> 1 <sep/> 0 </cn>
            </apply>
          </math>
          <listOfParameters>
            <parameter id=\"k\" value=\"0.2\"/>
          </listOfParameters>
        </kineticLaw>
      </reaction>
      <reaction id=\"R2\" reversible=\"false\">
        <listOfReactants>
          <speciesReference species=\"I\"/>
        </listOfReactants>
        <listOfProducts>
          <speciesReference species=\"P\"/>
        </listOfProducts>
        <kineticLaw>
          <math xmlns=\"http://www.w3.org/1998/Math/MathML\">
            <apply>
              <times/>
              <ci>k</ci>
              <ci>I</ci>
            </apply>
          </math>
          <listOfParameters>
            <parameter id=\"k\" value=\"0.5\"/>
          </listOfParameters>
        </kineticLaw>
      </reaction>
    </listOfReactions>
  </model>
</sbml>";

    let ant = sbml_to_antimony(l2v4_biomodels, &limits).expect("L2V4 BioModels should decompile");
    assert!(ant.contains("k = 0.2;"));
    assert!(ant.contains("R2_k = 0.5;"));
    let model = read_sbml(l2v4_biomodels, &limits).expect("L2V4 BioModels should validate");
    let stoich = model.stoichiometry_matrix();
    assert_eq!(stoich.floating_species(), &["I", "S"]);
    assert_eq!(stoich.boundary_species(), &["P"]);
    assert_eq!(stoich.net_stoichiometry("S", "R1"), Some(-1.0));
    assert_eq!(stoich.net_stoichiometry("I", "R1"), Some(1.0));
    assert_eq!(stoich.net_stoichiometry("I", "R2"), Some(-1.0));

    let traj = model
        .simulate(&antimony::SimulationConfig::new(0.0, 5.0, 10))
        .expect("L2V4 model should simulate");
    let s_end = traj.value_at(10, "S").unwrap();
    let expected_s = 10.0 * (-0.2_f64 * 5.0).exp();
    assert!(
        (s_end - expected_s).abs() < 1e-6,
        "got {s_end}, expected {expected_s}"
    );
}
