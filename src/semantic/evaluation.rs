//! One finite typed evaluator with explicit demand continuations. Static dependency
//! and unit walkers intentionally visit more nodes than this initializer traversal.
use super::initialization::Resolved;
use crate::syntax::{BinaryOp, ExprKind, ParsedDocument, UnaryOp, error};
use crate::{Diagnostic, DiagnosticCode};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
enum Value {
    Numeric(f64),
    Boolean(bool),
    Unknown,
}
impl Value {
    fn numeric(self) -> Option<f64> {
        match self {
            Self::Numeric(n) => Some(n),
            Self::Boolean(b) => Some(if b { 1.0 } else { 0.0 }),
            Self::Unknown => None,
        }
    }
    fn truth(self) -> Option<bool> {
        match self {
            Self::Numeric(n) => Some(n != 0.0),
            Self::Boolean(b) => Some(b),
            Self::Unknown => None,
        }
    }
    fn number(n: Option<f64>) -> Self {
        n.map_or(Self::Unknown, Self::Numeric)
    }
}
enum Task {
    Visit(usize),
    Apply(usize),
    PopFrame,
    Logical { id: usize, next: usize },
    Condition { id: usize, branch: usize },
}
pub(super) fn evaluate(
    document: &ParsedDocument,
    root: usize,
    values: &BTreeMap<String, Resolved>,
) -> Result<Option<f64>, Diagnostic> {
    let mut pending = vec![Task::Visit(root)];
    let mut operands = Vec::<Value>::new();
    let mut frames = Vec::<BTreeMap<String, Value>>::new();
    while let Some(task) = pending.pop() {
        let id = match task {
            Task::PopFrame => {
                frames.pop();
                continue;
            }
            Task::Visit(id) => {
                match &document.expressions[id].kind {
                    ExprKind::Logical { op, args, .. } => {
                        if args.is_empty() {
                            operands.push(Value::Boolean(op.identity()));
                        } else {
                            pending.push(Task::Logical { id, next: 1 });
                            pending.push(Task::Visit(args[0]));
                        }
                    }
                    ExprKind::Piecewise {
                        branches,
                        otherwise,
                    } => {
                        if let Some((_, condition)) = branches.first() {
                            pending.push(Task::Condition { id, branch: 0 });
                            pending.push(Task::Visit(*condition));
                        } else if let Some(otherwise) = otherwise {
                            pending.push(Task::Visit(*otherwise));
                        } else {
                            unreachable!("validated piecewise arity");
                        }
                    }
                    kind => {
                        pending.push(Task::Apply(id));
                        pending.extend(kind.children().rev().map(Task::Visit));
                    }
                }
                continue;
            }
            Task::Logical { id, next } => {
                let ExprKind::Logical { op, args, .. } = &document.expressions[id].kind else {
                    unreachable!()
                };
                match operands.pop().expect("logical operand").truth() {
                    None => operands.push(Value::Unknown),
                    Some(value) if value != op.identity() => operands.push(Value::Boolean(value)),
                    Some(_) if next == args.len() => operands.push(Value::Boolean(op.identity())),
                    Some(_) => {
                        pending.push(Task::Logical { id, next: next + 1 });
                        pending.push(Task::Visit(args[next]));
                    }
                }
                continue;
            }
            Task::Condition { id, branch } => {
                let ExprKind::Piecewise {
                    branches,
                    otherwise,
                } = &document.expressions[id].kind
                else {
                    unreachable!()
                };
                match operands.pop().expect("piecewise condition").truth() {
                    None => operands.push(Value::Unknown),
                    Some(true) => pending.push(Task::Visit(branches[branch].0)),
                    Some(false) if branch + 1 < branches.len() => {
                        pending.push(Task::Condition {
                            id,
                            branch: branch + 1,
                        });
                        pending.push(Task::Visit(branches[branch + 1].1));
                    }
                    Some(false) => {
                        if let Some(otherwise) = otherwise {
                            pending.push(Task::Visit(*otherwise));
                        } else {
                            return Err(error(
                                DiagnosticCode::DeclarationConflict,
                                document.expressions[id].span,
                                "known piecewise selection has no matching branch or otherwise in the finite initial domain",
                            ));
                        }
                    }
                }
                continue;
            }
            Task::Apply(id) => id,
        };
        let node = &document.expressions[id];
        let value = match &node.kind {
            ExprKind::Boolean(b) => Value::Boolean(*b),
            ExprKind::Number { value, .. } => Value::Numeric(*value),
            ExprKind::Rational {
                numerator,
                denominator,
            } => Value::Numeric(*numerator as f64 / *denominator as f64),
            ExprKind::Constant(constant) => Value::Numeric(constant.value()),
            ExprKind::Time => Value::Unknown,
            ExprKind::Symbol(name) => {
                if let Some(frame) = frames.last() {
                    frame[&name.text]
                } else {
                    Value::number(values[&name.text].value)
                }
            }
            ExprKind::Not { .. } => operands
                .pop()
                .expect("not operand")
                .truth()
                .map_or(Value::Unknown, |v| Value::Boolean(!v)),
            ExprKind::Call { function, args } => {
                let start = operands.len() - args.len();
                let numbers = operands[start..]
                    .iter()
                    .map(|v| v.numeric())
                    .collect::<Vec<_>>();
                let value = function.evaluate(&numbers).map_err(|message| {
                    error(DiagnosticCode::DeclarationConflict, node.span, message)
                })?;
                operands.truncate(start);
                Value::number(value)
            }
            ExprKind::UserCall { name, args } => {
                let start = operands.len() - args.len();
                let func = document
                    .functions
                    .iter()
                    .find(|f| f.name.text == name.text)
                    .expect("validated user function");
                let mut frame = BTreeMap::new();
                for (param, val) in func.parameters.iter().zip(&operands[start..]) {
                    frame.insert(param.text.clone(), *val);
                }
                operands.truncate(start);
                frames.push(frame);
                pending.push(Task::PopFrame);
                pending.push(Task::Visit(func.body));
                continue;
            }
            ExprKind::Compare { op, args, .. } => {
                let start = operands.len() - args.len();
                let values = &operands[start..];
                let value = if values.iter().any(|v| v.numeric().is_none()) {
                    Value::Unknown
                } else {
                    Value::Boolean(
                        values
                            .windows(2)
                            .all(|p| op.compare(p[0].numeric().unwrap(), p[1].numeric().unwrap())),
                    )
                };
                operands.truncate(start);
                value
            }
            ExprKind::Unary { op, .. } => {
                let value = operands.pop().expect("unary operand").numeric();
                Value::number(value.map(|v| if *op == UnaryOp::Minus { -v } else { v }))
            }
            ExprKind::Binary { op, .. } => {
                let right = operands.pop().expect("right operand").numeric();
                let left = operands.pop().expect("left operand").numeric();
                if *op == BinaryOp::Divide && right == Some(0.0) {
                    return Err(error(
                        DiagnosticCode::DeclarationConflict,
                        node.span,
                        "initialization division by zero is outside the finite real domain",
                    ));
                }
                Value::number(if let (Some(left), Some(right)) = (left, right) {
                    Some(match op {
                        BinaryOp::Add => left + right,
                        BinaryOp::Subtract => left - right,
                        BinaryOp::Multiply => left * right,
                        BinaryOp::Divide => left / right,
                        BinaryOp::Power
                            if (left < 0.0 && right.fract() != 0.0)
                                || (left == 0.0 && right <= 0.0) =>
                        {
                            return Err(error(
                                DiagnosticCode::DeclarationConflict,
                                node.span,
                                "initialization power is outside the supported real domain",
                            ));
                        }
                        BinaryOp::Power => left.powf(right),
                    })
                } else {
                    None
                })
            }
            ExprKind::Logical { .. } | ExprKind::Piecewise { .. } => {
                unreachable!("demand continuations handle conditional evaluation")
            }
        };
        if value.numeric().is_some_and(|v| !v.is_finite()) {
            return Err(error(
                DiagnosticCode::DeclarationConflict,
                node.span,
                "initialization expression must have finite real intermediate and final values",
            ));
        }
        // Preserve Boolean identity internally, while canonicalizing numeric signed zero.
        operands.push(match value {
            Value::Numeric(0.0) => Value::Numeric(0.0),
            _ => value,
        });
    }
    debug_assert_eq!(operands.len(), 1);
    Ok(operands.pop().expect("root value").numeric())
}
