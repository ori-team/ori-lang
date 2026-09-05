use ori_diagnostics::DiagnosticSink;
use ori_hir::HirModule;
use ori_types::{resolve::ResolvedModule, Ty};
use std::collections::HashSet;
use std::path::PathBuf;

use super::project::{namespace_of, LoadedSource};

/// Lower every resolved source module and merge the resulting HIR into one
/// program module for optimization and backend generation.
pub(super) fn lower_loaded_sources(
    loaded: &[LoadedSource],
    resolved: &ResolvedModule,
    sink: &mut DiagnosticSink,
) -> HirModule {
    let (first, rest) = loaded.split_first().expect("entry source is loaded");
    let first_namespace = namespace_of(&first.ast);
    let mut merged = ori_hir::lower(
        &first.ast,
        &resolved.def_map,
        &resolved.func_sigs,
        &resolved.value_sigs,
        &resolved.struct_sigs,
        &resolved.enum_sigs,
        &resolved.trait_sigs,
        &resolved.impl_sigs,
        &resolved.type_alias_sigs,
        &resolved.newtype_sigs,
        &resolved.reexports,
        &first_namespace,
        first.file_id,
        sink,
    );
    for source in rest {
        let namespace = namespace_of(&source.ast);
        let mut hir = ori_hir::lower(
            &source.ast,
            &resolved.def_map,
            &resolved.func_sigs,
            &resolved.value_sigs,
            &resolved.struct_sigs,
            &resolved.enum_sigs,
            &resolved.trait_sigs,
            &resolved.impl_sigs,
            &resolved.type_alias_sigs,
            &resolved.newtype_sigs,
            &resolved.reexports,
            &namespace,
            source.file_id,
            sink,
        );
        merged.structs.append(&mut hir.structs);
        merged.enums.append(&mut hir.enums);
        merged.traits.append(&mut hir.traits);
        merged.trait_impls.append(&mut hir.trait_impls);
        merged.funcs.append(&mut hir.funcs);
        merged.consts.append(&mut hir.consts);
        merged.externs.append(&mut hir.externs);
    }

    // Automatically append stdlib enums (e.g. ori.json.Value) to the HirModule
    // so they are correctly registered in layout computations in code generation backends.
    let json_val_def_id = resolved.def_map.lookup("ori.json.Value");
    if let Some(concrete_id) = json_val_def_id {
        if !merged.enums.iter().any(|e| e.def_id == concrete_id) {
            if let Some(sig) = resolved.enum_sigs.iter().find(|s| s.def_id == concrete_id) {
                let variants = sig
                    .variants
                    .iter()
                    .map(|v| {
                        let fields = v
                            .fields
                            .iter()
                            .map(|(fname, fty)| ori_hir::hir::HirField {
                                name: fname.clone(),
                                ty: fty.clone(),
                                contract: None,
                                span: ori_diagnostics::Span::DUMMY,
                            })
                            .collect();
                        ori_hir::hir::HirVariant {
                            name: v.name.clone(),
                            fields,
                            span: ori_diagnostics::Span::DUMMY,
                        }
                    })
                    .collect();
                let hir_enum = ori_hir::hir::HirEnum {
                    def_id: concrete_id,
                    name: smol_str::SmolStr::new("ori.json.Value"),
                    variants,
                    is_public: true,
                    span: ori_diagnostics::Span::DUMMY,
                };
                merged.enums.push(hir_enum);
            }
        }
    }

    ori_hir::insert_default_arguments(&mut merged);
    ori_hir::monomorphize_generics(&mut merged);
    let violations = ori_hir::verify_module(&merged);
    if !violations.is_empty() {
        eprintln!("HIR verifier violations: {violations:?}");
    }
    debug_assert!(
        violations.is_empty(),
        "HIR verifier failed after lowering: {violations:?}"
    );
    merged
}

/// Split the already-lowered program into linkable units. Shared type and
/// trait metadata is copied into every unit so each Cranelift module can lay
/// out values consistently; only the functions and globals owned by the
/// source file are defined there. Cross-file functions/globals are imports.
pub(super) fn split_native_modules(
    hir: &HirModule,
    loaded: &[LoadedSource],
) -> Vec<(PathBuf, HirModule)> {
    let namespaces = loaded
        .iter()
        .map(|source| (source.path.clone(), namespace_of(&source.ast)))
        .collect::<Vec<_>>();
    let belongs_to = |name: &str, namespace: &str| {
        name == namespace || name.starts_with(&format!("{namespace}."))
    };
    let owner_for = |name: &str| {
        namespaces
            .iter()
            .filter(|(_, namespace)| belongs_to(name, namespace))
            .max_by_key(|(_, namespace)| namespace.len())
            .map(|(path, namespace)| (path.clone(), namespace.clone()))
    };

    let mut modules = Vec::new();
    for (path, namespace) in &namespaces {
        let funcs = hir
            .funcs
            .iter()
            .filter(|function| {
                owner_for(function.name.as_str()).is_some_and(|(owner, _)| owner == path.clone())
            })
            .cloned()
            .collect::<Vec<_>>();
        let consts = hir
            .consts
            .iter()
            .filter(|constant| {
                owner_for(constant.name.as_str()).is_some_and(|(owner, _)| owner == path.clone())
            })
            .cloned()
            .collect::<Vec<_>>();
        if funcs.is_empty() && consts.is_empty() {
            continue;
        }

        let mut externs = hir.externs.clone();
        let mut known_paths = externs
            .iter()
            .map(|external| match external {
                ori_hir::hir::HirExtern::Func { path, .. }
                | ori_hir::hir::HirExtern::Var { path, .. } => path.clone(),
            })
            .collect::<HashSet<_>>();
        for function in &hir.funcs {
            if funcs.iter().any(|owned| owned.name == function.name) {
                continue;
            }
            if known_paths.insert(function.name.clone()) {
                externs.push(ori_hir::hir::HirExtern::Func {
                    path: function.name.clone(),
                    name: ori_codegen::native_func_symbol(function.name.as_str()).into(),
                    params: function.params.clone(),
                    return_ty: function.return_ty.clone(),
                    abi: "ori".into(),
                    span: function.span,
                });
            }
            let wrapper_path = format!("{}.__fnptr_wrapper", function.name);
            if known_paths.insert(wrapper_path.clone().into()) {
                let mut wrapper_params = Vec::with_capacity(function.params.len() + 1);
                wrapper_params.push(ori_hir::hir::HirParam {
                    name: "__env".into(),
                    ty: Ty::Handle(Box::new(Ty::Void)),
                    default: None,
                    contract: None,
                    variadic: false,
                    span: function.span,
                });
                wrapper_params.extend(function.params.clone());
                externs.push(ori_hir::hir::HirExtern::Func {
                    path: wrapper_path.into(),
                    name: ori_codegen::native_func_wrapper_symbol(function.name.as_str()).into(),
                    params: wrapper_params,
                    return_ty: function.return_ty.clone(),
                    abi: "ori".into(),
                    span: function.span,
                });
            }
        }
        for constant in &hir.consts {
            if consts.iter().any(|owned| owned.name == constant.name) || !constant.is_public {
                continue;
            }
            if known_paths.insert(constant.name.clone()) {
                externs.push(ori_hir::hir::HirExtern::Var {
                    path: constant.name.clone(),
                    name: ori_codegen::native_global_symbol(constant.name.as_str()).into(),
                    ty: constant.ty.clone(),
                    abi: "ori".into(),
                    span: constant.span,
                });
            }
        }
        // A source module is compiled as an independent object. Visibility
        // has already been checked by the front end; exporting the lowered
        // function symbols here is only an object-file boundary detail and
        // lets generated helpers (generic/trait/closure shims) call across
        // module objects without relying on local-linkage visibility.
        let funcs = funcs
            .into_iter()
            .map(|mut function| {
                function.is_public = true;
                function
            })
            .collect();
        modules.push((
            path.clone(),
            HirModule {
                namespace: namespace.clone().into(),
                structs: hir.structs.clone(),
                enums: hir.enums.clone(),
                traits: hir.traits.clone(),
                trait_impls: hir.trait_impls.clone(),
                funcs,
                consts,
                externs,
            },
        ));
    }
    modules
}
