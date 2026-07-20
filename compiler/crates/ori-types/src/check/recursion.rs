//! Reject a function that can only ever call itself.
//!
//! Detecting non-termination in general is the halting problem, so this does
//! **not** try. It catches one shape, the one beginners actually write:
//!
//! ```text
//! forever(n: int) -> int
//!     return forever(n + 1)     -- no path returns without recursing
//! end
//! ```
//!
//! The rule is "every path out of the body goes through a call to this same
//! function". When that holds the function cannot terminate, whatever the
//! arguments are, so it is an error rather than a warning — no correct program
//! has this shape.
//!
//! Everything here is deliberately **conservative**: unsure means silent. A
//! false positive would reject a working program, which is far worse than
//! missing a bug the runtime stack guard already reports.
//!
//! Not detected on purpose:
//! - a wrong base case (`f(n - 1)` that never reaches it) — needs value analysis;
//! - mutual recursion (`a` → `b` → `a`) — needs a call graph;
//! - recursion behind a condition the compiler cannot evaluate.

use ori_ast::expr::Expr;
use ori_ast::stmt::{Block, MatchCase, Stmt};
use smol_str::SmolStr;

/// True when every path through `body` reaches a call to `name` before it can
/// return or fall off the end.
pub(super) fn always_recurses(body: &Block, name: &SmolStr) -> bool {
    stmts_always_recurse(&body.stmts, name)
}

fn stmts_always_recurse(stmts: &[Stmt], name: &SmolStr) -> bool {
    for stmt in stmts {
        // Statements run in order: the first one that always recurses decides,
        // and anything that can leave first means there is an escape.
        if stmt_always_recurses(stmt, name) {
            return true;
        }
        if stmt_can_exit(stmt) {
            return false;
        }
    }
    false
}

fn stmt_always_recurses(stmt: &Stmt, name: &SmolStr) -> bool {
    match stmt {
        Stmt::Return(ret) => ret
            .value
            .as_ref()
            .is_some_and(|value| expr_always_calls(value, name)),
        Stmt::Expr(expr) => expr_always_calls(expr, name),
        Stmt::Const(c) => expr_always_calls(&c.value, name),
        Stmt::Var(v) => expr_always_calls(&v.value, name),
        Stmt::Assign(a) => expr_always_calls(&a.value, name),

        // Every branch must recurse, and the chain must be closed by `else` —
        // an open `if` can simply fall through.
        Stmt::If(if_stmt) => {
            let Some(else_block) = &if_stmt.else_block else {
                return false;
            };
            expr_always_calls(&if_stmt.condition, name)
                || (stmts_always_recurse(&if_stmt.then_block.stmts, name)
                    && if_stmt
                        .else_ifs
                        .iter()
                        .all(|(_, block)| stmts_always_recurse(&block.stmts, name))
                    && stmts_always_recurse(&else_block.stmts, name))
        }

        // Same idea for `match`: only conclusive with a `case else`, since
        // exhaustiveness of the other arms is checked elsewhere and a guard can
        // fall through.
        Stmt::Match(match_stmt) => {
            if expr_always_calls(&match_stmt.scrutinee, name) {
                return true;
            }
            let has_else = match_stmt
                .cases
                .iter()
                .any(|case| matches!(case, MatchCase::Else { .. }));
            if !has_else {
                return false;
            }
            match_stmt.cases.iter().all(|case| match case {
                // A guarded arm may not be taken at all.
                MatchCase::Pattern {
                    guard: Some(_), ..
                } => false,
                MatchCase::Pattern { body, .. } | MatchCase::Else { body, .. } => {
                    stmts_always_recurse(body, name)
                }
            })
        }

        // Loops may run zero times; `using`/`check` may recurse in their value.
        Stmt::Using(u) => expr_always_calls(&u.value, name),
        Stmt::Check(c) => expr_always_calls(&c.condition, name),
        _ => false,
    }
}

/// True when a statement can transfer control out of the function body.
///
/// This has to look **inside** nested blocks, not just at the top level. An
/// early guard such as
///
/// ```text
/// if n <= 0
///     return 0
/// end
/// return 1 + countdown(n - 1)
/// ```
///
/// leaves through the `if`, so the function is not unconditionally recursive
/// even though the last statement always recurses. Treating the `if` as
/// non-exiting produced exactly that false positive.
fn stmt_can_exit(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If(if_stmt) => {
            if_stmt.then_block.stmts.iter().any(stmt_can_exit)
                || if_stmt
                    .else_ifs
                    .iter()
                    .any(|(_, block)| block.stmts.iter().any(stmt_can_exit))
                || if_stmt
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block.stmts.iter().any(stmt_can_exit))
        }
        Stmt::Match(match_stmt) => match_stmt.cases.iter().any(|case| match case {
            MatchCase::Pattern { body, .. } | MatchCase::Else { body, .. } => {
                body.iter().any(stmt_can_exit)
            }
        }),
        Stmt::IfSome(if_some) => {
            if_some.then_block.stmts.iter().any(stmt_can_exit)
                || if_some
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block.stmts.iter().any(stmt_can_exit))
        }
        Stmt::While(w) => w.body.stmts.iter().any(stmt_can_exit),
        Stmt::WhileSome(w) => w.body.stmts.iter().any(stmt_can_exit),
        Stmt::For(f) => f.body.stmts.iter().any(stmt_can_exit),
        Stmt::Repeat(r) => r.body.stmts.iter().any(stmt_can_exit),
        Stmt::Loop(l) => l.body.stmts.iter().any(stmt_can_exit),
        _ => false,
    }
}

/// True when evaluating `expr` always reaches a call to `name`.
///
/// Only positions that are *unconditionally evaluated* count. Short-circuit
/// operands, closure bodies, and single branches of a conditional do not.
fn expr_always_calls(expr: &Expr, name: &SmolStr) -> bool {
    match expr {
        Expr::Call { callee, args, .. } => {
            if is_self_reference(callee, name) {
                return true;
            }
            // Arguments are evaluated before the call itself.
            args.iter().any(|arg| match &arg.value {
                ori_ast::expr::ArgValue::Expr(e) => expr_always_calls(e, name),
                _ => false,
            }) || expr_always_calls(callee, name)
        }

        // Both operands of an arithmetic/comparison operator are evaluated.
        // `and` / `or` short-circuit, so the right side does not count.
        Expr::Binary { op, lhs, rhs, .. } => {
            use ori_ast::expr::BinaryOp;
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                expr_always_calls(lhs, name)
            } else {
                expr_always_calls(lhs, name) || expr_always_calls(rhs, name)
            }
        }
        Expr::Unary { operand, .. } => expr_always_calls(operand, name),
        Expr::Field { object, .. } => expr_always_calls(object, name),
        Expr::TupleIndex { object, .. } => expr_always_calls(object, name),
        Expr::Index { object, index, .. } => {
            use ori_ast::expr::IndexExpr;
            let index_recurses = match index {
                IndexExpr::Single(i) => expr_always_calls(i, name),
                IndexExpr::Range { start, end } => {
                    start.as_ref().is_some_and(|e| expr_always_calls(e, name))
                        || end.as_ref().is_some_and(|e| expr_always_calls(e, name))
                }
            };
            expr_always_calls(object, name) || index_recurses
        }
        Expr::Pipe { value, func, .. } => {
            expr_always_calls(value, name) || is_self_reference(func, name)
        }

        // Only one branch runs, so both must recurse for the whole to.
        Expr::IfExpr {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            expr_always_calls(condition, name)
                || (expr_always_calls(then_expr, name) && expr_always_calls(else_expr, name))
        }

        // A closure body runs only when the closure is called.
        Expr::Closure(_) => false,

        _ => false,
    }
}

/// True when `callee` names the function being checked.
///
/// A bare call parses as a single-segment `QualifiedIdent`, not `Ident`, so
/// both are accepted. Multi-segment paths (`other.thing`) and method calls are
/// deliberately ignored: they may resolve elsewhere, and this analysis does not
/// track receivers.
fn is_self_reference(callee: &Expr, name: &SmolStr) -> bool {
    match callee {
        Expr::Ident(ident) => &ident.text == name,
        Expr::QualifiedIdent(path) => {
            path.parts.len() == 1 && &path.parts[0].text == name
        }
        _ => false,
    }
}
