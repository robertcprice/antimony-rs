//! Reference arithmetic is constructed as n-ary +/* groups before normalization.
//!
//! Construction appends to a left group but retains a right group. Only a final
//! two-operand group with a matching right group is flattened. Thus a+(b+c)
//! becomes (a+b)+c, but (a+(b+c))+d retains b+c. Flattening each binary subtree
//! too early would change both the reference tree and floating-point evaluation.
//! Source syntax stays untouched for formatting; every semantic consumer uses
//! this bounded normalized arena. No recursion or subtree duplication is needed.
use crate::syntax::{
    BinaryOp, Expr, ExprKind, InitialValue, ParsedDocument, Statement, UnaryOp, error,
};
use crate::{Diagnostic, DiagnosticCode, LimitKind, Span};

enum Shape {
    Atom(ExprKind),
    Conditional(ExprKind),
    Call(crate::Builtin, Vec<usize>),
    UserCall(crate::syntax::Name, Vec<usize>),
    Minus(usize),
    Group(BinaryOp, Vec<usize>),
}
impl Shape {
    fn children(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        let mut first = None;
        let mut args: &[usize] = &[];
        let mut conditional = None;
        match self {
            Self::Minus(child) => first = Some(*child),
            Self::Call(_, a) | Self::UserCall(_, a) | Self::Group(_, a) => args = a,
            Self::Conditional(kind) => conditional = Some(kind),
            Self::Atom(_) => (),
        }
        first
            .into_iter()
            .chain(args.iter().copied())
            .chain(conditional.into_iter().flat_map(ExprKind::children))
    }
}
fn chain_left(nodes: &[Node], id: usize) -> bool {
    match &nodes[id].shape {
        Shape::Conditional(ExprKind::Compare { .. }) => true,
        Shape::Conditional(ExprKind::Logical {
            op: crate::LogicalOp::And,
            args,
            ..
        }) => {
            args.len() >= 2
                && args.iter().all(|id| {
                    matches!(
                        nodes[*id].shape,
                        Shape::Conditional(ExprKind::Compare { .. })
                    )
                })
        }
        _ => false,
    }
}
struct Node {
    shape: Shape,
    span: Span,
}

pub(super) fn normalize(source: &ParsedDocument) -> Result<ParsedDocument, Diagnostic> {
    let mut nodes: Vec<Node> = Vec::with_capacity(source.expressions.len());
    let mut aliases: Vec<usize> = Vec::with_capacity(source.expressions.len());
    for original in &source.expressions {
        let shape = match &original.kind {
            ExprKind::Unary { op, child } => {
                let child = aliases[*child];
                if *op == UnaryOp::Plus {
                    aliases.push(child);
                    continue;
                }
                if let Shape::Minus(grandchild) = nodes[child].shape {
                    aliases.push(grandchild);
                    continue;
                }
                if let Shape::Atom(ExprKind::Rational {
                    numerator,
                    denominator,
                }) = &nodes[child].shape
                {
                    // Keep wide transient values: another outer minus can bring
                    // the final retained atom back inside the reference range.
                    Shape::Atom(ExprKind::Rational {
                        numerator: -*numerator,
                        denominator: *denominator,
                    })
                } else {
                    Shape::Minus(child)
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left = aliases[*left];
                let right = aliases[*right];
                let mut children = if associative(*op) {
                    match &mut nodes[left].shape {
                        Shape::Group(left_op, children) if left_op == op => {
                            std::mem::take(children)
                        }
                        _ => vec![left],
                    }
                } else {
                    vec![left]
                };
                children.push(right);
                Shape::Group(*op, children)
            }
            ExprKind::Not { .. }
            | ExprKind::Compare { .. }
            | ExprKind::Logical { .. }
            | ExprKind::Piecewise { .. } => {
                let mut kind = original.kind.map_children(|id| aliases[id]);
                match &mut kind {
                    ExprKind::Compare { args, infix, .. } => {
                        if *infix && chain_left(&nodes, args[0]) {
                            return Err(error(
                                DiagnosticCode::UnsupportedFeature,
                                original.span,
                                "infix comparison-chain rewriting is deferred; use explicit relational functions",
                            ));
                        }
                        *infix = false;
                    }
                    ExprKind::Logical { op, args, infix } => {
                        if *infix {
                            let left = args[0];
                            let right = args[1];
                            if let Shape::Conditional(ExprKind::Logical {
                                op: left_op,
                                args: children,
                                ..
                            }) = &mut nodes[left].shape
                                && *left_op == *op
                            {
                                // Consume obsolete vectors: never retain a growing copy
                                // at every node in a wide-function-plus-infix chain.
                                *args = std::mem::take(children);
                                args.push(right);
                            }
                        }
                        *infix = false;
                    }
                    _ => (),
                }
                Shape::Conditional(kind)
            }
            ExprKind::Call { function, args } => {
                Shape::Call(*function, args.iter().map(|id| aliases[*id]).collect())
            }
            ExprKind::UserCall { name, args } => {
                Shape::UserCall(name.clone(), args.iter().map(|id| aliases[*id]).collect())
            }
            atom => Shape::Atom(atom.clone()),
        };
        aliases.push(nodes.len());
        nodes.push(Node {
            shape,
            span: original.span,
        });
    }
    let mut document = source.clone();
    let mut arena = Vec::<Expr>::with_capacity(nodes.len());
    let mut lowered = vec![usize::MAX; nodes.len()];
    let mut roots = Vec::<&mut usize>::new();
    for func in &mut document.functions {
        roots.push(&mut func.body);
    }
    for statement in &mut document.statements {
        match statement {
            Statement::Reaction { rate, .. }
            | Statement::Rule {
                expression: rate, ..
            } => roots.push(rate),
            Statement::Initial {
                value: InitialValue::Expression(root),
                ..
            }
            | Statement::Compartment {
                value: Some(InitialValue::Expression(root)),
                ..
            }
            | Statement::Parameter {
                value: Some(InitialValue::Expression(root)),
                ..
            } => roots.push(root),
            Statement::Event {
                trigger,
                delay,
                priority,
                assignments,
                ..
            } => {
                roots.push(trigger);
                if let Some(delay) = delay {
                    roots.push(delay);
                }
                if let Some(priority) = priority {
                    roots.push(priority);
                }
                for assignment in assignments {
                    roots.push(&mut assignment.expression);
                }
            }
            _ => continue,
        }
    }
    for root in roots {
        let root_node = aliases[*root];
        // First normalize whole constructed groups, after their children. A
        // three-operand group must not normalize its first two operands as a
        // standalone binary expression: that group never existed upstream.
        let mut pending = vec![(root_node, false)];
        while let Some((id, visited)) = pending.pop() {
            if !visited {
                pending.push((id, true));
                pending.extend(nodes[id].shape.children().rev().map(|id| (id, false)));
                continue;
            }
            let pair = match &nodes[id].shape {
                Shape::Group(op, children) if associative(*op) && children.len() == 2 => {
                    Some((*op, children[0], children[1]))
                }
                _ => None,
            };
            if let Some((op, left, right)) = pair
                && let Shape::Group(right_op, children) = &mut nodes[right].shape
                && op == *right_op
            {
                let mut merged = Vec::with_capacity(children.len() + 1);
                merged.push(left);
                merged.append(children);
                nodes[id].shape = Shape::Group(op, merged);
            }
        }
        // Lower retained n-ary groups into ordered left folds. The result has
        // postordered indices for dimension analysis and shared evaluator/export
        // meaning. Check resulting depth as well as source parser depth.
        let mut pending = vec![(root_node, false)];
        while let Some((id, visited)) = pending.pop() {
            if !visited {
                pending.push((id, true));
                pending.extend(nodes[id].shape.children().rev().map(|id| (id, false)));
                continue;
            }
            let span = nodes[id].span;
            lowered[id] = match &nodes[id].shape {
                Shape::Atom(kind) => {
                    if let ExprKind::Rational {
                        numerator,
                        denominator,
                    } = kind
                    {
                        if i32::try_from(*numerator).is_err()
                            || i32::try_from(*denominator).is_err()
                        {
                            return Err(error(
                                DiagnosticCode::Syntax,
                                span,
                                "normalized rational components must fit the reference signed 32-bit range",
                            ));
                        }
                        if *denominator == 0 {
                            return Err(error(
                                DiagnosticCode::UnsupportedFeature,
                                span,
                                "zero-denominator rational literals serialize as nonfinite constants outside this slice",
                            ));
                        }
                    }
                    push(source, &mut arena, kind.clone(), span, 1)?
                }
                Shape::Conditional(kind) => {
                    let kind = kind.map_children(|id| lowered[id]);
                    let depth = kind.children().map(|id| arena[id].depth).max().unwrap_or(0) + 1;
                    push(source, &mut arena, kind, span, depth)?
                }
                Shape::Call(function, args) => {
                    let args = args.iter().map(|id| lowered[*id]).collect::<Vec<_>>();
                    let depth = args.iter().map(|id| arena[*id].depth).max().unwrap_or(0) + 1;
                    push(
                        source,
                        &mut arena,
                        ExprKind::Call {
                            function: *function,
                            args,
                        },
                        span,
                        depth,
                    )?
                }
                Shape::UserCall(name, args) => {
                    let args = args.iter().map(|id| lowered[*id]).collect::<Vec<_>>();
                    let depth = args.iter().map(|id| arena[*id].depth).max().unwrap_or(0) + 1;
                    push(
                        source,
                        &mut arena,
                        ExprKind::UserCall {
                            name: name.clone(),
                            args,
                        },
                        span,
                        depth,
                    )?
                }
                Shape::Minus(child) => {
                    let child = lowered[*child];
                    let depth = arena[child].depth + 1;
                    push(
                        source,
                        &mut arena,
                        ExprKind::Unary {
                            op: UnaryOp::Minus,
                            child,
                        },
                        span,
                        depth,
                    )?
                }
                Shape::Group(op, children) => {
                    let mut left = lowered[children[0]];
                    for right in children[1..].iter().map(|id| lowered[*id]) {
                        let depth = arena[left].depth.max(arena[right].depth) + 1;
                        left = push(
                            source,
                            &mut arena,
                            ExprKind::Binary {
                                op: *op,
                                left,
                                right,
                            },
                            span,
                            depth,
                        )?;
                    }
                    left
                }
            };
        }
        *root = lowered[root_node];
    }
    debug_assert!(arena.len() <= source.expressions.len());
    document.expressions = arena;
    Ok(document)
}
fn associative(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Add | BinaryOp::Multiply)
}
fn push(
    document: &ParsedDocument,
    arena: &mut Vec<Expr>,
    kind: ExprKind,
    span: Span,
    depth: usize,
) -> Result<usize, Diagnostic> {
    document
        .limits
        .check(LimitKind::ExpressionDepth, depth)
        .map_err(|e| error(DiagnosticCode::ResourceLimit, span, e.to_string()))?;
    if depth > 256 {
        return Err(error(
            DiagnosticCode::ResourceLimit,
            span,
            "normalized expression exceeds implementation hard depth ceiling 256",
        ));
    }
    if arena.len() >= document.expressions.len() {
        return Err(error(
            DiagnosticCode::ResourceLimit,
            span,
            "normalized node count exceeds admitted source nodes",
        ));
    }
    let id = arena.len();
    arena.push(Expr { kind, span, depth });
    Ok(id)
}
