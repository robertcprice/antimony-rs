#[path = "events.rs"]
mod events;
#[path = "expression.rs"]
mod expression;
#[path = "rational.rs"]
mod rational;
#[path = "units.rs"]
mod units;
use super::lexer::{Kind, Token, lex};
use super::{
    BinaryOp, Expr, ExprKind, InitialValue, Name, ParsedDocument, ParsedFunction, ParsedModule,
    Participant, Statement, UnaryOp, error,
};
use crate::{Diagnostic, DiagnosticCode, LimitKind, Limits, SourceFile, Span};
use std::collections::{BTreeMap, BTreeSet};

// Bounded expression work even if a caller raises the configurable ceiling.
const HARD_DEPTH: usize = 256;
type ParsedModelBlock = (Name, bool, Vec<Name>, Vec<Statement>, Option<Span>, Span);

pub(crate) fn parse_document(
    source: &SourceFile,
    limits: &Limits,
) -> Result<ParsedDocument, Diagnostic> {
    limits
        .check(LimitKind::SourceBytes, source.text().len())
        .map_err(|e| {
            error(
                DiagnosticCode::ResourceLimit,
                source.span(0, 0).expect("empty span"),
                e.to_string(),
            )
        })?;
    let tokens = lex(source, limits)?;
    Parser {
        source,
        limits,
        tokens,
        pos: 0,
        functions: Vec::new(),
        declared_functions: BTreeMap::new(),
        active_function: None,
        modules: Vec::new(),
        declared_modules: BTreeMap::new(),
        expressions: Vec::new(),
        statements: Vec::new(),
    }
    .document()
}
pub(super) struct Parser<'a> {
    pub(super) source: &'a SourceFile,
    pub(super) limits: &'a Limits,
    pub(super) tokens: Vec<Token>,
    pub(super) pos: usize,
    pub(super) functions: Vec<ParsedFunction>,
    pub(super) declared_functions: BTreeMap<String, (usize, Span)>,
    pub(super) active_function: Option<(String, BTreeSet<String>)>,
    pub(super) modules: Vec<ParsedModule>,
    pub(super) declared_modules: BTreeMap<String, (usize, Span)>,
    pub(super) expressions: Vec<Expr>,
    pub(super) statements: Vec<Statement>,
}
impl Parser<'_> {
    pub(super) fn token(&self) -> &Token {
        &self.tokens[self.pos]
    }
    pub(super) fn bump(&mut self) -> Token {
        let t = self.token().clone();
        if t.kind != Kind::Eof {
            self.pos += 1;
        }
        t
    }
    pub(super) fn is_word(&self, word: &str) -> bool {
        matches!(&self.token().kind, Kind::Ident(s) if s == word)
    }
    pub(super) fn fail(&self, message: &str) -> Diagnostic {
        error(DiagnosticCode::Syntax, self.token().span, message)
    }
    pub(super) fn unsupported(&self, message: &str) -> Diagnostic {
        error(
            DiagnosticCode::UnsupportedFeature,
            self.token().span,
            message,
        )
    }
    pub(super) fn expect(&mut self, kind: Kind, message: &str) -> Result<Token, Diagnostic> {
        if self.token().kind == kind {
            Ok(self.bump())
        } else {
            Err(self.fail(message))
        }
    }
    pub(super) fn name(&mut self) -> Result<Name, Diagnostic> {
        let token = self.bump();
        if matches!(&token.kind, Kind::Ident(text) if super::reserved::builtin(text) || text == "default_compartment")
        {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                token.span,
                "reserved builtin identifiers cannot be declared in this slice",
            ));
        }
        match token.kind {
            Kind::Ident(text)
                if !matches!(
                    text.as_str(),
                    "model"
                        | "module"
                        | "end"
                        | "species"
                        | "compartment"
                        | "const"
                        | "var"
                        | "in"
                        | "at"
                        | "after"
                        | "is"
                        | "delete"
                        | "function"
                        | "import"
                        | "unit"
                        | "formula"
                        | "substanceOnly"
                        | "time"
                        | "avogadro"
                ) =>
            {
                Ok(Name {
                    text,
                    span: token.span,
                })
            }
            _ => Err(error(
                DiagnosticCode::Syntax,
                token.span,
                "expected a non-reserved ASCII identifier",
            )),
        }
    }
    pub(super) fn dotted_name(&mut self) -> Result<Name, Diagnostic> {
        let first = self.name()?;
        let mut text = first.text;
        let mut end = first.span.end();
        while self.token().kind == Kind::Dot {
            self.bump();
            let part = self.name()?;
            text.push('.');
            text.push_str(&part.text);
            end = part.span.end();
        }
        let span = self
            .source
            .span(first.span.start(), end)
            .expect("dotted name span");
        Ok(Name { text, span })
    }
    fn function_ident(&mut self, is_param: bool) -> Result<Name, Diagnostic> {
        if self.token().kind == Kind::Dollar {
            return Err(
                self.unsupported("boundary '$' modifier is not supported on function parameters")
            );
        }
        if let Kind::Ident(text) = &self.token().kind
            && (crate::model::units::UnitKind::from_name(text).is_some()
                || (is_param
                    && matches!(
                        text.as_str(),
                        "true" | "True" | "TRUE" | "false" | "False" | "FALSE"
                    )))
        {
            return Err(
                self.fail("reserved unit or Boolean constant cannot name a function or parameter")
            );
        }
        self.name()
    }
    pub(super) fn is_declared_model_symbol(&self, target: &str) -> bool {
        self.statements.iter().any(|s| match s {
            Statement::UnitDefinition { name, .. }
            | Statement::UnitAnnotation { name, .. }
            | Statement::Compartment { name, .. }
            | Statement::Parameter { name, .. }
            | Statement::Initial { name, .. }
            | Statement::Rule { name, .. } => name.text == target,
            Statement::Species {
                name, compartment, ..
            } => name.text == target || compartment.as_ref().is_some_and(|c| c.text == target),
            Statement::Reaction {
                name,
                reactants,
                products,
                ..
            } => {
                name.text == target
                    || reactants
                        .iter()
                        .chain(products)
                        .any(|p| p.name.text == target)
            }
            Statement::Event {
                name, assignments, ..
            } => name.text == target || assignments.iter().any(|a| a.variable.text == target),
            Statement::Alias { left, right, .. } => left.text == target || right.text == target,
            Statement::Submodel {
                instance,
                args,
                timeconv,
                extentconv,
                ..
            } => {
                instance.text == target
                    || args.iter().any(|a| a.text == target)
                    || timeconv.as_ref().is_some_and(|t| t.text == target)
                    || extentconv.as_ref().is_some_and(|e| e.text == target)
            }
            Statement::Delete { targets, .. } => targets.iter().any(|t| t.text == target),
        })
    }
    pub(super) fn separators(&mut self) {
        while matches!(self.token().kind, Kind::Sep | Kind::Newline) {
            self.bump();
        }
    }
    pub(super) fn terminator(&mut self) -> Result<(), Diagnostic> {
        if matches!(self.token().kind, Kind::Sep | Kind::Newline) {
            self.separators();
            Ok(())
        } else if self.token().kind == Kind::Eof || self.is_word("end") {
            Ok(())
        } else if matches!(
            self.token().kind,
            Kind::Unsupported | Kind::LParen | Kind::Ident(_)
        ) {
            Err(self
                .unsupported("construct or inline unit quantity is outside this flat-model slice"))
        } else {
            Err(self.fail("expected newline or semicolon"))
        }
    }
    fn function_definition(&mut self) -> Result<(), Diagnostic> {
        let start = self.bump().span;
        let fn_name = self.function_ident(false)?;
        if fn_name.text == "__main"
            || self.declared_functions.contains_key(&fn_name.text)
            || self.declared_modules.contains_key(&fn_name.text)
            || self.is_declared_model_symbol(&fn_name.text)
        {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                fn_name.span,
                format!(
                    "duplicate or conflicting function declaration {}",
                    fn_name.text
                ),
            ));
        }
        self.expect(Kind::LParen, "expected '(' after function name")?;
        let mut parameters = Vec::new();
        let mut param_set = BTreeSet::new();
        if self.token().kind != Kind::RParen {
            loop {
                let param = self.function_ident(true)?;
                if param.text == fn_name.text
                    || self.declared_functions.contains_key(&param.text)
                    || !param_set.insert(param.text.clone())
                {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        param.span,
                        format!("duplicate or conflicting formal parameter {}", param.text),
                    ));
                }
                parameters.push(param);
                if self.token().kind != Kind::Comma {
                    break;
                }
                self.bump();
                if self.token().kind == Kind::RParen {
                    return Err(
                        self.fail("trailing comma is not permitted in function parameter list")
                    );
                }
            }
        }
        let rparen = self.expect(Kind::RParen, "expected ')' after function parameters")?;
        self.separators();
        if self.is_word("end") {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                rparen.span,
                "empty function definitions without a body expression are outside this slice",
            ));
        }
        if self.tokens[self.pos..]
            .iter()
            .take_while(|t| {
                !matches!(t.kind, Kind::Sep | Kind::Newline | Kind::Eof)
                    && !matches!(&t.kind, Kind::Ident(s) if s == "end")
            })
            .any(|t| {
                matches!(
                    t.kind,
                    Kind::Equal | Kind::ColonEqual | Kind::PrimeEqual | Kind::Arrow(_)
                )
            })
        {
            return Err(self.fail(
                "assignments, rules, and reactions are not permitted inside a function body",
            ));
        }
        self.active_function = Some((fn_name.text.clone(), param_set));
        let body = self.expression(0, 1)?;
        self.active_function = None;
        self.separators();
        if !self.is_word("end") {
            return Err(self.fail(
                "expected 'end' to close function definition; only one body expression is permitted",
            ));
        }
        let end_span = self.bump().span;
        self.terminator()?;
        self.limits
            .checked_add(LimitKind::ModelEntities, self.functions.len(), 1)
            .map_err(|e| error(DiagnosticCode::ResourceLimit, fn_name.span, e.to_string()))?;
        self.declared_functions
            .insert(fn_name.text.clone(), (parameters.len(), fn_name.span));
        let span = self
            .source
            .span(start.start(), end_span.end())
            .expect("function span");
        self.functions.push(ParsedFunction {
            name: fn_name,
            parameters,
            body,
            span,
        });
        Ok(())
    }
    fn parse_model_block(&mut self) -> Result<ParsedModelBlock, Diagnostic> {
        let start = self.bump().span;
        let starred = if self.token().kind == Kind::Star {
            self.bump();
            true
        } else {
            false
        };
        let model_name = self.name()?;
        if self.declared_functions.contains_key(&model_name.text)
            || self.declared_modules.contains_key(&model_name.text)
        {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                model_name.span,
                "model identifier collides with an existing user-defined function or module",
            ));
        }
        let mut ports = Vec::new();
        let mut port_set = BTreeSet::new();
        let mut port_dollar_span = None;
        if self.token().kind == Kind::LParen {
            self.bump();
            if self.token().kind != Kind::RParen {
                loop {
                    if self.token().kind == Kind::Dollar {
                        let d_span = self.bump().span;
                        if port_dollar_span.is_none() {
                            port_dollar_span = Some(d_span);
                        }
                    }
                    let port = self.name()?;
                    if !port_set.insert(port.text.clone()) {
                        return Err(error(
                            DiagnosticCode::DeclarationConflict,
                            port.span,
                            format!("duplicate model port {}", port.text),
                        ));
                    }
                    ports.push(port);
                    if self.token().kind != Kind::Comma {
                        break;
                    }
                    self.bump();
                }
            }
            self.expect(Kind::RParen, "expected ')' after model ports")?;
        }
        if !matches!(self.token().kind, Kind::Sep | Kind::Newline) {
            if matches!(self.token().kind, Kind::Ident(_)) {
                return Err(self.unsupported(
                    "this slice requires a newline or semicolon after the model header",
                ));
            }
            return Err(self.fail("expected newline or semicolon after model header"));
        }
        self.bump();
        self.separators();
        let prev_statements = std::mem::take(&mut self.statements);
        while self.token().kind != Kind::Eof && !self.is_word("end") {
            if self.is_word("function") {
                return Err(
                    self.fail("function definitions must be at top level outside model blocks")
                );
            }
            self.statement()?;
        }
        if !self.is_word("end") {
            return Err(self.fail("named model is missing end"));
        }
        let end_span = self.bump().span;
        self.separators();
        let block_statements = std::mem::replace(&mut self.statements, prev_statements);
        let span = self
            .source
            .span(start.start(), end_span.end())
            .expect("model block span");
        Ok((
            model_name,
            starred,
            ports,
            block_statements,
            port_dollar_span,
            span,
        ))
    }
    fn document(mut self) -> Result<ParsedDocument, Diagnostic> {
        self.separators();
        let mut parsed_blocks: Vec<ParsedModelBlock> = Vec::new();
        while self.token().kind != Kind::Eof {
            if self.is_word("function") {
                self.function_definition()?;
                self.separators();
            } else if self.is_word("model") || self.is_word("module") {
                if !self.statements.is_empty() {
                    return Err(self.unsupported(
                        "mixing top-level model statements with named model blocks is outside this slice",
                    ));
                }
                let block = self.parse_model_block()?;
                self.declared_modules
                    .insert(block.0.text.clone(), (block.2.len(), block.0.span));
                parsed_blocks.push(block);
                self.separators();
            } else {
                self.statement()?;
            }
        }
        let name = if parsed_blocks.is_empty() {
            "__main".to_owned()
        } else if !self.statements.is_empty() {
            if parsed_blocks
                .iter()
                .any(|(_, starred, _, _, _, _)| *starred)
            {
                return Err(self.unsupported(
                    "mixing top-level model statements with a starred model block is outside this slice",
                ));
            }
            for (m_name, _, ports, stmts, port_dollar, span) in &parsed_blocks {
                if let Some(d_span) = port_dollar {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        *d_span,
                        "boundary '$' modifiers in model port lists are outside this slice",
                    ));
                }
                self.modules.push(ParsedModule {
                    name: m_name.clone(),
                    ports: ports.clone(),
                    statements: stmts.clone(),
                    span: *span,
                });
            }
            "__main".to_owned()
        } else {
            let main_idx = parsed_blocks
                .iter()
                .rposition(|(_, starred, _, _, _, _)| *starred)
                .unwrap_or(parsed_blocks.len() - 1);
            for (idx, (m_name, _, ports, stmts, port_dollar, span)) in
                parsed_blocks.iter().enumerate()
            {
                if let Some(d_span) = port_dollar {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        *d_span,
                        "boundary '$' modifiers in model port lists are outside this slice",
                    ));
                }
                if idx == main_idx && !ports.is_empty() {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        ports[0].span,
                        "model ports on the main exported model are outside this slice",
                    ));
                }
                if idx != main_idx {
                    self.modules.push(ParsedModule {
                        name: m_name.clone(),
                        ports: ports.clone(),
                        statements: stmts.clone(),
                        span: *span,
                    });
                }
            }
            let (main_name, _, _, main_stmts, _, _) = parsed_blocks.swap_remove(main_idx);
            self.statements = main_stmts;
            main_name.text
        };
        if self.statements.is_empty() && self.functions.is_empty() && self.modules.is_empty() {
            return Err(self.fail("empty models are outside the initialization flat-model slice"));
        }
        Ok(ParsedDocument {
            source: self.source.clone(),
            limits: self.limits.clone(),
            name,
            functions: self.functions,
            modules: self.modules,
            statements: self.statements,
            expressions: self.expressions,
        })
    }
    fn statement(&mut self) -> Result<(), Diagnostic> {
        if self.unit_statement()? {
            return Ok(());
        }
        let start = self.token().span;
        if self.is_word("at") {
            let empty_name = Name {
                text: String::new(),
                span: start,
            };
            let ev = self.event_statement(empty_name, start)?;
            self.statements.push(ev);
            return self.terminator();
        }
        if self.is_word("delete") {
            self.bump();
            let mut targets = Vec::new();
            loop {
                targets.push(self.dotted_name()?);
                if self.token().kind != Kind::Comma {
                    break;
                }
                self.bump();
            }
            self.statements.push(Statement::Delete { targets });
            return self.terminator();
        }
        if matches!(
            &self.token().kind,
            Kind::Ident(s)
                if matches!(
                    s.as_str(),
                    "import" | "function" | "unit" | "units" | "model" | "module"
                )
        ) {
            return Err(self.unsupported("feature is outside this flat-model slice"));
        }
        let qualifier = if self.is_word("const") || self.is_word("var") {
            let value = self.is_word("const");
            self.bump();
            Some(value)
        } else {
            None
        };
        let substance_only = if self.is_word("substanceOnly") {
            self.bump();
            if !self.is_word("species") {
                return Err(self.fail("'substanceOnly' must be followed by 'species'"));
            }
            true
        } else {
            false
        };
        let compartment = self.is_word("compartment");
        let species = self.is_word("species");
        let formula = self.is_word("formula");
        if compartment || species || formula || qualifier.is_some() {
            if compartment || species || formula {
                self.bump();
            }
            loop {
                let dollar = self.token().kind == Kind::Dollar;
                if dollar {
                    self.bump();
                }
                let name = self.name()?;
                let qualifier = if dollar { Some(true) } else { qualifier };
                let location = if species && self.is_word("in") {
                    self.bump();
                    Some(self.name()?)
                } else {
                    None
                };
                let (value, rule) = match self.token().kind {
                    Kind::Equal => {
                        self.bump();
                        (Some(self.initial_value()?), None)
                    }
                    Kind::ColonEqual | Kind::PrimeEqual => {
                        let op = self.bump();
                        let kind = if op.kind == Kind::ColonEqual {
                            crate::RuleKind::Assignment
                        } else {
                            crate::RuleKind::Rate
                        };
                        let (expression, lexical_scalar) = self.rule_rhs(kind, op.span)?;
                        let span = self
                            .source
                            .span(name.span.start(), self.tokens[self.pos - 1].span.end())
                            .expect("rule span");
                        (
                            None,
                            Some(Statement::Rule {
                                kind,
                                name: name.clone(),
                                expression,
                                lexical_scalar,
                                qualifier: dollar.then_some(true),
                                span,
                            }),
                        )
                    }
                    _ => (None, None),
                };
                if let Some(rule) = rule {
                    self.statements.push(rule);
                }
                if compartment {
                    self.statements.push(Statement::Compartment {
                        name,
                        value,
                        qualifier,
                    });
                } else if species {
                    self.statements.push(Statement::Species {
                        name: name.clone(),
                        compartment: location,
                        boundary: qualifier == Some(true),
                        qualifier,
                        substance_only,
                    });
                    if let Some(value) = value {
                        self.statements.push(Statement::Initial { name, value });
                    }
                } else {
                    self.statements.push(Statement::Parameter {
                        name,
                        value,
                        qualifier,
                    });
                }
                if self.token().kind != Kind::Comma {
                    break;
                }
                self.bump();
            }
            return self.terminator();
        }
        // Check for named event (`E1: at ...`) or named submodel instantiation (`M1: sub(...)`)
        let named_prefix = matches!(self.token().kind, Kind::Ident(_))
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.kind == Kind::Colon);
        if named_prefix
            && matches!(
                self.tokens.get(self.pos + 2).map(|t| &t.kind),
                Some(Kind::Ident(s)) if s == "at"
            )
        {
            let name = self.name()?;
            self.bump(); // consume ':'
            let ev = self.event_statement(name, start)?;
            self.statements.push(ev);
            return self.terminator();
        }
        if named_prefix
            && matches!(
                self.tokens.get(self.pos + 2).map(|t| &t.kind),
                Some(Kind::Ident(_))
            )
            && self
                .tokens
                .get(self.pos + 3)
                .is_some_and(|t| t.kind == Kind::LParen)
        {
            let instance = self.name()?;
            self.bump(); // consume ':'
            let sub = self.submodel_call(instance, start)?;
            self.statements.push(sub);
            return self.terminator();
        }
        // Check for unnamed submodel instantiation (`sub(A, B);`)
        if matches!(self.token().kind, Kind::Ident(_))
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.kind == Kind::LParen)
        {
            let empty_instance = Name {
                text: String::new(),
                span: start,
            };
            let sub = self.submodel_call(empty_instance, start)?;
            self.statements.push(sub);
            return self.terminator();
        }
        // Recognize reaction grammar before consuming a possible first participant.
        let reaction = named_prefix
            || self.tokens[self.pos..]
                .iter()
                .take_while(|t| !matches!(t.kind, Kind::Sep | Kind::Newline | Kind::Eof))
                .any(|t| matches!(t.kind, Kind::Arrow(_)));
        if reaction {
            let name = if named_prefix {
                let name = self.name()?;
                self.bump();
                name
            } else {
                Name {
                    text: String::new(),
                    span: start,
                }
            };
            let reactants = self.participants(true)?;
            let arrow = self.bump();
            let Kind::Arrow(reversible) = arrow.kind else {
                return Err(error(
                    DiagnosticCode::Syntax,
                    arrow.span,
                    "expected -> or => reaction arrow",
                ));
            };
            let products = self.participants(false)?;
            self.expect(
                Kind::Sep,
                "reaction participants must be followed by a semicolon (not newline)",
            )?;
            let rate = self.expression(0, 1)?;
            self.statements.push(Statement::Reaction {
                name,
                start,
                reactants,
                products,
                reversible,
                rate,
            });
        } else {
            let num_offset = usize::from(matches!(self.token().kind, Kind::Minus | Kind::Plus));
            if matches!(
                self.tokens.get(self.pos + num_offset).map(|t| &t.kind),
                Some(Kind::Number(_))
            ) && self
                .tokens
                .get(self.pos + num_offset + 1)
                .is_some_and(|t| t.kind == Kind::Equal)
            {
                return Err(self.unsupported("algebraic rules are outside this slice"));
            }
            let dollar = self.token().kind == Kind::Dollar;
            if dollar {
                self.bump();
            }
            let name = self.dotted_name()?;
            if self.declared_functions.contains_key(&name.text) {
                return Err(error(
                    DiagnosticCode::Syntax,
                    name.span,
                    "declared function name cannot be assigned a scalar or rule",
                ));
            }
            if !dollar && self.is_word("is") {
                self.bump();
                if self.token().kind == Kind::Unsupported {
                    return Err(self.unsupported(
                        "display-name annotations (a is \"...\") are outside this slice",
                    ));
                }
                let right = self.dotted_name()?;
                let span = self
                    .source
                    .span(name.span.start(), right.span.end())
                    .expect("alias span");
                self.statements.push(Statement::Alias {
                    left: name,
                    right,
                    span,
                });
                return self.terminator();
            }
            match self.token().kind {
                Kind::Equal => {
                    if dollar {
                        self.statements.push(Statement::Parameter {
                            name: name.clone(),
                            value: None,
                            qualifier: Some(true),
                        });
                    }
                    self.bump();
                    let value = self.initial_value()?;
                    self.statements.push(Statement::Initial { name, value });
                }
                Kind::ColonEqual | Kind::PrimeEqual => {
                    let op = self.bump();
                    let kind = if op.kind == Kind::ColonEqual {
                        crate::RuleKind::Assignment
                    } else {
                        crate::RuleKind::Rate
                    };
                    let (expression, lexical_scalar) = self.rule_rhs(kind, op.span)?;
                    let span = self
                        .source
                        .span(start.start(), self.tokens[self.pos - 1].span.end())
                        .expect("rule span");
                    self.statements.push(Statement::Rule {
                        kind,
                        name,
                        expression,
                        lexical_scalar,
                        qualifier: dollar.then_some(true),
                        span,
                    });
                }
                Kind::Unsupported => {
                    return Err(self.unsupported("rules and annotations are outside this slice"));
                }
                _ if matches!(self.token().kind, Kind::Sep | Kind::Newline | Kind::Eof)
                    || self.is_word("end") =>
                {
                    return Err(error(
                        DiagnosticCode::Syntax,
                        name.span,
                        "bare identifiers are not complete Antimony statements",
                    ));
                }
                _ => return Err(self.unsupported("construct is outside this flat-model slice")),
            }
        }
        self.terminator()
    }
    fn submodel_call(&mut self, instance: Name, start: Span) -> Result<Statement, Diagnostic> {
        let module = self.name()?;
        self.expect(Kind::LParen, "expected '(' after submodel name")?;
        let mut args = Vec::new();
        if self.token().kind != Kind::RParen {
            loop {
                args.push(self.dotted_name()?);
                if self.token().kind != Kind::Comma {
                    break;
                }
                self.bump();
            }
        }
        let rparen = self.expect(Kind::RParen, "expected ')' after submodel arguments")?;
        let mut timeconv = None;
        let mut extentconv = None;
        let mut end_pos = rparen.span.end();
        while self.token().kind == Kind::Comma {
            self.bump();
            if self.is_word("timeconv") {
                let kw_span = self.bump().span;
                if timeconv.is_some() {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        kw_span,
                        "duplicate 'timeconv' modifier on submodel instantiation",
                    ));
                }
                self.expect(Kind::Equal, "expected '=' after 'timeconv'")?;
                if matches!(
                    self.token().kind,
                    Kind::Number(_) | Kind::Minus | Kind::Plus
                ) {
                    return Err(self.unsupported(
                        "literal numeric submodel conversion factors overwrite a shared parameter in upstream libAntimony; bind an explicit named parameter",
                    ));
                }
                let conv = self.dotted_name()?;
                end_pos = conv.span.end();
                timeconv = Some(conv);
            } else if self.is_word("extentconv") {
                let kw_span = self.bump().span;
                if extentconv.is_some() {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        kw_span,
                        "duplicate 'extentconv' modifier on submodel instantiation",
                    ));
                }
                self.expect(Kind::Equal, "expected '=' after 'extentconv'")?;
                if matches!(
                    self.token().kind,
                    Kind::Number(_) | Kind::Minus | Kind::Plus
                ) {
                    return Err(self.unsupported(
                        "literal numeric submodel conversion factors overwrite a shared parameter in upstream libAntimony; bind an explicit named parameter",
                    ));
                }
                let conv = self.dotted_name()?;
                end_pos = conv.span.end();
                extentconv = Some(conv);
            } else {
                return Err(self.fail(
                    "expected 'timeconv = <ident>' or 'extentconv = <ident>' after ',' in submodel instantiation",
                ));
            }
        }
        let span = self
            .source
            .span(start.start(), end_pos)
            .expect("submodel call span");
        Ok(Statement::Submodel {
            instance,
            module,
            args,
            timeconv,
            extentconv,
            span,
        })
    }
    fn rule_rhs(
        &mut self,
        kind: crate::RuleKind,
        op_span: Span,
    ) -> Result<(usize, Option<f64>), Diagnostic> {
        if matches!(self.token().kind, Kind::Sep | Kind::Comma | Kind::Eof) {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                op_span,
                "empty rule clearing is outside this slice",
            ));
        }
        if self.token().kind == Kind::Newline {
            let mut look = self.pos;
            while matches!(self.tokens[look].kind, Kind::Newline | Kind::Sep) {
                look += 1;
            }
            if matches!(
                self.tokens[look].kind,
                Kind::Eof | Kind::Ident(_) | Kind::Dollar
            ) {
                return Err(error(
                    DiagnosticCode::UnsupportedFeature,
                    op_span,
                    "empty rule clearing is outside this slice",
                ));
            }
            return Err(self.fail("rule right-hand side cannot begin after a newline"));
        }
        if self.is_word("end") {
            return Err(self.fail("expected rule expression before end"));
        }
        let lexical_scalar = if kind == crate::RuleKind::Assignment {
            let offset = usize::from(self.token().kind == Kind::Minus);
            if let Some(Kind::Number(text)) = self.tokens.get(self.pos + offset).map(|t| &t.kind)
                && self.tokens.get(self.pos + offset + 1).is_some_and(|t| {
                    matches!(t.kind, Kind::Sep | Kind::Newline | Kind::Comma | Kind::Eof)
                        || matches!(&t.kind, Kind::Ident(s) if s == "end")
                })
            {
                let sign = if offset == 1 { -1.0 } else { 1.0 };
                text.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .map(|v| v * sign)
            } else {
                None
            }
        } else {
            None
        };
        let expression = self.expression(0, 1)?;
        Ok((expression, lexical_scalar))
    }
    fn initial_value(&mut self) -> Result<InitialValue, Diagnostic> {
        // Preserve the scalar serialization envelope, but only for exact
        // literal syntax. Parentheses and unary plus are assignments upstream.
        let offset = usize::from(self.token().kind == Kind::Minus);
        if matches!(
            self.tokens.get(self.pos + offset).map(|t| &t.kind),
            Some(Kind::Number(_))
        ) && self.tokens.get(self.pos + offset + 1).is_some_and(|t| {
            matches!(t.kind, Kind::Sep | Kind::Newline | Kind::Comma | Kind::Eof)
                || matches!(&t.kind, Kind::Ident(s) if s == "end")
        }) {
            return self.scalar().map(InitialValue::Scalar);
        }
        self.expression(0, 1).map(InitialValue::Expression)
    }
    fn scalar(&mut self) -> Result<f64, Diagnostic> {
        let sign = if self.token().kind == Kind::Minus {
            self.bump();
            -1.0
        } else {
            if self.token().kind == Kind::Plus {
                self.bump();
            }
            1.0
        };
        let token = self.bump();
        let Kind::Number(text) = token.kind else {
            return Err(error(
                DiagnosticCode::UnsupportedFeature,
                token.span,
                "this slice requires literal numeric initialization; expressions are deferred",
            ));
        };
        let value = text.parse::<f64>().map_err(|_| {
            error(
                DiagnosticCode::Syntax,
                token.span,
                "invalid numeric literal",
            )
        })? * sign;
        if !value.is_finite() {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                token.span,
                "initial value must be finite",
            ));
        }
        super::numeric::check_scalar(value, &text, token.span)?;
        if matches!(
            self.token().kind,
            Kind::Plus
                | Kind::Minus
                | Kind::Star
                | Kind::Slash
                | Kind::Caret
                | Kind::LParen
                | Kind::Compare(_)
                | Kind::Logical(_)
        ) {
            return Err(self.unsupported(
                "compartment expression initialization is deferred; use a fixed scalar literal",
            ));
        }
        Ok(value)
    }
    fn participants(&mut self, left: bool) -> Result<Vec<Participant>, Diagnostic> {
        let mut result = Vec::new();
        if (left && matches!(self.token().kind, Kind::Arrow(_)))
            || (!left && self.token().kind == Kind::Sep)
        {
            return Ok(result);
        }
        loop {
            let stoichiometry = if let Kind::Number(text) = &self.token().kind {
                let text = text.clone();
                let value = text.parse::<f64>().unwrap_or(f64::NAN);
                let span = self.bump().span;
                super::numeric::check_scalar(value, &text, span)?;
                if !value.is_finite() || value <= 0.0 {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        span,
                        "stoichiometry must be finite and positive",
                    ));
                }
                value
            } else {
                1.0
            };
            let boundary = self.token().kind == Kind::Dollar;
            if boundary {
                self.bump();
            }
            let name = self.dotted_name()?;
            result.push(Participant {
                name,
                stoichiometry,
                boundary,
            });
            if self.token().kind != Kind::Plus {
                break;
            }
            self.bump();
        }
        Ok(result)
    }
    fn check_depth(&self, depth: usize, span: Span) -> Result<(), Diagnostic> {
        self.limits
            .check(LimitKind::ExpressionDepth, depth)
            .map_err(|e| error(DiagnosticCode::ResourceLimit, span, e.to_string()))?;
        if depth > HARD_DEPTH {
            return Err(error(
                DiagnosticCode::ResourceLimit,
                span,
                "expression exceeds implementation hard depth ceiling 256",
            ));
        }
        Ok(())
    }
    fn push_expr(&mut self, kind: ExprKind, span: Span, depth: usize) -> Result<usize, Diagnostic> {
        self.check_depth(depth, span)?;
        self.limits
            .checked_add(LimitKind::Tokens, self.expressions.len(), 1)
            .map_err(|e| error(DiagnosticCode::ResourceLimit, span, e.to_string()))?;
        let index = self.expressions.len();
        self.expressions.push(Expr { kind, span, depth });
        Ok(index)
    }
}
