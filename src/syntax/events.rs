//! Parsing for `at` discrete event statements, triggers, delays, options, and assignments.
use super::{Kind, Name, Parser, Statement, error};
use crate::syntax::ParsedEventAssignment;
use crate::{Diagnostic, DiagnosticCode, Span};

impl Parser<'_> {
    pub(super) fn event_statement(
        &mut self,
        name: Name,
        start: Span,
    ) -> Result<Statement, Diagnostic> {
        let at_tok = self.bump();
        debug_assert!(matches!(&at_tok.kind, Kind::Ident(s) if s == "at"));

        let first_expr = self.expression(0, 1)?;
        let (delay, trigger) = if self.is_word("after") {
            self.bump();
            let trig = self.expression(0, 1)?;
            (Some(first_expr), trig)
        } else {
            (None, first_expr)
        };

        let mut use_values_from_trigger_time = true;
        let mut persistent = true;
        let mut initial_value = true;
        let mut priority = None;

        while self.token().kind == Kind::Comma {
            self.bump();
            let opt_tok = self.bump();
            let Kind::Ident(opt_name) = opt_tok.kind else {
                return Err(error(
                    DiagnosticCode::Syntax,
                    opt_tok.span,
                    "expected event option name ('t0', 'persistent', 'fromTrigger', or 'priority') after ','",
                ));
            };
            self.expect(Kind::Equal, "expected '=' after event option name")?;
            match opt_name.as_str() {
                "t0" => {
                    initial_value = self.event_bool_option("t0")?;
                }
                "persistent" => {
                    persistent = self.event_bool_option("persistent")?;
                }
                "fromTrigger" => {
                    use_values_from_trigger_time = self.event_bool_option("fromTrigger")?;
                }
                "priority" => {
                    priority = Some(self.expression(0, 1)?);
                }
                _ => {
                    return Err(error(
                        DiagnosticCode::Syntax,
                        opt_tok.span,
                        format!(
                            "unknown event option {opt_name:?}; supported options are 't0', 'persistent', 'fromTrigger', and 'priority'"
                        ),
                    ));
                }
            }
        }

        let colon = self.expect(Kind::Colon, "expected ':' after event trigger")?;
        if matches!(self.token().kind, Kind::Sep | Kind::Newline | Kind::Eof) || self.is_word("end")
        {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                colon.span,
                "events without event assignments are outside this slice",
            ));
        }

        let mut assignments = Vec::new();
        loop {
            let var = self.dotted_name()?;
            if self.declared_functions.contains_key(&var.text) {
                return Err(error(
                    DiagnosticCode::Syntax,
                    var.span,
                    "declared function name cannot be an event assignment target",
                ));
            }
            self.expect(
                Kind::Equal,
                "expected '=' in event assignment (use '=' rather than ':=')",
            )?;
            let expr = self.expression(0, 1)?;
            let assign_end = self.tokens[self.pos - 1].span.end();
            let assign_span = self
                .source
                .span(var.span.start(), assign_end)
                .expect("event assignment span");
            assignments.push(ParsedEventAssignment {
                variable: var,
                expression: expr,
                span: assign_span,
            });
            if self.token().kind != Kind::Comma {
                break;
            }
            self.bump();
        }

        let end_pos = self.tokens[self.pos - 1].span.end();
        let span = self
            .source
            .span(start.start(), end_pos)
            .expect("event span");
        Ok(Statement::Event {
            name,
            start,
            trigger,
            delay,
            priority,
            use_values_from_trigger_time,
            persistent,
            initial_value,
            assignments,
            span,
        })
    }

    fn event_bool_option(&mut self, option_name: &str) -> Result<bool, Diagnostic> {
        let tok = self.bump();
        match &tok.kind {
            Kind::Ident(s) if s == "true" => Ok(true),
            Kind::Ident(s) if s == "false" => Ok(false),
            _ => Err(error(
                DiagnosticCode::Syntax,
                tok.span,
                format!("event option '{option_name}' requires lowercase 'true' or 'false'"),
            )),
        }
    }
}
