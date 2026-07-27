//! Debug metadata discovery shared by native compilation and debugger tools.

use ori_hir::{HirBlock, HirExpr, HirFunc, HirModule, HirStmt};
use ori_types::Ty;

use super::project::{namespace_of, LoadedSource};

pub(super) fn collect_native_debug_functions(
    hir: &HirModule,
    loaded: &[LoadedSource],
) -> Vec<ori_codegen::DebugFunction> {
    let mut functions = Vec::new();
    for function in &hir.funcs {
        let source = loaded
            .iter()
            .filter(|source| {
                let namespace = namespace_of(&source.ast);
                function.name == namespace || function.name.starts_with(&format!("{namespace}."))
            })
            .find(|source| function.span.start as usize <= source.source.len())
            .or_else(|| loaded.first());
        let Some(source) = source else { continue };
        let line = source_line(&source.source, function.span.start);
        let variables = collect_debug_variables(function, &source.source);
        functions.push(ori_codegen::DebugFunction {
            name: function.name.to_string(),
            source: source.path.clone(),
            line,
            variables,
        });
    }
    functions
}

fn collect_debug_variables(function: &HirFunc, source: &str) -> Vec<ori_codegen::DebugVariable> {
    let mut variables = Vec::new();
    for parameter in &function.params {
        variables.push(ori_codegen::DebugVariable {
            name: parameter.name.to_string(),
            ty: parameter.ty.display(),
            line: source_line(source, parameter.span.start),
        });
    }
    for capture in &function.closure_captures {
        variables.push(ori_codegen::DebugVariable {
            name: format!("capture.{}", capture.name),
            ty: capture.ty.display(),
            line: source_line(source, function.span.start),
        });
    }
    collect_debug_block_variables(&function.body, source, &mut variables);
    variables.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then_with(|| left.name.cmp(&right.name))
    });
    variables.dedup_by(|left, right| left.name == right.name && left.line == right.line);
    variables
}

fn collect_debug_block_variables(
    block: &HirBlock,
    source: &str,
    variables: &mut Vec<ori_codegen::DebugVariable>,
) {
    for statement in &block.stmts {
        match statement {
            HirStmt::Let { name, ty, span, .. } | HirStmt::Using { name, ty, span, .. } => {
                variables.push(ori_codegen::DebugVariable {
                    name: name.to_string(),
                    ty: ty.display(),
                    line: source_line(source, span.start),
                });
            }
            HirStmt::If {
                then,
                else_ifs,
                else_,
                ..
            } => {
                collect_debug_block_variables(then, source, variables);
                for (_, branch) in else_ifs {
                    collect_debug_block_variables(branch, source, variables);
                }
                if let Some(branch) = else_ {
                    collect_debug_block_variables(branch, source, variables);
                }
            }
            HirStmt::While { body, .. }
            | HirStmt::Loop { body, .. }
            | HirStmt::Repeat { body, .. }
            | HirStmt::WhileSome { body, .. } => {
                collect_debug_block_variables(body, source, variables);
            }
            HirStmt::For {
                binding,
                index_binding,
                elem_ty,
                body,
                span,
                ..
            } => {
                variables.push(ori_codegen::DebugVariable {
                    name: binding.to_string(),
                    ty: elem_ty.display(),
                    line: source_line(source, span.start),
                });
                if let Some(index) = index_binding {
                    variables.push(ori_codegen::DebugVariable {
                        name: index.to_string(),
                        ty: Ty::Int.display(),
                        line: source_line(source, span.start),
                    });
                }
                collect_debug_block_variables(body, source, variables);
            }
            HirStmt::IfSome {
                binding,
                inner_ty,
                then,
                else_,
                ..
            } => {
                variables.push(ori_codegen::DebugVariable {
                    name: binding.to_string(),
                    ty: inner_ty.display(),
                    line: source_line(source, statement_span(statement)),
                });
                collect_debug_block_variables(then, source, variables);
                if let Some(branch) = else_ {
                    collect_debug_block_variables(branch, source, variables);
                }
            }
            HirStmt::Match { arms, .. } => {
                for arm in arms {
                    collect_debug_pattern_variables(
                        &arm.pattern,
                        source,
                        arm.span.start,
                        variables,
                    );
                    for statement in &arm.body {
                        let block = HirBlock {
                            stmts: vec![statement.clone()],
                            span: arm.span,
                        };
                        collect_debug_block_variables(&block, source, variables);
                    }
                }
            }
            HirStmt::Assign { .. }
            | HirStmt::Return(_, _)
            | HirStmt::Break(_)
            | HirStmt::Continue(_)
            | HirStmt::Expr(_)
            | HirStmt::Check { .. } => {}
        }
    }
}

fn collect_debug_pattern_variables(
    pattern: &ori_hir::hir::HirPattern,
    source: &str,
    offset: u32,
    variables: &mut Vec<ori_codegen::DebugVariable>,
) {
    match pattern {
        ori_hir::hir::HirPattern::Binding(name, ty) => {
            variables.push(ori_codegen::DebugVariable {
                name: name.to_string(),
                ty: ty.display(),
                line: source_line(source, offset),
            });
        }
        ori_hir::hir::HirPattern::Some_(inner)
        | ori_hir::hir::HirPattern::Ok_(inner)
        | ori_hir::hir::HirPattern::Err_(inner) => {
            collect_debug_pattern_variables(inner, source, offset, variables);
        }
        ori_hir::hir::HirPattern::Variant { fields, .. } => {
            for (_, field) in fields {
                collect_debug_pattern_variables(field, source, offset, variables);
            }
        }
        ori_hir::hir::HirPattern::Tuple(items) | ori_hir::hir::HirPattern::Or(items) => {
            for item in items {
                collect_debug_pattern_variables(item, source, offset, variables);
            }
        }
        ori_hir::hir::HirPattern::Wildcard
        | ori_hir::hir::HirPattern::BoolLit(_)
        | ori_hir::hir::HirPattern::IntLit(_)
        | ori_hir::hir::HirPattern::StrLit(_)
        | ori_hir::hir::HirPattern::None_ => {}
    }
}

fn statement_span(statement: &HirStmt) -> u32 {
    match statement {
        HirStmt::Let { span, .. }
        | HirStmt::Assign { span, .. }
        | HirStmt::Expr(HirExpr { span, .. })
        | HirStmt::If { span, .. }
        | HirStmt::While { span, .. }
        | HirStmt::For { span, .. }
        | HirStmt::Loop { span, .. }
        | HirStmt::Repeat { span, .. }
        | HirStmt::Match { span, .. }
        | HirStmt::IfSome { span, .. }
        | HirStmt::WhileSome { span, .. }
        | HirStmt::Using { span, .. }
        | HirStmt::Check { span, .. } => span.start,
        HirStmt::Return(_, span) | HirStmt::Break(span) | HirStmt::Continue(span) => span.start,
    }
}

fn source_line(source: &str, offset: u32) -> u64 {
    let offset = (offset as usize).min(source.len());
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count() as u64
        + 1
}
