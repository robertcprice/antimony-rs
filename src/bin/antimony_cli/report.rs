//! Versioned CLI-only reports; this is not a serialized model interchange format.
use super::args::{Command, HELP};
use antimony::{Diagnostic, LabelStyle, Severity, SourceFile, ValidatedModel};
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::fmt::Write as _;

/// Lossy display labels are explicitly marked; filesystem operations use OsStr.
pub(super) fn display(path: &OsStr) -> Value {
    json!({"display":path.to_string_lossy(), "lossy":path.to_str().is_none()})
}

pub(super) struct Report {
    pub command: Option<Command>,
    pub exit_code: u8,
    pub source: Value,
    pub model: Value,
    pub output: Value,
    pub diagnostics: Vec<Value>,
}
impl Report {
    pub(super) fn new(command: Option<Command>) -> Self {
        Self {
            command,
            exit_code: 0,
            source: Value::Null,
            model: Value::Null,
            output: Value::Null,
            diagnostics: Vec::new(),
        }
    }
    pub(super) fn model(&mut self, model: &ValidatedModel) {
        self.model = json!({
            "id":model.id(),"compartments":model.compartments().len(),
            "species":model.species().len(),"parameters":model.parameters().len(),
            "reactions":model.reactions().len(),"initial_assignments":model.initial_assignments().len(),
            "unit_definitions":model.unit_definitions().len()
        });
        for diagnostic in model.warnings() {
            self.diagnostic("resolve", diagnostic, model.source());
        }
    }
    pub(super) fn message(
        &mut self,
        stage: &str,
        severity: &str,
        code: &str,
        message: impl AsRef<str>,
    ) {
        self.diagnostics.push(json!({
            "stage":stage,"severity":severity,"code":code,"message":message.as_ref(),
            "labels":[],"notes":[]
        }));
    }
    pub(super) fn fail(
        &mut self,
        exit_code: u8,
        stage: &str,
        code: &str,
        message: impl AsRef<str>,
    ) {
        self.exit_code = exit_code;
        self.message(stage, "error", code, message);
    }
    pub(super) fn reject(&mut self, stage: &str, diagnostic: &Diagnostic, source: &SourceFile) {
        self.exit_code = 2;
        self.diagnostic(stage, diagnostic, source);
    }
    fn diagnostic(&mut self, stage: &str, diagnostic: &Diagnostic, source: &SourceFile) {
        let severity = match diagnostic.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
            _ => "unknown",
        };
        let labels = diagnostic.labels().iter().map(|label| {
            let span = label.span();
            let position = |offset| {
                if span.source_id() != source.id() {
                    return Value::Null;
                }
                source.location(offset).map_or(Value::Null, |p| {
                    json!({"line":p.line(),"column":p.column(),"byte_column":p.byte_column()})
                })
            };
            let style = match label.style() {
                LabelStyle::Primary => "primary",
                LabelStyle::Secondary => "secondary",
            };
            json!({"style":style,"source_id":span.source_id().get(),
                "start_byte":span.start(),"end_byte":span.end(),
                "start":position(span.start()),"end":position(span.end()),"message":label.message()})
        }).collect::<Vec<_>>();
        self.diagnostics
            .push(json!({"stage":stage,"severity":severity,
            "code":diagnostic.code().as_str(),"message":diagnostic.message(),
            "labels":labels,"notes":diagnostic.notes()}));
    }
    pub(super) fn value(&self) -> Value {
        let status = match self.exit_code {
            0 => "success",
            2 => "rejected",
            _ => "error",
        };
        json!({"schema":"antimony-cli-report-v1", "command":self.command.map(Command::name),
            "status":status,"exit_code":self.exit_code,"profile_id":antimony::IMPLEMENTED_PROFILE,
            "source":self.source,"model":self.model,"output":self.output,"diagnostics":self.diagnostics})
    }
    pub(super) fn human(&self) -> String {
        let mut result = String::new();
        let field = |value: &Value, key| {
            value
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        for diagnostic in &self.diagnostics {
            // Help is trusted static adapter text, never a source-derived message.
            if diagnostic["code"] == "help" && diagnostic["stage"] == "usage" {
                result.push_str(HELP);
                continue;
            }
            let _ = writeln!(
                result,
                "{}[{}] ({}): {}",
                escape(&field(diagnostic, "severity")),
                escape(&field(diagnostic, "code")),
                escape(&field(diagnostic, "stage")),
                escape(&field(diagnostic, "message"))
            );
            if let Some(labels) = diagnostic["labels"].as_array() {
                for label in labels {
                    let _ = writeln!(
                        result,
                        "  {}:{}:{} (bytes {}..{}, {}): {}",
                        escape(self.source["display"].as_str().unwrap_or("<source>")),
                        label["start"]["line"],
                        label["start"]["column"],
                        label["start_byte"],
                        label["end_byte"],
                        escape(&field(label, "style")),
                        escape(&field(label, "message"))
                    );
                }
            }
            if let Some(notes) = diagnostic["notes"].as_array() {
                for note in notes {
                    let _ = writeln!(result, "  note: {}", escape(note.as_str().unwrap_or("")));
                }
            }
        }
        if self.exit_code == 0
            && !self.model.is_null()
            && let Some(command) = self.command
        {
            let _ = writeln!(
                result,
                "{}: success (profile {})",
                command.name(),
                antimony::IMPLEMENTED_PROFILE
            );
        }
        result
    }
}

/// Keep display text readable without allowing terminal-control injection.
pub(super) fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for ch in text.chars() {
        if ch.is_control()
            || matches!(ch, '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            escaped.extend(ch.escape_default());
        } else {
            escaped.push(ch);
        }
    }
    escaped
}
