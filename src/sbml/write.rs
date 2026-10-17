//! Structured SBML L3V2 Core XML serializer for [`ValidatedModel`].
use super::units;
use crate::model::{ExpressionId, ExpressionKind, RuleKind, SpeciesReference};
use crate::syntax::{BinaryOp, UnaryOp, error};
use crate::{Diagnostic, DiagnosticCode, ValidatedModel};
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};

/// SBML output with its implemented profile and non-fatal validation notices.
#[derive(Clone, Debug)]
pub struct SbmlDocument {
    xml: String,
    profile_id: &'static str,
    warnings: Vec<Diagnostic>,
}
impl SbmlDocument {
    /// Serialized UTF-8 XML. Independent SBML validation remains a separate gate.
    pub fn xml(&self) -> &str {
        &self.xml
    }
    /// Subset for which the input model was checked.
    pub fn profile_id(&self) -> &'static str {
        self.profile_id
    }
    /// Preserved model notices; no warnings are discarded by serialization.
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }
}

/// Serialize a validated units-slice model as SBML Level 3 Version 2 Core.
///
/// No filesystem/network access, implicit numerical simulation, or mathematical
/// simplification occurs. Explicit unit metadata is preserved without value conversion.
pub fn write_sbml(model: &ValidatedModel) -> Result<SbmlDocument, Diagnostic> {
    let mut xml = Xml(Writer::new(Vec::new()));
    emit(model, &mut xml).map_err(|e| {
        error(
            DiagnosticCode::Export,
            model.source().span(0, 0).expect("empty span"),
            e.to_string(),
        )
    })?;
    let xml =
        String::from_utf8(xml.0.into_inner()).expect("writer emits only UTF-8 source and literals");
    Ok(SbmlDocument {
        xml,
        profile_id: model.profile_id(),
        warnings: model.warnings().to_vec(),
    })
}
pub(super) struct Xml(Writer<Vec<u8>>);
impl Xml {
    pub(super) fn start(
        &mut self,
        name: &str,
        attributes: &[(&str, &str)],
    ) -> quick_xml::Result<()> {
        let mut tag = BytesStart::new(name);
        tag.extend_attributes(attributes.iter().copied());
        self.0.write_event(Event::Start(tag))?;
        Ok(())
    }
    pub(super) fn empty(
        &mut self,
        name: &str,
        attributes: &[(&str, &str)],
    ) -> quick_xml::Result<()> {
        let mut tag = BytesStart::new(name);
        tag.extend_attributes(attributes.iter().copied());
        self.0.write_event(Event::Empty(tag))?;
        Ok(())
    }
    pub(super) fn end(&mut self, name: &str) -> quick_xml::Result<()> {
        self.0.write_event(Event::End(BytesEnd::new(name)))?;
        Ok(())
    }
    pub(super) fn text(&mut self, text: &str) -> quick_xml::Result<()> {
        self.0.write_event(Event::Text(BytesText::new(text)))?;
        Ok(())
    }
}
fn emit(model: &ValidatedModel, x: &mut Xml) -> quick_xml::Result<()> {
    x.0.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    x.start(
        "sbml",
        &[
            ("xmlns", "http://www.sbml.org/sbml/level3/version2/core"),
            ("level", "3"),
            ("version", "2"),
        ],
    )?;
    let mut model_attributes = vec![("id", model.id())];
    for default in [
        crate::ModelUnit::Substance,
        crate::ModelUnit::Volume,
        crate::ModelUnit::Area,
        crate::ModelUnit::Length,
        crate::ModelUnit::Extent,
        crate::ModelUnit::Time,
    ] {
        if let Some(unit) = model.model_units(default) {
            model_attributes.push((default.sbml_attribute(), unit));
        }
    }
    x.start("model", &model_attributes)?;
    if !model.function_definitions().is_empty() {
        x.start("listOfFunctionDefinitions", &[])?;
        for function in model.function_definitions() {
            x.start("functionDefinition", &[("id", function.id())])?;
            x.start("math", &[("xmlns", "http://www.w3.org/1998/Math/MathML")])?;
            x.start("lambda", &[])?;
            for parameter in function.parameters() {
                x.start("bvar", &[])?;
                x.start("ci", &[])?;
                x.text(parameter)?;
                x.end("ci")?;
                x.end("bvar")?;
            }
            math(model, x, function.body())?;
            x.end("lambda")?;
            x.end("math")?;
            x.end("functionDefinition")?;
        }
        x.end("listOfFunctionDefinitions")?;
    }
    units::definitions(model, x)?;
    if !model.compartments().is_empty() {
        x.start("listOfCompartments", &[])?;
        for compartment in model.compartments() {
            let size = compartment.size().map(|v| v.to_string());
            let mut attributes = vec![
                ("id", compartment.id()),
                ("spatialDimensions", "3"),
                (
                    "constant",
                    if compartment.constant() {
                        "true"
                    } else {
                        "false"
                    },
                ),
            ];
            if let Some(size) = &size
                && compartment.initial_expression().is_none()
            {
                attributes.insert(1, ("size", size));
            }
            if let Some(unit) = model.units(compartment.id()) {
                attributes.push(("units", unit));
            }
            if compartment.is_implicit() {
                attributes.push(("sboTerm", "SBO:0000410"));
            }
            x.empty("compartment", &attributes)?;
        }
        x.end("listOfCompartments")?;
    }
    if !model.species().is_empty() {
        x.start("listOfSpecies", &[])?;
        for species in model.species() {
            let concentration = species.initial_concentration().map(|v| v.to_string());
            let amount = species.initial_amount().map(|v| v.to_string());
            let mut attributes = vec![
                ("id", species.id()),
                ("compartment", species.compartment()),
                (
                    "hasOnlySubstanceUnits",
                    if species.has_only_substance_units() {
                        "true"
                    } else {
                        "false"
                    },
                ),
                (
                    "boundaryCondition",
                    if species.boundary_condition() {
                        "true"
                    } else {
                        "false"
                    },
                ),
                ("constant", "false"),
            ];
            if let Some(unit) = model.species_substance_units(species.id()) {
                attributes.push(("substanceUnits", unit));
            }
            if species.initial_expression().is_none() {
                if let Some(amount) = &amount {
                    attributes.push(("initialAmount", amount));
                } else if let Some(concentration) = &concentration {
                    attributes.push(("initialConcentration", concentration));
                }
            }
            x.empty("species", &attributes)?;
        }
        x.end("listOfSpecies")?;
    }
    if !model.parameters().is_empty() {
        x.start("listOfParameters", &[])?;
        for parameter in model.parameters() {
            let value = parameter.value().map(|v| v.to_string());
            let mut attributes = vec![
                ("id", parameter.id()),
                (
                    "constant",
                    if parameter.constant() {
                        "true"
                    } else {
                        "false"
                    },
                ),
            ];
            if let Some(unit) = model.units(parameter.id()) {
                attributes.push(("units", unit));
            }
            if let Some(value) = &value
                && parameter.initial_expression().is_none()
            {
                attributes.push(("value", value));
            }
            x.empty("parameter", &attributes)?;
        }
        x.end("listOfParameters")?;
    }
    if !model.initial_assignments().is_empty() {
        x.start("listOfInitialAssignments", &[])?;
        for initial in model.initial_assignments() {
            x.start("initialAssignment", &[("symbol", initial.symbol())])?;
            x.start("math", &[("xmlns", "http://www.w3.org/1998/Math/MathML")])?;
            math(model, x, initial.expression())?;
            x.end("math")?;
            x.end("initialAssignment")?;
        }
        x.end("listOfInitialAssignments")?;
    }
    if !model.rules().is_empty() {
        x.start("listOfRules", &[])?;
        for rule in model.rules() {
            let tag = match rule.kind() {
                RuleKind::Assignment => "assignmentRule",
                RuleKind::Rate => "rateRule",
            };
            x.start(tag, &[("variable", rule.target())])?;
            x.start("math", &[("xmlns", "http://www.w3.org/1998/Math/MathML")])?;
            math(model, x, rule.expression())?;
            x.end("math")?;
            x.end(tag)?;
        }
        x.end("listOfRules")?;
    }
    if !model.reactions().is_empty() {
        x.start("listOfReactions", &[])?;
        for reaction in model.reactions() {
            x.start(
                "reaction",
                &[
                    ("id", reaction.id()),
                    (
                        "reversible",
                        if reaction.reversible() {
                            "true"
                        } else {
                            "false"
                        },
                    ),
                ],
            )?;
            refs(x, "listOfReactants", reaction.reactants())?;
            refs(x, "listOfProducts", reaction.products())?;
            if !reaction.modifiers().is_empty() {
                x.start("listOfModifiers", &[])?;
                for species in reaction.modifiers() {
                    x.empty("modifierSpeciesReference", &[("species", species)])?;
                }
                x.end("listOfModifiers")?;
            }
            x.start("kineticLaw", &[])?;
            x.start("math", &[("xmlns", "http://www.w3.org/1998/Math/MathML")])?;
            math(model, x, reaction.rate())?;
            x.end("math")?;
            x.end("kineticLaw")?;
            x.end("reaction")?;
        }
        x.end("listOfReactions")?;
    }
    if !model.events().is_empty() {
        x.start("listOfEvents", &[])?;
        for event in model.events() {
            x.start(
                "event",
                &[
                    ("id", event.id()),
                    (
                        "useValuesFromTriggerTime",
                        if event.use_values_from_trigger_time() {
                            "true"
                        } else {
                            "false"
                        },
                    ),
                ],
            )?;
            x.start(
                "trigger",
                &[
                    (
                        "initialValue",
                        if event.trigger().initial_value() {
                            "true"
                        } else {
                            "false"
                        },
                    ),
                    (
                        "persistent",
                        if event.trigger().persistent() {
                            "true"
                        } else {
                            "false"
                        },
                    ),
                ],
            )?;
            x.start("math", &[("xmlns", "http://www.w3.org/1998/Math/MathML")])?;
            math(model, x, event.trigger().expression())?;
            x.end("math")?;
            x.end("trigger")?;
            if let Some(delay) = event.delay() {
                x.start("delay", &[])?;
                x.start("math", &[("xmlns", "http://www.w3.org/1998/Math/MathML")])?;
                math(model, x, delay)?;
                x.end("math")?;
                x.end("delay")?;
            }
            if let Some(priority) = event.priority() {
                x.start("priority", &[])?;
                x.start("math", &[("xmlns", "http://www.w3.org/1998/Math/MathML")])?;
                math(model, x, priority)?;
                x.end("math")?;
                x.end("priority")?;
            }
            x.start("listOfEventAssignments", &[])?;
            for assignment in event.assignments() {
                x.start("eventAssignment", &[("variable", assignment.variable())])?;
                x.start("math", &[("xmlns", "http://www.w3.org/1998/Math/MathML")])?;
                math(model, x, assignment.expression())?;
                x.end("math")?;
                x.end("eventAssignment")?;
            }
            x.end("listOfEventAssignments")?;
            x.end("event")?;
        }
        x.end("listOfEvents")?;
    }
    x.end("model")?;
    x.end("sbml")
}
fn refs(x: &mut Xml, list: &str, refs: &[SpeciesReference]) -> quick_xml::Result<()> {
    if refs.is_empty() {
        return Ok(());
    }
    x.start(list, &[])?;
    for reference in refs {
        x.empty(
            "speciesReference",
            &[
                ("species", reference.species()),
                ("stoichiometry", &reference.stoichiometry().to_string()),
                ("constant", "true"),
            ],
        )?;
    }
    x.end(list)
}
fn math(model: &ValidatedModel, x: &mut Xml, root: ExpressionId) -> quick_xml::Result<()> {
    // Explicit traversal avoids call-stack growth while preserving binary grouping.
    enum Task {
        Node(ExpressionId),
        EndApply,
        EndLogBase,
        Start(&'static str),
        End(&'static str),
    }
    let mut pending = vec![Task::Node(root)];
    while let Some(task) = pending.pop() {
        let id = match task {
            Task::Node(id) => id,
            Task::Start(tag) => {
                x.start(tag, &[])?;
                continue;
            }
            Task::End(tag) => {
                x.end(tag)?;
                continue;
            }
            Task::EndApply => {
                x.end("apply")?;
                continue;
            }
            Task::EndLogBase => {
                x.end("logbase")?;
                continue;
            }
        };
        match model
            .expression(id)
            .expect("validated expression ID")
            .kind()
        {
            ExpressionKind::Boolean(value) => {
                x.empty(if *value { "true" } else { "false" }, &[])?
            }
            ExpressionKind::Not { child } => {
                x.start("apply", &[])?;
                x.empty("not", &[])?;
                pending.push(Task::EndApply);
                pending.push(Task::Node(*child));
            }
            ExpressionKind::Compare { op, args } => {
                x.start("apply", &[])?;
                x.empty(op.mathml(), &[])?;
                pending.push(Task::EndApply);
                pending.extend(args.iter().rev().map(|id| Task::Node(*id)));
            }
            ExpressionKind::Logical { op, args } => {
                x.start("apply", &[])?;
                x.empty(op.mathml(), &[])?;
                pending.push(Task::EndApply);
                pending.extend(args.iter().rev().map(|id| Task::Node(*id)));
            }
            ExpressionKind::Piecewise {
                branches,
                otherwise,
            } => {
                x.start("piecewise", &[])?;
                pending.push(Task::End("piecewise"));
                if let Some(id) = otherwise {
                    pending.push(Task::End("otherwise"));
                    pending.push(Task::Node(*id));
                    pending.push(Task::Start("otherwise"));
                }
                for branch in branches.iter().rev() {
                    pending.push(Task::End("piece"));
                    pending.push(Task::Node(branch.condition));
                    pending.push(Task::Node(branch.value));
                    pending.push(Task::Start("piece"));
                }
            }
            ExpressionKind::Time => {
                x.start(
                    "csymbol",
                    &[
                        ("encoding", "text"),
                        ("definitionURL", "http://www.sbml.org/sbml/symbols/time"),
                    ],
                )?;
                x.text("time")?;
                x.end("csymbol")?;
            }
            ExpressionKind::Constant(constant) => x.empty(constant.mathml_element(), &[])?,
            ExpressionKind::Call { function, args } => {
                x.start("apply", &[])?;
                x.empty(function.mathml_operator(), &[])?;
                pending.push(Task::EndApply);
                match function {
                    crate::Builtin::Log => {
                        x.start("logbase", &[])?;
                        pending.push(Task::Node(args[1]));
                        pending.push(Task::EndLogBase);
                        pending.push(Task::Node(args[0]));
                    }
                    crate::Builtin::Sqrt | crate::Builtin::Log10 => {
                        let (tag, n) = if *function == crate::Builtin::Sqrt {
                            ("degree", "2")
                        } else {
                            ("logbase", "10")
                        };
                        x.start(tag, &[])?;
                        number(x, n, false)?;
                        x.end(tag)?;
                        pending.push(Task::Node(args[0]));
                    }
                    _ => pending.extend(args.iter().rev().map(|id| Task::Node(*id))),
                }
            }
            ExpressionKind::UserCall { name, args } => {
                x.start("apply", &[])?;
                x.start("ci", &[])?;
                x.text(name)?;
                x.end("ci")?;
                pending.push(Task::EndApply);
                pending.extend(args.iter().rev().map(|id| Task::Node(*id)));
            }
            ExpressionKind::Rational {
                numerator,
                denominator,
            } => {
                x.start("cn", &[("type", "rational")])?;
                x.text(&numerator.to_string())?;
                x.empty("sep", &[])?;
                x.text(&denominator.to_string())?;
                x.end("cn")?;
            }
            ExpressionKind::Number { lexeme, .. } => number(x, lexeme, false)?,
            ExpressionKind::Symbol { name, .. } => {
                x.start("ci", &[])?;
                x.text(name)?;
                x.end("ci")?;
            }
            ExpressionKind::Unary {
                op: UnaryOp::Plus,
                child,
            } => pending.push(Task::Node(*child)),
            ExpressionKind::Unary {
                op: UnaryOp::Minus,
                child,
            } => {
                // Reference-backed literal representation only, not algebraic simplification.
                if let ExpressionKind::Number { lexeme, .. } =
                    model.expression(*child).expect("validated child").kind()
                {
                    number(x, lexeme, true)?;
                    continue;
                }
                x.start("apply", &[])?;
                x.empty("minus", &[])?;
                pending.push(Task::EndApply);
                pending.push(Task::Node(*child));
            }
            ExpressionKind::Binary { op, left, right } => {
                x.start("apply", &[])?;
                x.empty(
                    match op {
                        BinaryOp::Add => "plus",
                        BinaryOp::Subtract => "minus",
                        BinaryOp::Multiply => "times",
                        BinaryOp::Divide => "divide",
                        BinaryOp::Power => "power",
                    },
                    &[],
                )?;
                pending.push(Task::EndApply);
                pending.push(Task::Node(*right));
                pending.push(Task::Node(*left));
            }
        }
    }
    Ok(())
}

// Match the pinned reference numeric AST categories without re-rounding a source
// literal to match a comparator. Decimal mantissa/exponent come from source digits.
fn number(x: &mut Xml, lexeme: &str, negative: bool) -> quick_xml::Result<()> {
    let value: f64 = lexeme.parse().expect("validated numeric literal");
    if let Some((mut mantissa, exponent)) = crate::syntax::numeric::scientific_parts(lexeme, value)
    {
        if negative {
            mantissa.insert(0, '-');
        }
        x.start("cn", &[("type", "e-notation")])?;
        x.text(&mantissa)?;
        x.empty("sep", &[])?;
        x.text(&exponent.to_string())?;
    } else {
        let kind = if value.fract() == 0.0 && value <= f64::from(i32::MAX) {
            "integer"
        } else {
            "real"
        };
        // The source number has no sign: -2147483648 is negation of a real
        // 2147483648 node upstream, not an i32-min integer literal.
        let signed = if negative && value != 0.0 {
            -value
        } else {
            value
        };
        x.start("cn", &[("type", kind)])?;
        x.text(&signed.to_string())?;
    }
    x.end("cn")
}
