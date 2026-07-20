use crate::def::{DefId, DefKind, DefMap};
use crate::lower::lower_type_with_aliases;
use crate::ty::{expand_ty_aliases, Ty};
use ori_ast::common::{AttrArg, WhereClause, WhereConstraint};
use ori_ast::item::{ImportDecl, Item, ItemWithAttrs, Param, ParamKind, SourceFile};
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label};
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};

/// Resolved type information for a single function (signature only for now).
#[derive(Debug, Clone)]
pub struct FuncSig {
    pub def_id: DefId,
    pub param_names: Vec<SmolStr>,
    pub params: Vec<Ty>,
    pub param_defaults: Vec<bool>,
    pub param_variadic: Vec<bool>,
    pub where_constraints: Vec<WhereConstraintSig>,
    pub return_ty: Ty,
    pub is_mut: bool,
}

#[derive(Debug, Clone)]
pub struct ValueSig {
    pub def_id: DefId,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct DeprecatedSig {
    pub def_id: DefId,
    pub message: SmolStr,
}

#[derive(Debug, Clone)]
pub struct StructSig {
    pub def_id: DefId,
    pub fields: Vec<(SmolStr, Ty)>,
}

#[derive(Debug, Clone)]
pub struct EnumSig {
    pub def_id: DefId,
    pub variants: Vec<EnumVariantSig>,
}

#[derive(Debug, Clone)]
pub struct EnumVariantSig {
    pub name: SmolStr,
    pub fields: Vec<(SmolStr, Ty)>,
}

#[derive(Debug, Clone)]
pub struct WhereConstraintSig {
    pub param_index: u32,
    pub param_name: SmolStr,
    pub trait_def_id: DefId,
    pub negative: bool,
}

#[derive(Debug, Clone)]
pub struct TraitMethodSig {
    pub name: SmolStr,
    pub params: Vec<Ty>,
    pub return_ty: Ty,
    pub is_mut: bool,
    pub has_default: bool,
    pub span: ori_diagnostics::Span,
}

#[derive(Debug, Clone)]
pub struct TraitSig {
    pub def_id: DefId,
    pub methods: Vec<TraitMethodSig>,
}

#[derive(Debug, Clone)]
pub struct ImplMethodSig {
    pub name: SmolStr,
    pub func_def_id: DefId,
}

#[derive(Debug, Clone)]
pub struct ImplSig {
    pub trait_def_id: DefId,
    pub type_def_id: DefId,
    pub methods: Vec<ImplMethodSig>,
}

#[derive(Debug, Clone)]
pub struct ReExport {
    pub namespace: SmolStr,
    pub alias: SmolStr,
    pub target: SmolStr,
}

/// The resolved form of a `type alias Name<T> = ...` declaration.
#[derive(Debug, Clone)]
pub struct TypeAliasSig {
    pub def_id: DefId,
    /// Names of the type parameters in declaration order (e.g. `["T", "U"]`).
    pub type_params: Vec<SmolStr>,
    /// The underlying type with `Ty::Param` placeholders for each type param.
    pub ty: Ty,
}

/// `newtype UserId = int` — the nominal type's representation.
///
/// Unlike a type alias this is **not** substituted while checking: the name
/// stays a distinct `Ty::Named`, which is what makes it nominal. The
/// representation is only applied when lowering to HIR, so the newtype costs
/// nothing at runtime.
#[derive(Debug, Clone)]
pub struct NewtypeSig {
    pub def_id: DefId,
    pub repr: Ty,
}

/// The output of resolving a single source file.
#[derive(Debug)]
pub struct ResolvedModule {
    pub def_map: DefMap,
    pub func_sigs: Vec<FuncSig>,
    pub value_sigs: Vec<ValueSig>,
    pub struct_sigs: Vec<StructSig>,
    pub enum_sigs: Vec<EnumSig>,
    pub trait_sigs: Vec<TraitSig>,
    pub impl_sigs: Vec<ImplSig>,
    pub type_alias_sigs: Vec<TypeAliasSig>,
    pub newtype_sigs: Vec<NewtypeSig>,
    pub deprecated_sigs: Vec<DeprecatedSig>,
    pub reexports: Vec<ReExport>,
    pub namespace: SmolStr,
}

/// Build a `ResolvedModule` from a `SourceFile`.
///
/// Phase 1 — register all top-level definitions.
/// Phase 2 — lower all type annotations to `Ty`.
pub fn resolve(file: &SourceFile, file_id: FileId, sink: &mut DiagnosticSink) -> ResolvedModule {
    let namespace = SmolStr::new(file.namespace.name.to_string());
    resolve_many(&[(file, file_id)], namespace, sink)
}

pub fn resolve_many<S: Into<SmolStr>>(
    files: &[(&SourceFile, FileId)],
    entry_namespace: S,
    sink: &mut DiagnosticSink,
) -> ResolvedModule {
    let mut def_map = DefMap::default();
    let mut implemented_pairs = HashMap::new();
    let core_traits = register_core_traits(&mut def_map);
    let stdlib_error_def_id = register_stdlib_error_type(&mut def_map);
    let stdlib_json_value_def_id = register_stdlib_json_value_enum(&mut def_map);

    // ── Phase 1: register definitions ────────────────────────────────────────
    for (file, file_id) in files {
        let namespace = SmolStr::new(file.namespace.name.to_string());
        for item in &file.items {
            register_item(
                &item.item,
                &namespace,
                &mut def_map,
                &mut implemented_pairs,
                *file_id,
                sink,
            );
        }
    }

    // ── Phase 1b: free binds (`slot = freeFn` on apply) after all funcs exist ─
    // Aliases `namespace.Type.slot` → free function DefId so inherent method
    // lookup and HIR call the bound function.
    for (file, file_id) in files {
        let namespace = SmolStr::new(file.namespace.name.to_string());
        for item in &file.items {
            if let Item::Apply(apply) = &item.item {
                register_apply_free_binds(&namespace, apply, &mut def_map, *file_id, sink);
            }
        }
    }

    // ── Phase 2: lower function signatures ───────────────────────────────────
    let reexports = collect_reexports(files);

    let mut func_sigs = Vec::new();
    let mut value_sigs = Vec::new();
    let mut struct_sigs = vec![builtin_stdlib_error_struct_sig(stdlib_error_def_id)];
    let mut enum_sigs = vec![builtin_stdlib_json_value_enum_sig(stdlib_json_value_def_id)];
    let mut trait_sigs = builtin_core_trait_sigs(&core_traits);
    let mut impl_sigs = Vec::new();
    let mut type_alias_sigs = Vec::new();
    let mut newtype_sigs = Vec::new();
    let deprecated_sigs = collect_deprecated_sigs(files, &def_map);
    for (file, file_id) in files {
        let namespace = SmolStr::new(file.namespace.name.to_string());
        let aliases = import_aliases(file, &reexports);
        for item in &file.items {
            match &item.item {
                Item::Struct(s) => {
                    let path = format!("{}.{}", namespace, s.name.text);
                    if let Some(def_id) = def_map.lookup(&path) {
                        let tp: Vec<SmolStr> =
                            s.type_params.iter().map(|p| p.name.text.clone()).collect();
                        let mut seen_fields: HashSet<SmolStr> = HashSet::new();
                        let fields: Vec<(SmolStr, Ty)> = s
                            .fields
                            .iter()
                            .filter_map(|f| {
                                let ty = lower_type_with_aliases(
                                    &f.ty, &namespace, &tp, &def_map, *file_id, sink, &aliases,
                                );
                                let name = f.name.text.clone();
                                if !seen_fields.insert(name.clone()) {
                                    sink.emit(
                                        Diagnostic::error(
                                            "bind.duplicate_field",
                                            format!(
                                                "duplicate field `{}` in struct `{}`",
                                                name, s.name.text
                                            ),
                                        )
                                        .with_label(Label::primary(
                                            *file_id,
                                            f.name.span,
                                            "duplicate field name",
                                        ))
                                        .with_action(
                                            "rename or remove one of the duplicate fields",
                                        ),
                                    );
                                    // Still collect the field so lowering doesn't panic on missing
                                    // fields, but the error has been emitted.
                                }
                                Some((name, ty))
                            })
                            .collect();
                        struct_sigs.push(StructSig { def_id, fields });

                        for m in &s.methods {
                            let mut all_tp = tp.clone();
                            all_tp.extend(m.type_params.iter().map(|p| p.name.text.clone()));
                            let mut m_aliases = aliases.clone();
                            m_aliases.insert(SmolStr::new("Self"), s.name.text.clone());
                            let mut params: Vec<Ty> = m
                                .params
                                .iter()
                                .map(|p| {
                                    lower_type_with_aliases(
                                        &p.ty, &namespace, &all_tp, &def_map, *file_id, sink,
                                        &m_aliases,
                                    )
                                })
                                .collect();
                            if !has_explicit_self_param(&m.params) {
                                params.insert(0, Ty::Named(def_id, Vec::new()));
                            }
                            let return_ty = m
                                .return_ty
                                .as_ref()
                                .map(|t| {
                                    lower_type_with_aliases(
                                        t, &namespace, &all_tp, &def_map, *file_id, sink,
                                        &m_aliases,
                                    )
                                })
                                .unwrap_or(Ty::Void);
                            let return_ty = async_return_ty(m.is_async, return_ty);
                            let m_path = format!("{}.{}.{}", namespace, s.name.text, m.name.text);
                            if let Some(m_def_id) = def_map.lookup(&m_path) {
                                let where_constraints = combined_where_constraints(
                                    s.where_clause.as_ref(),
                                    m.where_clause.as_ref(),
                                    &all_tp,
                                    &namespace,
                                    &aliases,
                                    &def_map,
                                    *file_id,
                                    sink,
                                );
                                func_sigs.push(FuncSig {
                                    def_id: m_def_id,
                                    param_names: method_param_names(&m.params),
                                    params,
                                    param_defaults: method_param_default_flags(&m.params),
                                    param_variadic: method_param_variadic_flags(&m.params),
                                    where_constraints,
                                    return_ty,
                                    is_mut: m.is_mut,
                                });
                            }
                        }
                        let _ = where_constraints(
                            s.where_clause.as_ref(),
                            &tp,
                            &namespace,
                            &aliases,
                            &def_map,
                            *file_id,
                            sink,
                        );
                    }
                }
                Item::Enum(e) => {
                    let path = format!("{}.{}", namespace, e.name.text);
                    if let Some(def_id) = def_map.lookup(&path) {
                        let tp: Vec<SmolStr> =
                            e.type_params.iter().map(|p| p.name.text.clone()).collect();
                        let mut seen_variants: HashSet<SmolStr> = HashSet::new();
                        let variants: Vec<EnumVariantSig> = e
                            .variants
                            .iter()
                            .filter_map(|variant| {
                                let variant_name = variant.name.text.clone();
                                if !seen_variants.insert(variant_name.clone()) {
                                    sink.emit(
                                        Diagnostic::error(
                                            "bind.duplicate_variant",
                                            format!("duplicate variant `{}` in enum `{}`", variant_name, e.name.text),
                                        )
                                        .with_label(Label::primary(
                                            *file_id,
                                            variant.name.span,
                                            "duplicate variant name",
                                        ))
                                        .with_action("rename or remove one of the duplicate variants"),
                                    );
                                    // Still collect the variant so lowering doesn't panic.
                                }
                                let mut seen_variant_fields: HashSet<SmolStr> = HashSet::new();
                                let fields = variant
                                    .fields
                                    .iter()
                                    .filter_map(|field| {
                                        let ty = lower_type_with_aliases(
                                            &field.ty, &namespace, &tp, &def_map, *file_id,
                                            sink, &aliases,
                                        );
                                        let field_name = field.name.text.clone();
                                        if !seen_variant_fields.insert(field_name.clone()) {
                                            sink.emit(
                                                Diagnostic::error(
                                                    "bind.duplicate_field",
                                                    format!("duplicate field `{}` in variant `{}` of enum `{}`", field_name, variant_name, e.name.text),
                                                )
                                                .with_label(Label::primary(
                                                    *file_id,
                                                    field.name.span,
                                                    "duplicate field name",
                                                ))
                                                .with_action("rename or remove one of the duplicate fields"),
                                            );
                                        }
                                        Some((field_name, ty))
                                    })
                                    .collect();
                                Some(EnumVariantSig {
                                    name: variant_name,
                                    fields,
                                })
                            })
                            .collect();
                        enum_sigs.push(EnumSig { def_id, variants });
                    }
                }
                Item::Apply(apply) => {
                    let type_def_id =
                        resolve_qualified_def_id(&apply.for_type, &namespace, &aliases, &def_map);
                    let type_name = apply.for_type.last().text.clone();
                    let tp: Vec<SmolStr> = apply
                        .type_params
                        .iter()
                        .map(|p| p.name.text.clone())
                        .collect();
                    let self_ty = type_def_id
                        .map(|def_id| Ty::Named(def_id, Vec::new()))
                        .unwrap_or(Ty::Infer(0));

                    // Free methods + free binds: inherent on the type.
                    // Binds were aliased in phase 1b (`Type.slot` → free fn DefId);
                    // missing targets are already diagnosed there.
                    for member in &apply.free_members {
                        if let ori_ast::item::ApplyMember::Method(m) = member {
                            resolve_apply_method_func_sig(
                                m,
                                &namespace,
                                &type_name,
                                None,
                                &tp,
                                apply.where_clause.as_ref(),
                                &aliases,
                                &HashMap::new(),
                                &def_map,
                                self_ty.clone(),
                                *file_id,
                                sink,
                                &mut func_sigs,
                            );
                        }
                    }

                    let _ = where_constraints(
                        apply.where_clause.as_ref(),
                        &tp,
                        &namespace,
                        &aliases,
                        &def_map,
                        *file_id,
                        sink,
                    );

                    for use_sec in &apply.uses {
                        let trait_def_id = resolve_qualified_def_id(
                            &use_sec.trait_name,
                            &namespace,
                            &aliases,
                            &def_map,
                        );
                        let section_assoc_types: HashMap<SmolStr, ori_ast::ty::Type> = use_sec
                            .associated_types
                            .iter()
                            .map(|(name, ty)| (name.text.clone(), ty.clone()))
                            .collect();
                        let mut impl_methods = Vec::new();
                        for member in &use_sec.members {
                            match member {
                                ori_ast::item::ApplyMember::Method(m) => {
                                    let m_path = format!(
                                        "{}.{}.{}.{}",
                                        namespace,
                                        type_name,
                                        use_sec.trait_name.last().text,
                                        m.name.text
                                    );
                                    if let Some(m_def_id) = def_map.lookup(&m_path) {
                                        impl_methods.push(ImplMethodSig {
                                            name: m.name.text.clone(),
                                            func_def_id: m_def_id,
                                        });
                                    }
                                    resolve_apply_method_func_sig(
                                        m,
                                        &namespace,
                                        &type_name,
                                        Some(use_sec.trait_name.last().text.as_str()),
                                        &tp,
                                        apply.where_clause.as_ref(),
                                        &aliases,
                                        &section_assoc_types,
                                        &def_map,
                                        self_ty.clone(),
                                        *file_id,
                                        sink,
                                        &mut func_sigs,
                                    );
                                }
                                ori_ast::item::ApplyMember::Bind { slot, target, .. } => {
                                    // Compile-time bind: slot is provided by free function `target`.
                                    let target_path = format!("{}.{}", namespace, target.text);
                                    if let Some(target_def_id) = def_map.lookup(&target_path) {
                                        impl_methods.push(ImplMethodSig {
                                            name: slot.text.clone(),
                                            func_def_id: target_def_id,
                                        });
                                    } else {
                                        sink.emit(
                                            Diagnostic::error(
                                                "name.undefined",
                                                format!(
                                                    "bind target `{}` was not found",
                                                    target.text
                                                ),
                                            )
                                            .with_label(Label::primary(
                                                *file_id,
                                                target.span,
                                                "unknown function",
                                            ))
                                            .with_action(
                                                "bind to a free function declared in this module",
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                        if let (Some(trait_def_id), Some(type_def_id)) = (trait_def_id, type_def_id)
                        {
                            impl_sigs.push(ImplSig {
                                trait_def_id,
                                type_def_id,
                                methods: impl_methods,
                            });
                        }
                    }
                }
                Item::Trait(t) => {
                    let path = format!("{}.{}", namespace, t.name.text);
                    let trait_def_id = def_map.lookup(&path);
                    let mut tp: Vec<SmolStr> =
                        t.type_params.iter().map(|p| p.name.text.clone()).collect();
                    for m in &t.members {
                        if let ori_ast::item::TraitMember::Type(name) = m {
                            tp.push(name.text.clone());
                        }
                    }
                    let mut methods = Vec::new();
                    for m in &t.members {
                        match m {
                            ori_ast::item::TraitMember::Required(sig) => {
                                let mut all_tp = tp.clone();
                                all_tp.extend(sig.type_params.iter().map(|p| p.name.text.clone()));
                                let mut m_aliases = aliases.clone();
                                m_aliases.insert(SmolStr::new("Self"), t.name.text.clone());
                                let mut params: Vec<Ty> = sig
                                    .params
                                    .iter()
                                    .map(|p| {
                                        lower_type_with_aliases(
                                            &p.ty, &namespace, &all_tp, &def_map, *file_id, sink,
                                            &m_aliases,
                                        )
                                    })
                                    .collect();
                                if !has_explicit_self_param(&sig.params) {
                                    let self_ty = trait_def_id
                                        .map(|def_id| Ty::Named(def_id, Vec::new()))
                                        .unwrap_or(Ty::Infer(0));
                                    params.insert(0, self_ty);
                                }
                                let return_ty = sig
                                    .return_ty
                                    .as_ref()
                                    .map(|ty| {
                                        lower_type_with_aliases(
                                            ty, &namespace, &all_tp, &def_map, *file_id, sink,
                                            &m_aliases,
                                        )
                                    })
                                    .unwrap_or(Ty::Void);
                                let return_ty = async_return_ty(sig.is_async, return_ty);
                                methods.push(TraitMethodSig {
                                    name: sig.name.text.clone(),
                                    params: params.clone(),
                                    return_ty: return_ty.clone(),
                                    is_mut: sig.is_mut,
                                    has_default: false,
                                    span: sig.span,
                                });
                                let m_path =
                                    format!("{}.{}.{}", namespace, t.name.text, sig.name.text);
                                if let Some(m_def_id) = def_map.lookup(&m_path) {
                                    let where_constraints = combined_where_constraints(
                                        t.where_clause.as_ref(),
                                        sig.where_clause.as_ref(),
                                        &all_tp,
                                        &namespace,
                                        &aliases,
                                        &def_map,
                                        *file_id,
                                        sink,
                                    );
                                    func_sigs.push(FuncSig {
                                        def_id: m_def_id,
                                        param_names: method_param_names(&sig.params),
                                        params,
                                        param_defaults: method_param_default_flags(&sig.params),
                                        param_variadic: method_param_variadic_flags(&sig.params),
                                        where_constraints,
                                        return_ty,
                                        is_mut: sig.is_mut,
                                    });
                                }
                            }
                            ori_ast::item::TraitMember::Default(func) => {
                                let mut all_tp = tp.clone();
                                all_tp.extend(func.type_params.iter().map(|p| p.name.text.clone()));
                                let mut m_aliases = aliases.clone();
                                m_aliases.insert(SmolStr::new("Self"), t.name.text.clone());
                                let mut params: Vec<Ty> = func
                                    .params
                                    .iter()
                                    .map(|p| {
                                        lower_type_with_aliases(
                                            &p.ty, &namespace, &all_tp, &def_map, *file_id, sink,
                                            &m_aliases,
                                        )
                                    })
                                    .collect();
                                if !has_explicit_self_param(&func.params) {
                                    let self_ty = trait_def_id
                                        .map(|def_id| Ty::Named(def_id, Vec::new()))
                                        .unwrap_or(Ty::Infer(0));
                                    params.insert(0, self_ty);
                                }
                                let return_ty = func
                                    .return_ty
                                    .as_ref()
                                    .map(|ty| {
                                        lower_type_with_aliases(
                                            ty, &namespace, &all_tp, &def_map, *file_id, sink,
                                            &m_aliases,
                                        )
                                    })
                                    .unwrap_or(Ty::Void);
                                let return_ty = async_return_ty(func.is_async, return_ty);
                                methods.push(TraitMethodSig {
                                    name: func.name.text.clone(),
                                    params: params.clone(),
                                    return_ty: return_ty.clone(),
                                    is_mut: func.is_mut,
                                    has_default: true,
                                    span: func.span,
                                });
                                let m_path =
                                    format!("{}.{}.{}", namespace, t.name.text, func.name.text);
                                if let Some(m_def_id) = def_map.lookup(&m_path) {
                                    let where_constraints = combined_where_constraints(
                                        t.where_clause.as_ref(),
                                        func.where_clause.as_ref(),
                                        &all_tp,
                                        &namespace,
                                        &aliases,
                                        &def_map,
                                        *file_id,
                                        sink,
                                    );
                                    func_sigs.push(FuncSig {
                                        def_id: m_def_id,
                                        param_names: method_param_names(&func.params),
                                        params,
                                        param_defaults: method_param_default_flags(&func.params),
                                        param_variadic: method_param_variadic_flags(&func.params),
                                        where_constraints,
                                        return_ty,
                                        is_mut: func.is_mut,
                                    });
                                }
                            }
                            ori_ast::item::TraitMember::Type(_) => {}
                        }
                    }
                    let _ = where_constraints(
                        t.where_clause.as_ref(),
                        &tp,
                        &namespace,
                        &aliases,
                        &def_map,
                        *file_id,
                        sink,
                    );
                    if let Some(def_id) = trait_def_id {
                        trait_sigs.push(TraitSig { def_id, methods });
                    }
                }
                Item::Func(f) => {
                    let tp: Vec<SmolStr> =
                        f.type_params.iter().map(|p| p.name.text.clone()).collect();
                    let params = f
                        .params
                        .iter()
                        .map(|p| {
                            lower_type_with_aliases(
                                &p.ty, &namespace, &tp, &def_map, *file_id, sink, &aliases,
                            )
                        })
                        .collect();
                    let return_ty = f
                        .return_ty
                        .as_ref()
                        .map(|t| {
                            lower_type_with_aliases(
                                t, &namespace, &tp, &def_map, *file_id, sink, &aliases,
                            )
                        })
                        .unwrap_or(Ty::Void);
                    let return_ty = async_return_ty(f.is_async, return_ty);
                    let path = format!("{}.{}", namespace, f.name.text);
                    if let Some(def_id) = def_map.lookup(&path) {
                        let where_constraints = where_constraints(
                            f.where_clause.as_ref(),
                            &tp,
                            &namespace,
                            &aliases,
                            &def_map,
                            *file_id,
                            sink,
                        );
                        func_sigs.push(FuncSig {
                            def_id,
                            param_names: param_names(&f.params),
                            params,
                            param_defaults: param_default_flags(&f.params),
                            param_variadic: param_variadic_flags(&f.params),
                            where_constraints,
                            return_ty,
                            is_mut: f.is_mut,
                        });
                    }
                }
                Item::Const(c) => {
                    let path = format!("{}.{}", namespace, c.name.text);
                    if let Some(def_id) = def_map.lookup(&path) {
                        let ty = lower_type_with_aliases(
                            &c.ty,
                            &namespace,
                            &[],
                            &def_map,
                            *file_id,
                            sink,
                            &aliases,
                        );
                        value_sigs.push(ValueSig { def_id, ty });
                    }
                }
                Item::Var(v) => {
                    let path = format!("{}.{}", namespace, v.name.text);
                    if let Some(def_id) = def_map.lookup(&path) {
                        let ty = lower_type_with_aliases(
                            &v.ty,
                            &namespace,
                            &[],
                            &def_map,
                            *file_id,
                            sink,
                            &aliases,
                        );
                        value_sigs.push(ValueSig { def_id, ty });
                    }
                }
                Item::Extern(ext) => {
                    for member in &ext.members {
                        match member {
                            ori_ast::item::ExternMember::Func {
                                name,
                                params,
                                return_ty,
                                ..
                            } => {
                                let path = format!("{}.{}", namespace, name.text);
                                if let Some(def_id) = def_map.lookup(&path) {
                                    let resolved_params: Vec<Ty> = params
                                        .iter()
                                        .map(|p| {
                                            lower_type_with_aliases(
                                                &p.ty,
                                                &namespace,
                                                &[],
                                                &def_map,
                                                *file_id,
                                                sink,
                                                &aliases,
                                            )
                                        })
                                        .collect();
                                    let resolved_return_ty = return_ty
                                        .as_ref()
                                        .map(|t| {
                                            lower_type_with_aliases(
                                                t,
                                                &namespace,
                                                &[],
                                                &def_map,
                                                *file_id,
                                                sink,
                                                &aliases,
                                            )
                                        })
                                        .unwrap_or(Ty::Void);
                                    func_sigs.push(FuncSig {
                                        def_id,
                                        param_names: param_names(params),
                                        params: resolved_params,
                                        param_defaults: param_default_flags(params),
                                        param_variadic: param_variadic_flags(params),
                                        where_constraints: Vec::new(),
                                        return_ty: resolved_return_ty,
                                        is_mut: false,
                                    });
                                }
                            }
                            ori_ast::item::ExternMember::Var { name, ty, .. } => {
                                let path = format!("{}.{}", namespace, name.text);
                                if let Some(def_id) = def_map.lookup(&path) {
                                    let resolved_ty = lower_type_with_aliases(
                                        ty,
                                        &namespace,
                                        &[],
                                        &def_map,
                                        *file_id,
                                        sink,
                                        &aliases,
                                    );
                                    value_sigs.push(ValueSig {
                                        def_id,
                                        ty: resolved_ty,
                                    });
                                }
                            }
                        }
                    }
                }
                Item::Newtype(n) => {
                    let path = format!("{}.{}", namespace, n.name.text);
                    if let Some(def_id) = def_map.lookup(&path) {
                        let repr = lower_type_with_aliases(
                            &n.repr,
                            &namespace,
                            &[],
                            &def_map,
                            *file_id,
                            sink,
                            &aliases,
                        );
                        newtype_sigs.push(NewtypeSig { def_id, repr });
                    }
                }
                Item::Alias(a) => {
                    let path = format!("{}.{}", namespace, a.name.text);
                    if let Some(def_id) = def_map.lookup(&path) {
                        let tp: Vec<SmolStr> =
                            a.type_params.iter().map(|p| p.name.text.clone()).collect();
                        let ty = lower_type_with_aliases(
                            &a.ty, &namespace, &tp, &def_map, *file_id, sink, &aliases,
                        );
                        type_alias_sigs.push(TypeAliasSig {
                            def_id,
                            type_params: tp,
                            ty,
                        });
                    }
                }
            }
        }
    }

    // ── Phase 3: normalize type aliases in all collected signatures ─────────────
    // Build a DefId → (arity, underlying Ty) map from the alias sigs, then
    // expand alias references in every Ty stored in the resolved module.
    // We iterate up to 16 times to handle chains of aliases (A → B → int).
    for _ in 0..16 {
        let alias_map: HashMap<DefId, (usize, Ty)> = type_alias_sigs
            .iter()
            .map(|s| (s.def_id, (s.type_params.len(), s.ty.clone())))
            .collect();
        let mut changed = false;
        let expand = |ty: Ty| -> Ty { expand_ty_aliases(ty, &def_map, &alias_map) };
        // Expand alias sigs themselves (for chained aliases)
        for sig in &mut type_alias_sigs {
            let new_ty = expand(sig.ty.clone());
            if new_ty != sig.ty {
                changed = true;
                sig.ty = new_ty;
            }
        }
        // Expand func sig param/return types
        for sig in &mut func_sigs {
            for ty in &mut sig.params {
                let new_ty = expand(ty.clone());
                if new_ty != *ty {
                    changed = true;
                    *ty = new_ty;
                }
            }
            let new_ret = expand(sig.return_ty.clone());
            if new_ret != sig.return_ty {
                changed = true;
                sig.return_ty = new_ret;
            }
        }
        // Expand struct field types
        for sig in &mut struct_sigs {
            for (_, ty) in &mut sig.fields {
                let new_ty = expand(ty.clone());
                if new_ty != *ty {
                    changed = true;
                    *ty = new_ty;
                }
            }
        }
        // Expand value sig types (consts/vars)
        for sig in &mut value_sigs {
            let new_ty = expand(sig.ty.clone());
            if new_ty != sig.ty {
                changed = true;
                sig.ty = new_ty;
            }
        }
        // Expand trait method types
        for sig in &mut trait_sigs {
            for m in &mut sig.methods {
                for ty in &mut m.params {
                    let new_ty = expand(ty.clone());
                    if new_ty != *ty {
                        changed = true;
                        *ty = new_ty;
                    }
                }
                let new_ret = expand(m.return_ty.clone());
                if new_ret != m.return_ty {
                    changed = true;
                    m.return_ty = new_ret;
                }
            }
        }
        // Note: ImplSig methods only carry name/func_def_id references;
        // the actual method types are in func_sigs (already normalized above).
        if !changed {
            break;
        }
    }

    ResolvedModule {
        def_map,
        func_sigs,
        value_sigs,
        struct_sigs,
        enum_sigs,
        trait_sigs,
        impl_sigs,
        type_alias_sigs,
        newtype_sigs,
        deprecated_sigs,
        reexports,
        namespace: entry_namespace.into(),
    }
}

fn combined_where_constraints(
    outer: Option<&WhereClause>,
    inner: Option<&WhereClause>,
    type_params: &[SmolStr],
    namespace: &str,
    aliases: &HashMap<SmolStr, SmolStr>,
    def_map: &DefMap,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) -> Vec<WhereConstraintSig> {
    let mut constraints = where_constraints(
        outer,
        type_params,
        namespace,
        aliases,
        def_map,
        file_id,
        sink,
    );
    constraints.extend(where_constraints(
        inner,
        type_params,
        namespace,
        aliases,
        def_map,
        file_id,
        sink,
    ));
    constraints
}

fn async_return_ty(is_async: bool, inner: Ty) -> Ty {
    if is_async {
        Ty::Future(Box::new(inner))
    } else {
        inner
    }
}

fn where_constraints(
    clause: Option<&WhereClause>,
    type_params: &[SmolStr],
    namespace: &str,
    aliases: &HashMap<SmolStr, SmolStr>,
    def_map: &DefMap,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) -> Vec<WhereConstraintSig> {
    let Some(clause) = clause else {
        return Vec::new();
    };

    let mut constraints = Vec::new();
    for constraint in &clause.constraints {
        let (param, bound, negative, span) = match constraint {
            WhereConstraint::Is { param, bound, span } => (param, bound, false, *span),
            WhereConstraint::IsNot { param, bound, span } => (param, bound, true, *span),
        };

        let Some(param_index) = type_params.iter().position(|p| p == &param.text) else {
            sink.emit(
                Diagnostic::error(
                    "generic.unknown_type_param",
                    format!("unknown generic parameter `{}` in where clause", param.text),
                )
                .with_label(Label::primary(file_id, param.span, "unknown parameter"))
                .with_action("use one of the generic parameters declared on this item"),
            );
            continue;
        };

        let Some(trait_def_id) = resolve_qualified_def_id(bound, namespace, aliases, def_map)
        else {
            sink.emit(
                Diagnostic::error(
                    "type.undefined_name",
                    format!("undefined trait `{}` in where clause", bound),
                )
                .with_label(Label::primary(file_id, bound.span, "trait not found"))
                .with_action("define or import the trait before using it in `where`"),
            );
            continue;
        };

        if def_map.get(trait_def_id).kind != DefKind::Trait {
            sink.emit(
                Diagnostic::error(
                    "generic.constraint_not_trait",
                    format!("`{}` is not a trait", bound),
                )
                .with_label(Label::primary(file_id, span, "constraint declared here"))
                .with_action("use a trait name after `is`"),
            );
            continue;
        }

        constraints.push(WhereConstraintSig {
            param_index: param_index as u32,
            param_name: param.text.clone(),
            trait_def_id,
            negative,
        });
    }
    constraints
}

fn param_default_flags(params: &[Param]) -> Vec<bool> {
    params
        .iter()
        .map(|param| {
            matches!(
                param.kind,
                ParamKind::Default(_) | ParamKind::DefaultAndContract(_, _)
            )
        })
        .collect()
}

fn param_variadic_flags(params: &[Param]) -> Vec<bool> {
    params
        .iter()
        .map(|param| matches!(param.kind, ParamKind::Variadic))
        .collect()
}

fn param_names(params: &[Param]) -> Vec<SmolStr> {
    params.iter().map(|param| param.name.text.clone()).collect()
}

/// Lower a free or trait-method FuncDecl from an `apply` block into `func_sigs`.
fn resolve_apply_method_func_sig(
    m: &ori_ast::item::FuncDecl,
    namespace: &str,
    type_name: &SmolStr,
    trait_name: Option<&str>,
    apply_tp: &[SmolStr],
    apply_where: Option<&WhereClause>,
    aliases: &HashMap<SmolStr, SmolStr>,
    // `type Item = int` declared in the enclosing `use Trait` section, so the
    // signature can name it. Empty for free members.
    associated_types: &HashMap<SmolStr, ori_ast::ty::Type>,
    def_map: &DefMap,
    self_ty: Ty,
    file_id: FileId,
    sink: &mut DiagnosticSink,
    func_sigs: &mut Vec<FuncSig>,
) {
    let mut all_tp = apply_tp.to_vec();
    all_tp.extend(m.type_params.iter().map(|p| p.name.text.clone()));
    let mut m_aliases = aliases.clone();
    m_aliases.insert(SmolStr::new("Self"), type_name.clone());
    let mut params: Vec<Ty> = m
        .params
        .iter()
        .map(|p| {
            crate::lower::lower_type_with_local_aliases(
                &p.ty,
                namespace,
                &all_tp,
                def_map,
                file_id,
                sink,
                &m_aliases,
                associated_types,
            )
        })
        .collect();
    if !has_explicit_self_param(&m.params) {
        params.insert(0, self_ty);
    }
    let return_ty = m
        .return_ty
        .as_ref()
        .map(|t| {
            crate::lower::lower_type_with_local_aliases(
                t,
                namespace,
                &all_tp,
                def_map,
                file_id,
                sink,
                &m_aliases,
                associated_types,
            )
        })
        .unwrap_or(Ty::Void);
    let return_ty = async_return_ty(m.is_async, return_ty);
    let m_path = match trait_name {
        Some(trait_name) => format!("{}.{}.{}.{}", namespace, type_name, trait_name, m.name.text),
        None => format!("{}.{}.{}", namespace, type_name, m.name.text),
    };
    if let Some(m_def_id) = def_map.lookup(&m_path) {
        let where_constraints = combined_where_constraints(
            apply_where,
            m.where_clause.as_ref(),
            &all_tp,
            namespace,
            aliases,
            def_map,
            file_id,
            sink,
        );
        func_sigs.push(FuncSig {
            def_id: m_def_id,
            param_names: method_param_names(&m.params),
            params,
            param_defaults: method_param_default_flags(&m.params),
            param_variadic: method_param_variadic_flags(&m.params),
            where_constraints,
            return_ty,
            is_mut: m.is_mut,
        });
    }
}

fn has_explicit_self_param(params: &[Param]) -> bool {
    params
        .first()
        .is_some_and(|param| param.name.text.as_str() == "self")
}

fn method_param_names(params: &[Param]) -> Vec<SmolStr> {
    let mut names = param_names(params);
    if !has_explicit_self_param(params) {
        names.insert(0, SmolStr::new("self"));
    }
    names
}

fn method_param_default_flags(params: &[Param]) -> Vec<bool> {
    let mut flags = param_default_flags(params);
    if !has_explicit_self_param(params) {
        flags.insert(0, false);
    }
    flags
}

fn method_param_variadic_flags(params: &[Param]) -> Vec<bool> {
    let mut flags = param_variadic_flags(params);
    if !has_explicit_self_param(params) {
        flags.insert(0, false);
    }
    flags
}

fn resolve_qualified_def_id(
    name: &ori_ast::common::QualifiedName,
    namespace: &str,
    aliases: &HashMap<SmolStr, SmolStr>,
    def_map: &DefMap,
) -> Option<DefId> {
    let raw = name.to_string();
    let expanded = expand_qualified_alias(&raw, aliases);
    def_map
        .lookup(&expanded)
        .or_else(|| def_map.lookup(&format!("{}.{}", namespace, expanded)))
}

fn expand_qualified_alias(name: &str, aliases: &HashMap<SmolStr, SmolStr>) -> String {
    let mut prefix_end = name.len();
    loop {
        let prefix = &name[..prefix_end];
        if let Some(full_ns) = aliases.get(prefix) {
            let suffix = &name[prefix_end..];
            if suffix.is_empty() {
                return full_ns.to_string();
            }
            return format!("{}{}", full_ns, suffix);
        }
        if let Some(dot) = name[..prefix_end].rfind('.') {
            prefix_end = dot;
        } else {
            break;
        }
    }
    name.to_string()
}

// ── Registration helpers ──────────────────────────────────────────────────────

const CORE_TRAIT_NAMES: &[&str] = &[
    "Displayable",
    "Addable",
    "Subtractable",
    "Multiplicable",
    "Divisible",
    "Equatable",
    "Comparable",
    "Hashable",
    "Disposable",
    "Iterable",
    "Default",
    "Error",
    "Cloneable",
    "Transferable",
];

fn register_core_traits(def_map: &mut DefMap) -> Vec<(SmolStr, DefId)> {
    CORE_TRAIT_NAMES
        .iter()
        .map(|name| {
            let name_s = SmolStr::new(*name);
            let path = SmolStr::new(format!("ori.core.{name}"));
            let def_id = def_map.register(
                DefKind::Trait,
                name_s.clone(),
                path,
                true,
                ori_diagnostics::Span::DUMMY,
            );
            (name_s, def_id)
        })
        .collect()
}

fn register_stdlib_error_type(def_map: &mut DefMap) -> DefId {
    def_map.register(
        DefKind::Struct,
        SmolStr::new("Error"),
        SmolStr::new("ori.Error"),
        true,
        ori_diagnostics::Span::DUMMY,
    )
}

fn register_stdlib_json_value_enum(def_map: &mut DefMap) -> DefId {
    def_map.register(
        DefKind::Enum,
        SmolStr::new("Value"),
        SmolStr::new("ori.json.Value"),
        true,
        ori_diagnostics::Span::DUMMY,
    )
}

fn builtin_stdlib_error_struct_sig(def_id: DefId) -> StructSig {
    StructSig {
        def_id,
        fields: vec![
            (SmolStr::new("code"), Ty::String),
            (SmolStr::new("message"), Ty::String),
            // Error chaining: message describing the original cause.
            // Empty string means there is no cause.
            // Future: migrate to `optional[any[Error]]` once the C backend supports
            // recursive struct field types.
            (SmolStr::new("cause"), Ty::String),
        ],
    }
}

fn builtin_stdlib_json_value_enum_sig(def_id: DefId) -> EnumSig {
    EnumSig {
        def_id,
        variants: vec![
            EnumVariantSig {
                name: SmolStr::new("Null"),
                fields: Vec::new(),
            },
            EnumVariantSig {
                name: SmolStr::new("Bool"),
                fields: vec![(SmolStr::new("value"), Ty::Bool)],
            },
            EnumVariantSig {
                name: SmolStr::new("Number"),
                fields: vec![(SmolStr::new("value"), Ty::Float)],
            },
            EnumVariantSig {
                name: SmolStr::new("String"),
                fields: vec![(SmolStr::new("value"), Ty::String)],
            },
            EnumVariantSig {
                name: SmolStr::new("Array"),
                fields: vec![(
                    SmolStr::new("items"),
                    Ty::List(Box::new(Ty::Named(def_id, Vec::new()))),
                )],
            },
            EnumVariantSig {
                name: SmolStr::new("Object"),
                fields: vec![(
                    SmolStr::new("fields"),
                    Ty::Map(
                        Box::new(Ty::String),
                        Box::new(Ty::Named(def_id, Vec::new())),
                    ),
                )],
            },
        ],
    }
}

fn builtin_core_trait_sigs(core_traits: &[(SmolStr, DefId)]) -> Vec<TraitSig> {
    core_traits
        .iter()
        .map(|(name, def_id)| {
            let self_ty = Ty::Named(*def_id, Vec::new());
            let methods = match name.as_str() {
                "Addable" => vec![TraitMethodSig {
                    name: SmolStr::new("add"),
                    params: vec![self_ty.clone(), self_ty.clone()],
                    return_ty: self_ty,
                    is_mut: false,
                    has_default: false,
                    span: ori_diagnostics::Span::DUMMY,
                }],
                "Subtractable" => vec![TraitMethodSig {
                    name: SmolStr::new("subtract"),
                    params: vec![self_ty.clone(), self_ty.clone()],
                    return_ty: self_ty,
                    is_mut: false,
                    has_default: false,
                    span: ori_diagnostics::Span::DUMMY,
                }],
                "Multiplicable" => vec![TraitMethodSig {
                    name: SmolStr::new("multiply"),
                    params: vec![self_ty.clone(), self_ty.clone()],
                    return_ty: self_ty,
                    is_mut: false,
                    has_default: false,
                    span: ori_diagnostics::Span::DUMMY,
                }],
                "Divisible" => vec![TraitMethodSig {
                    name: SmolStr::new("divide"),
                    params: vec![self_ty.clone(), self_ty.clone()],
                    return_ty: self_ty,
                    is_mut: false,
                    has_default: false,
                    span: ori_diagnostics::Span::DUMMY,
                }],
                "Equatable" => vec![TraitMethodSig {
                    name: SmolStr::new("equals"),
                    params: vec![self_ty.clone(), self_ty],
                    return_ty: Ty::Bool,
                    is_mut: false,
                    has_default: false,
                    span: ori_diagnostics::Span::DUMMY,
                }],
                "Comparable" => vec![TraitMethodSig {
                    name: SmolStr::new("compare"),
                    params: vec![self_ty.clone(), self_ty],
                    return_ty: Ty::Int,
                    is_mut: false,
                    has_default: false,
                    span: ori_diagnostics::Span::DUMMY,
                }],
                "Disposable" => vec![TraitMethodSig {
                    name: SmolStr::new("dispose"),
                    params: vec![self_ty],
                    return_ty: Ty::Void,
                    is_mut: true,
                    has_default: false,
                    span: ori_diagnostics::Span::DUMMY,
                }],
                "Displayable" => vec![TraitMethodSig {
                    name: SmolStr::new("display"),
                    params: vec![self_ty],
                    return_ty: Ty::String,
                    is_mut: false,
                    has_default: false,
                    span: ori_diagnostics::Span::DUMMY,
                }],
                _ => Vec::new(),
            };
            TraitSig {
                def_id: *def_id,
                methods,
            }
        })
        .collect()
}

/// Phase 1b: alias free bind slots to free functions for inherent method lookup.
fn register_apply_free_binds(
    ns: &str,
    apply: &ori_ast::item::ApplyDecl,
    def_map: &mut DefMap,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) {
    let type_name = apply.for_type.last().text.as_str();
    for member in &apply.free_members {
        let ori_ast::item::ApplyMember::Bind { slot, target, span } = member else {
            continue;
        };
        let target_path = format!("{}.{}", ns, target.text);
        let Some(target_def_id) = def_map.lookup(&target_path) else {
            sink.emit(
                Diagnostic::error(
                    "name.undefined",
                    format!("bind target `{}` was not found", target.text),
                )
                .with_label(Label::primary(file_id, target.span, "unknown function"))
                .with_action("bind to a free function declared in this module"),
            );
            continue;
        };
        let slot_path = SmolStr::new(format!("{}.{}.{}", ns, type_name, slot.text));
        if def_map.lookup(&slot_path).is_some() {
            sink.emit(
                Diagnostic::error(
                    "name.duplicate",
                    format!("duplicate definition `{}.{}`", type_name, slot.text),
                )
                .with_label(Label::primary(file_id, *span, "defined again here"))
                .with_action("rename or remove one of the definitions"),
            );
            continue;
        }
        def_map.alias_path(slot_path, target_def_id);
    }
}

fn register_def(
    def_map: &mut DefMap,
    ns: &str,
    kind: DefKind,
    name: &SmolStr,
    is_public: bool,
    span: ori_diagnostics::Span,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) {
    let path = SmolStr::new(format!("{}.{}", ns, name));
    if def_map.lookup(&path).is_some() {
        sink.emit(
            Diagnostic::error("name.duplicate", format!("duplicate definition `{}`", name))
                .with_label(Label::primary(file_id, span, "defined again here"))
                .with_action("rename or remove one of the definitions"),
        );
        return;
    }
    def_map.register(kind, name.clone(), path, is_public, span);
}

fn register_item(
    item: &Item,
    ns: &str,
    def_map: &mut DefMap,
    implemented_pairs: &mut HashMap<(String, String), ori_diagnostics::Span>,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) {
    match item {
        Item::Struct(s) => {
            register_def(
                def_map,
                ns,
                DefKind::Struct,
                &s.name.text,
                s.visibility.is_public(),
                s.span,
                file_id,
                sink,
            );
            for m in &s.methods {
                let m_name = SmolStr::new(format!("{}.{}", s.name.text, m.name.text));
                register_def(
                    def_map,
                    ns,
                    DefKind::Func,
                    &m_name,
                    m.visibility.is_public(),
                    m.span,
                    file_id,
                    sink,
                );
            }
        }
        Item::Enum(e) => register_def(
            def_map,
            ns,
            DefKind::Enum,
            &e.name.text,
            e.visibility.is_public(),
            e.span,
            file_id,
            sink,
        ),
        Item::Trait(t) => {
            register_def(
                def_map,
                ns,
                DefKind::Trait,
                &t.name.text,
                t.visibility.is_public(),
                t.span,
                file_id,
                sink,
            );
            for m in &t.members {
                match m {
                    ori_ast::item::TraitMember::Required(sig) => {
                        let m_name = SmolStr::new(format!("{}.{}", t.name.text, sig.name.text));
                        register_def(
                            def_map,
                            ns,
                            DefKind::Func,
                            &m_name,
                            sig.visibility.is_public(),
                            sig.span,
                            file_id,
                            sink,
                        );
                    }
                    ori_ast::item::TraitMember::Default(func) => {
                        let m_name = SmolStr::new(format!("{}.{}", t.name.text, func.name.text));
                        register_def(
                            def_map,
                            ns,
                            DefKind::Func,
                            &m_name,
                            func.visibility.is_public(),
                            func.span,
                            file_id,
                            sink,
                        );
                    }
                    ori_ast::item::TraitMember::Type(_) => {}
                }
            }
        }
        Item::Func(f) => register_def(
            def_map,
            ns,
            DefKind::Func,
            &f.name.text,
            f.visibility.is_public(),
            f.span,
            file_id,
            sink,
        ),
        Item::Newtype(n) => register_def(
            def_map,
            ns,
            DefKind::Newtype,
            &n.name.text,
            n.visibility.is_public(),
            n.span,
            file_id,
            sink,
        ),
        Item::Alias(a) => register_def(
            def_map,
            ns,
            DefKind::TypeAlias,
            &a.name.text,
            a.visibility.is_public(),
            a.span,
            file_id,
            sink,
        ),
        Item::Const(c) => register_def(
            def_map,
            ns,
            DefKind::Const,
            &c.name.text,
            c.visibility.is_public(),
            c.span,
            file_id,
            sink,
        ),
        Item::Var(v) => register_def(
            def_map,
            ns,
            DefKind::Var,
            &v.name.text,
            v.visibility.is_public(),
            v.span,
            file_id,
            sink,
        ),
        Item::Extern(ext) => {
            for member in &ext.members {
                match member {
                    ori_ast::item::ExternMember::Func {
                        visibility,
                        name,
                        span,
                        ..
                    } => register_def(
                        def_map,
                        ns,
                        DefKind::Extern,
                        &name.text,
                        visibility.is_public(),
                        *span,
                        file_id,
                        sink,
                    ),
                    ori_ast::item::ExternMember::Var {
                        visibility,
                        name,
                        span,
                        ..
                    } => register_def(
                        def_map,
                        ns,
                        DefKind::Var,
                        &name.text,
                        visibility.is_public(),
                        *span,
                        file_id,
                        sink,
                    ),
                }
            }
        }
        Item::Apply(apply) => {
            let type_key = qualify_name_in_namespace(&apply.for_type, ns);
            let type_name = apply.for_type.last().text.clone();

            for member in &apply.free_members {
                if let ori_ast::item::ApplyMember::Method(m) = member {
                    let m_name = SmolStr::new(format!("{}.{}", type_name, m.name.text));
                    register_def(
                        def_map,
                        ns,
                        DefKind::Func,
                        &m_name,
                        m.visibility.is_public(),
                        m.span,
                        file_id,
                        sink,
                    );
                }
            }

            for use_sec in &apply.uses {
                let trait_key = qualify_name_in_namespace(&use_sec.trait_name, ns);
                let key = (trait_key.clone(), type_key.clone());
                if implemented_pairs.insert(key, use_sec.span).is_some() {
                    sink.emit(
                        Diagnostic::error(
                            "bind.duplicate_implement",
                            format!("`{}` is already applied to `{}`", trait_key, type_key),
                        )
                        .with_label(Label::primary(
                            file_id,
                            use_sec.span,
                            "duplicate apply/use here",
                        ))
                        .with_action(
                            "keep only one `use Trait` for this trait/type pair across apply blocks",
                        ),
                    );
                    continue;
                }
                for member in &use_sec.members {
                    if let ori_ast::item::ApplyMember::Method(m) = member {
                        let m_name = SmolStr::new(format!(
                            "{}.{}.{}",
                            type_name,
                            use_sec.trait_name.last().text,
                            m.name.text
                        ));
                        register_def(
                            def_map,
                            ns,
                            DefKind::Func,
                            &m_name,
                            m.visibility.is_public(),
                            m.span,
                            file_id,
                            sink,
                        );
                    }
                }
            }
        }
    }
}

fn qualify_name_in_namespace(name: &ori_ast::common::QualifiedName, ns: &str) -> String {
    if name.is_single() {
        format!("{}.{}", ns, name)
    } else {
        name.to_string()
    }
}

fn collect_deprecated_sigs(
    files: &[(&SourceFile, FileId)],
    def_map: &DefMap,
) -> Vec<DeprecatedSig> {
    let mut deprecated = Vec::new();
    for (file, _) in files {
        let namespace = SmolStr::new(file.namespace.name.to_string());
        for item in &file.items {
            let Some(message) = deprecated_message(item) else {
                continue;
            };
            for path in item_def_paths(&item.item, &namespace) {
                if let Some(def_id) = def_map.lookup(&path) {
                    deprecated.push(DeprecatedSig {
                        def_id,
                        message: message.clone(),
                    });
                }
            }
        }
    }
    deprecated
}

fn deprecated_message(item: &ItemWithAttrs) -> Option<SmolStr> {
    item.attrs.iter().find_map(|attr| {
        if attr.name.text != "deprecated" {
            return None;
        }
        match attr.args.as_slice() {
            [AttrArg::String(message, _)] => Some(message.clone()),
            _ => None,
        }
    })
}

fn item_def_paths(item: &Item, namespace: &str) -> Vec<String> {
    match item {
        Item::Struct(s) => vec![format!("{}.{}", namespace, s.name.text)],
        Item::Enum(e) => vec![format!("{}.{}", namespace, e.name.text)],
        Item::Trait(t) => vec![format!("{}.{}", namespace, t.name.text)],
        Item::Func(f) => vec![format!("{}.{}", namespace, f.name.text)],
        Item::Alias(a) => vec![format!("{}.{}", namespace, a.name.text)],
        Item::Newtype(n) => vec![format!("{}.{}", namespace, n.name.text)],
        Item::Const(c) => vec![format!("{}.{}", namespace, c.name.text)],
        Item::Var(v) => vec![format!("{}.{}", namespace, v.name.text)],
        Item::Extern(ext) => ext
            .members
            .iter()
            .map(|member| match member {
                ori_ast::item::ExternMember::Func { name, .. }
                | ori_ast::item::ExternMember::Var { name, .. } => {
                    format!("{}.{}", namespace, name.text)
                }
            })
            .collect(),
        Item::Apply(_) => Vec::new(),
    }
}

fn collect_reexports(files: &[(&SourceFile, FileId)]) -> Vec<ReExport> {
    let mut reexports = Vec::new();
    for (file, _) in files {
        let namespace = SmolStr::new(file.namespace.name.to_string());
        for import in &file.imports {
            if !import.visibility.is_public() {
                continue;
            }
            if !import.selected.is_empty() {
                for item in &import.selected {
                    reexports.push(ReExport {
                        namespace: namespace.clone(),
                        alias: selected_import_alias(item),
                        target: selected_import_target(import, item),
                    });
                }
            } else if let Some(alias) = direct_import_alias(import) {
                // S3: bare `public import path` has no local binding — only
                // `path = alias` and selective forms re-export a name.
                reexports.push(ReExport {
                    namespace: namespace.clone(),
                    alias,
                    target: SmolStr::new(import.path.to_string()),
                });
            }
        }
    }
    reexports
}

pub fn import_aliases(file: &SourceFile, reexports: &[ReExport]) -> HashMap<SmolStr, SmolStr> {
    let mut aliases = HashMap::new();
    for import in &file.imports {
        if !import.selected.is_empty() {
            for item in &import.selected {
                aliases.insert(
                    selected_import_alias(item),
                    selected_import_target(import, item),
                );
            }
        } else if let Some(alias) = direct_import_alias(import) {
            aliases.insert(alias, SmolStr::new(import.path.to_string()));
        } else {
            // Bare whole-module import: no short alias. Map the full path to
            // itself so `ori.io.print` resolves/tracks usage without creating
            // an implicit last-segment binding like `io`.
            let path = SmolStr::new(import.path.to_string());
            aliases.insert(path.clone(), path);
        }
    }

    for _ in 0..reexports.len().saturating_add(1) {
        let mut changed = false;
        let snapshot: Vec<(SmolStr, SmolStr)> = aliases
            .iter()
            .map(|(visible, target)| (visible.clone(), target.clone()))
            .collect();
        for (visible_prefix, target_ns) in snapshot {
            // Skip identity full-path keys when expanding reexports through
            // short aliases only (multi-segment bare keys are not reexport hosts).
            if visible_prefix.contains('.') {
                continue;
            }
            for reexport in reexports.iter().filter(|r| r.namespace == target_ns) {
                let visible = SmolStr::new(format!("{}.{}", visible_prefix, reexport.alias));
                if aliases.contains_key(&visible) {
                    continue;
                }
                aliases.insert(visible, reexport.target.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    aliases
}

/// Explicit `path = alias` binding, if any. Bare imports return `None` (S3).
fn direct_import_alias(import: &ImportDecl) -> Option<SmolStr> {
    import.alias.as_ref().map(|a| a.text.clone())
}

fn selected_import_alias(item: &ori_ast::item::ImportItem) -> SmolStr {
    item.alias
        .as_ref()
        .map(|a| a.text.clone())
        .unwrap_or_else(|| item.name.text.clone())
}

fn selected_import_target(import: &ImportDecl, item: &ori_ast::item::ImportItem) -> SmolStr {
    SmolStr::new(format!("{}.{}", import.path, item.name.text))
}
