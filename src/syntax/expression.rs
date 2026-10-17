//! Explicit Pratt continuation stack: nested calls never consume the Rust call stack.
//! Binding powers and source arena ordering remain identical to arithmetic v0.1.
use super::*;
use crate::syntax::functions::Function;
#[derive(Clone, Copy)]
enum Infix {
    Arithmetic(BinaryOp),
    Compare(crate::ComparisonOp),
    Logical(crate::LogicalOp),
}
impl Infix {
    fn expression(self, left: usize, right: usize) -> ExprKind {
        match self {
            Self::Arithmetic(op) => ExprKind::Binary { op, left, right },
            Self::Compare(op) => ExprKind::Compare {
                op,
                args: vec![left, right],
                infix: true,
            },
            Self::Logical(op) => ExprKind::Logical {
                op,
                args: vec![left, right],
                infix: true,
            },
        }
    }
}

enum Task {
    Parse {
        min_bp: u8,
        recursion: usize,
    },
    Continue {
        min_bp: u8,
        recursion: usize,
    },
    Unary {
        op: Option<UnaryOp>,
        span: Span,
    },
    Binary {
        op: Infix,
        left: usize,
    },
    CloseParenthesis,
    Call {
        function: Function,
        args: Vec<usize>,
        span: Span,
        recursion: usize,
        has_arg: bool,
    },
}
impl Parser<'_> {
    pub(super) fn expression(&mut self, min_bp: u8, recursion: usize) -> Result<usize, Diagnostic> {
        let mut tasks = vec![Task::Parse { min_bp, recursion }];
        let mut values = Vec::<usize>::new();
        while let Some(task) = tasks.pop() {
            match task {
                Task::Parse { min_bp, recursion } => {
                    self.check_depth(recursion, self.token().span)?;
                    tasks.push(Task::Continue { min_bp, recursion });
                    let token = self.bump();
                    match token.kind {
                        Kind::Number(lexeme) => {
                            let value = lexeme.parse::<f64>().map_err(|_| {
                                error(DiagnosticCode::Syntax, token.span, "invalid number")
                            })?;
                            if !value.is_finite() {
                                return Err(error(
                                    DiagnosticCode::DeclarationConflict,
                                    token.span,
                                    "arithmetic literal must be finite",
                                ));
                            }
                            crate::syntax::numeric::check_scalar(value, &lexeme, token.span)?;
                            let value = crate::syntax::numeric::expression_value(&lexeme, value);
                            values.push(self.push_expr(
                                ExprKind::Number { lexeme, value },
                                token.span,
                                1,
                            )?);
                        }
                        Kind::Ident(text) => {
                            if matches!(
                                text.as_str(),
                                "true" | "True" | "TRUE" | "false" | "False" | "FALSE"
                            ) {
                                let mut end = token.span.end();
                                if self.token().kind == Kind::LParen {
                                    if matches!(text.as_str(), "true" | "false") {
                                        return Err(error(
                                            DiagnosticCode::Syntax,
                                            token.span,
                                            "lowercase Boolean constant is not callable",
                                        ));
                                    }
                                    self.bump();
                                    end = self.token().span.end();
                                    self.expect(
                                        Kind::RParen,
                                        "Boolean alias call accepts no arguments",
                                    )?;
                                }
                                let span = self
                                    .source
                                    .span(token.span.start(), end)
                                    .expect("Boolean span");
                                values.push(self.push_expr(
                                    ExprKind::Boolean(matches!(
                                        text.as_str(),
                                        "true" | "True" | "TRUE"
                                    )),
                                    span,
                                    1,
                                )?);
                            } else if text == "time" {
                                if self.active_function.is_some() {
                                    return Err(error(
                                        DiagnosticCode::DeclarationConflict,
                                        token.span,
                                        "time is not in scope inside a user-defined function body",
                                    ));
                                }
                                let mut end = token.span.end();
                                if self.token().kind == Kind::LParen {
                                    self.bump();
                                    end = self.token().span.end();
                                    self.expect(Kind::RParen, "time() accepts no arguments")?;
                                }
                                let span = self
                                    .source
                                    .span(token.span.start(), end)
                                    .expect("time span");
                                values.push(self.push_expr(ExprKind::Time, span, 1)?);
                            } else if matches!(text.as_str(), "pi" | "exponentiale") {
                                if self.token().kind == Kind::LParen {
                                    return Err(error(
                                        DiagnosticCode::Syntax,
                                        token.span,
                                        "mathematical constant is not callable",
                                    ));
                                }
                                let constant = if text == "pi" {
                                    crate::NumericConstant::Pi
                                } else {
                                    crate::NumericConstant::ExponentialE
                                };
                                values.push(self.push_expr(
                                    ExprKind::Constant(constant),
                                    token.span,
                                    1,
                                )?);
                            } else if let Some(function) = Function::from_name(&text) {
                                if text == "not" && self.token().kind != Kind::LParen {
                                    return Err(error(
                                        DiagnosticCode::UnsupportedFeature,
                                        token.span,
                                        "textual prefix not is deferred; use ! or not(...)",
                                    ));
                                }
                                if self.rational_at(self.pos)?.is_some() {
                                    return Err(error(
                                        DiagnosticCode::Syntax,
                                        self.token().span,
                                        "rational notation consumes the function opening parenthesis; group an operand to express division",
                                    ));
                                }
                                self.expect(
                                    Kind::LParen,
                                    "numeric builtin requires parenthesized arguments",
                                )?;
                                let has_arg = self.token().kind != Kind::RParen;
                                tasks.push(Task::Call {
                                    function,
                                    args: Vec::new(),
                                    span: token.span,
                                    recursion,
                                    has_arg,
                                });
                                if has_arg {
                                    tasks.push(Task::Parse {
                                        min_bp: 0,
                                        recursion: recursion + 1,
                                    });
                                }
                            } else {
                                if crate::syntax::reserved::builtin(&text) {
                                    return Err(error(
                                        DiagnosticCode::UnsupportedFeature,
                                        token.span,
                                        "builtin is outside this numeric slice",
                                    ));
                                }
                                if self.token().kind == Kind::LParen {
                                    if let Some(&(arity, _)) = self.declared_functions.get(&text) {
                                        if self.rational_at(self.pos)?.is_some() {
                                            return Err(error(
                                                DiagnosticCode::Syntax,
                                                self.token().span,
                                                "rational notation consumes the function opening parenthesis; group an operand to express division",
                                            ));
                                        }
                                        self.expect(
                                            Kind::LParen,
                                            "user function requires parenthesized arguments",
                                        )?;
                                        let has_arg = self.token().kind != Kind::RParen;
                                        tasks.push(Task::Call {
                                            function: Function::User {
                                                name: Name {
                                                    text,
                                                    span: token.span,
                                                },
                                                arity,
                                            },
                                            args: Vec::new(),
                                            span: token.span,
                                            recursion,
                                            has_arg,
                                        });
                                        if has_arg {
                                            tasks.push(Task::Parse {
                                                min_bp: 0,
                                                recursion: recursion + 1,
                                            });
                                        }
                                        continue;
                                    }
                                    if self.active_function.as_ref().is_some_and(|(cur, params)| {
                                        cur == &text || params.contains(&text)
                                    }) || self.is_declared_model_symbol(&text)
                                    {
                                        return Err(error(
                                            DiagnosticCode::DeclarationConflict,
                                            token.span,
                                            "recursive function call or calling a non-function symbol as a function is not permitted",
                                        ));
                                    }
                                    return Err(self.unsupported(
                                        "user functions must be declared before use",
                                    ));
                                }
                                if self.declared_functions.contains_key(&text)
                                    || self
                                        .active_function
                                        .as_ref()
                                        .is_some_and(|(cur, _)| cur == &text)
                                {
                                    return Err(error(
                                        DiagnosticCode::Syntax,
                                        token.span,
                                        "function identifier cannot be used as a bare variable",
                                    ));
                                }
                                if let Some((_, params)) = &self.active_function
                                    && !params.contains(&text)
                                {
                                    return Err(error(
                                        DiagnosticCode::DeclarationConflict,
                                        token.span,
                                        "function body may only reference its formal parameters",
                                    ));
                                }
                                let mut sym_text = text;
                                let mut sym_end = token.span.end();
                                while self.token().kind == Kind::Dot {
                                    self.bump();
                                    let part = self.name()?;
                                    sym_text.push('.');
                                    sym_text.push_str(&part.text);
                                    sym_end = part.span.end();
                                }
                                let sym_span = self
                                    .source
                                    .span(token.span.start(), sym_end)
                                    .expect("symbol span");
                                values.push(self.push_expr(
                                    ExprKind::Symbol(Name {
                                        text: sym_text,
                                        span: sym_span,
                                    }),
                                    sym_span,
                                    1,
                                )?);
                            }
                        }
                        Kind::Plus | Kind::Minus | Kind::Not => {
                            let op = match token.kind {
                                Kind::Plus => Some(UnaryOp::Plus),
                                Kind::Minus => Some(UnaryOp::Minus),
                                _ => None,
                            };
                            tasks.push(Task::Unary {
                                op,
                                span: token.span,
                            });
                            tasks.push(Task::Parse {
                                min_bp: 9,
                                recursion: recursion + 1,
                            });
                        }
                        Kind::LParen => {
                            if let Some((numerator, denominator, end)) =
                                self.rational_at(self.pos - 1)?
                            {
                                self.check_depth(recursion + 1, token.span)?;
                                let span = self
                                    .source
                                    .span(token.span.start(), self.tokens[end - 1].span.end())
                                    .expect("rational span");
                                self.pos = end;
                                values.push(self.push_expr(
                                    ExprKind::Rational {
                                        numerator,
                                        denominator,
                                    },
                                    span,
                                    1,
                                )?);
                                continue;
                            }
                            tasks.push(Task::CloseParenthesis);
                            tasks.push(Task::Parse {
                                min_bp: 0,
                                recursion: recursion + 1,
                            });
                        }
                        Kind::Unsupported => {
                            return Err(error(
                                DiagnosticCode::UnsupportedFeature,
                                token.span,
                                "expression construct is outside this slice",
                            ));
                        }
                        _ => {
                            return Err(error(
                                DiagnosticCode::Syntax,
                                token.span,
                                "expected arithmetic expression",
                            ));
                        }
                    }
                }
                Task::Continue { min_bp, recursion } => {
                    let (op, lbp, rbp) = match &self.token().kind {
                        Kind::Logical(op) => (Infix::Logical(*op), 1, 2),
                        Kind::Compare(op) => (Infix::Compare(*op), 3, 4),
                        Kind::Plus => (Infix::Arithmetic(BinaryOp::Add), 5, 6),
                        Kind::Minus => (Infix::Arithmetic(BinaryOp::Subtract), 5, 6),
                        Kind::Star => (Infix::Arithmetic(BinaryOp::Multiply), 7, 8),
                        Kind::Slash => (Infix::Arithmetic(BinaryOp::Divide), 7, 8),
                        Kind::Caret => (Infix::Arithmetic(BinaryOp::Power), 11, 11),
                        Kind::Ident(s) if matches!(s.as_str(), "and" | "or" | "AND" | "OR") => {
                            return Err(self.unsupported("textual logical operators are deferred; use &&/|| or functional calls"));
                        }
                        _ => continue,
                    };
                    if lbp < min_bp {
                        continue;
                    }
                    self.bump();
                    let left = values.pop().expect("parsed left operand");
                    tasks.push(Task::Continue { min_bp, recursion });
                    tasks.push(Task::Binary { op, left });
                    tasks.push(Task::Parse {
                        min_bp: rbp,
                        recursion: recursion + 1,
                    });
                }
                Task::Unary { op, span } => {
                    let child = values.pop().expect("parsed unary operand");
                    let span = self
                        .source
                        .span(span.start(), self.expressions[child].span.end())
                        .expect("unary span");
                    values.push(self.push_expr(
                        match op {
                            Some(op) => ExprKind::Unary { op, child },
                            None => ExprKind::Not { child },
                        },
                        span,
                        self.expressions[child].depth + 1,
                    )?);
                }
                Task::Binary { op, left } => {
                    let right = values.pop().expect("parsed right operand");
                    let span = self
                        .source
                        .span(
                            self.expressions[left].span.start(),
                            self.expressions[right].span.end(),
                        )
                        .expect("binary span");
                    let depth = self.expressions[left]
                        .depth
                        .max(self.expressions[right].depth)
                        + 1;
                    values.push(self.push_expr(op.expression(left, right), span, depth)?);
                }
                Task::CloseParenthesis => {
                    self.expect(Kind::RParen, "expected closing parenthesis")?;
                }
                Task::Call {
                    function,
                    mut args,
                    span,
                    recursion,
                    has_arg,
                } => {
                    if has_arg {
                        args.push(values.pop().expect("parsed call argument"));
                    }
                    if self.token().kind == Kind::Comma {
                        self.bump();
                        tasks.push(Task::Call {
                            function,
                            args,
                            span,
                            recursion,
                            has_arg: true,
                        });
                        tasks.push(Task::Parse {
                            min_bp: 0,
                            recursion: recursion + 1,
                        });
                        continue;
                    }
                    let end = self.token().span.end();
                    self.expect(Kind::RParen, "expected closing function parenthesis")?;
                    let depth = args
                        .iter()
                        .map(|id| self.expressions[*id].depth)
                        .max()
                        .unwrap_or(0)
                        + 1;
                    let span = self.source.span(span.start(), end).expect("call span");
                    values.push(self.push_expr(function.finish(args, span)?, span, depth)?);
                }
            }
        }
        debug_assert_eq!(values.len(), 1);
        Ok(values.pop().expect("parsed root"))
    }
}
