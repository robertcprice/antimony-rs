//! Emit exact preserved unit components; never convert numeric entity values.
use super::Xml;
use crate::ValidatedModel;

pub(super) fn definitions(model: &ValidatedModel, x: &mut Xml) -> quick_xml::Result<()> {
    if model.unit_definitions().is_empty() {
        return Ok(());
    }
    x.start("listOfUnitDefinitions", &[])?;
    for definition in model.unit_definitions() {
        x.start("unitDefinition", &[("id", definition.id())])?;
        x.start("listOfUnits", &[])?;
        for component in definition.components() {
            x.empty(
                "unit",
                &[
                    ("kind", component.kind().as_str()),
                    ("exponent", &component.exponent().to_string()),
                    ("scale", &component.scale().to_string()),
                    ("multiplier", &component.multiplier().to_string()),
                ],
            )?;
        }
        x.end("listOfUnits")?;
        x.end("unitDefinition")?;
    }
    x.end("listOfUnitDefinitions")
}
