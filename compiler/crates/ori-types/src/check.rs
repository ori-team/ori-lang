use crate::def::{DefId, DefKind, DefMap};
use crate::literal::{parse_float_literal, parse_int_literal, NumericLiteralErrorKind};
use crate::lower::lower_type_with_aliases;
use crate::resolve::{
    import_aliases, DeprecatedSig, EnumSig, FuncSig, ImplSig, ReExport, StructSig, TraitSig,
    ValueSig, WhereConstraintSig,
};
use crate::ty::{expand_ty_aliases, substitute_ty_params};
use crate::ty::{OpaqueTy, Ty};
use ori_ast::common::{Attr, AttrArg, Name, QualifiedName, Visibility, WhereConstraint};
use ori_ast::expr::{Arg, ArgValue, BinaryOp, ClosureBody, Expr, FStrPart, UnaryOp};
use ori_ast::item::{
    ApplyDecl, ApplyMember, ExternBlock, FuncDecl, ImportDecl, ImportItem, Item, ItemWithAttrs,
    Param, ParamKind, SourceFile,
};
use ori_ast::pattern::Pattern;
use ori_ast::stmt::{Block, LValue, Stmt};
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label, Span};
use smol_str::SmolStr;
use std::collections::{HashMap, HashSet};

mod constraints;
mod match_exhaustiveness;

// ── Environment ───────────────────────────────────────────────────────────────

/// A lexical scope: maps variable names to their types.
#[derive(Debug, Default, Clone)]
struct Scope {
    vars: HashMap<SmolStr, Ty>,
    mutable: HashSet<SmolStr>,
    using_bindings: HashSet<SmolStr>,
}

impl Scope {
    fn bind(&mut self, name: SmolStr, ty: Ty) {
        self.bind_with_flags(name, ty, false, false);
    }

    fn bind_with_flags(&mut self, name: SmolStr, ty: Ty, mutable: bool, using_binding: bool) {
        if mutable {
            self.mutable.insert(name.clone());
        }
        if using_binding {
            self.using_bindings.insert(name.clone());
        }
        self.vars.insert(name, ty);
    }

    fn get(&self, name: &str) -> Option<&Ty> {
        self.vars.get(name)
    }

    fn contains(&self, name: &str) -> bool {
        self.vars.contains_key(name)
    }

    fn is_mutable(&self, name: &str) -> bool {
        self.mutable.contains(name)
    }

    fn is_using_binding(&self, name: &str) -> bool {
        self.using_bindings.contains(name)
    }
}

// ── Checker ───────────────────────────────────────────────────────────────────

pub struct Checker<'a> {
    def_map: &'a DefMap,
    func_sigs: &'a [FuncSig], // return types for all declared functions
    value_sigs: &'a [ValueSig],
    struct_sigs: &'a [StructSig],
    enum_sigs: &'a [EnumSig],
    trait_sigs: &'a [TraitSig],
    impl_sigs: &'a [ImplSig],
    deprecated_sigs: &'a [DeprecatedSig],
    reexports: &'a [ReExport],
    namespace: &'a str,
    file_id: FileId,
    sink: &'a mut DiagnosticSink,
    scopes: Vec<Scope>,
    aliases: HashMap<SmolStr, SmolStr>,
    used_aliases: HashSet<SmolStr>,
    current_return_ty: Option<Ty>,
    current_func_def_id: Option<DefId>,
    current_func_is_generic: bool,
    current_async_depth: usize,
    loop_depth: usize,
    closure_scope_roots: Vec<usize>,
    transferable_closure_depth: usize,
    current_where_constraints: Vec<WhereConstraintSig>,
    infer: HashMap<u32, Ty>,
    /// `DefId` -> `(arity, underlying_ty)` for each `type alias` declaration.
    type_alias_map: HashMap<DefId, (usize, Ty)>,
}

impl<'a> Checker<'a> {
    pub fn new(
        def_map: &'a DefMap,
        func_sigs: &'a [FuncSig],
        value_sigs: &'a [ValueSig],
        struct_sigs: &'a [StructSig],
        enum_sigs: &'a [EnumSig],
        trait_sigs: &'a [TraitSig],
        impl_sigs: &'a [ImplSig],
        type_alias_sigs: &[crate::resolve::TypeAliasSig],
        deprecated_sigs: &'a [DeprecatedSig],
        reexports: &'a [ReExport],
        namespace: &'a str,
        file_id: FileId,
        sink: &'a mut DiagnosticSink,
    ) -> Self {
        let type_alias_map: HashMap<DefId, (usize, Ty)> = type_alias_sigs
            .iter()
            .map(|s| (s.def_id, (s.type_params.len(), s.ty.clone())))
            .collect();
        Self {
            def_map,
            func_sigs,
            value_sigs,
            struct_sigs,
            enum_sigs,
            trait_sigs,
            impl_sigs,
            deprecated_sigs,
            reexports,
            namespace,
            file_id,
            sink,
            scopes: vec![Scope::default()],
            aliases: HashMap::new(),
            used_aliases: HashSet::new(),
            current_return_ty: None,
            current_func_def_id: None,
            current_func_is_generic: false,
            current_async_depth: 0,
            loop_depth: 0,
            closure_scope_roots: Vec::new(),
            transferable_closure_depth: 0,
            current_where_constraints: Vec::new(),
            infer: HashMap::new(),
            type_alias_map,
        }
    }

    /// Look up the return type of a function by its DefId.
    fn func_return_ty(&self, def_id: DefId) -> Option<Ty> {
        self.func_sigs
            .iter()
            .find(|s| s.def_id == def_id)
            .map(|s| s.return_ty.clone())
    }

    fn func_sig(&self, def_id: DefId) -> Option<FuncSig> {
        self.func_sigs.iter().find(|s| s.def_id == def_id).cloned()
    }

    fn value_ty(&self, def_id: DefId) -> Option<Ty> {
        self.value_sigs
            .iter()
            .find(|s| s.def_id == def_id)
            .map(|s| s.ty.clone())
    }

    fn struct_field_ty(&self, def_id: DefId, field: &str) -> Option<Ty> {
        self.struct_sigs
            .iter()
            .find(|s| s.def_id == def_id)
            .and_then(|s| s.fields.iter().find(|(name, _)| name == field))
            .map(|(_, ty)| ty.clone())
    }

    fn enum_sig(&self, def_id: DefId) -> Option<&EnumSig> {
        self.enum_sigs.iter().find(|s| s.def_id == def_id)
    }

    fn trait_sig(&self, def_id: DefId) -> Option<&TraitSig> {
        self.trait_sigs.iter().find(|s| s.def_id == def_id)
    }

    fn named_type_implements_trait(&self, type_def_id: DefId, trait_def_id: DefId) -> bool {
        self.impl_sigs
            .iter()
            .any(|sig| sig.type_def_id == type_def_id && sig.trait_def_id == trait_def_id)
    }

    fn iterable_impl_for_type(&self, type_def_id: DefId) -> Option<&ImplSig> {
        self.impl_sigs
            .iter()
            .filter(|sig| sig.type_def_id == type_def_id)
            .find(|sig| {
                self.def_map
                    .get(sig.trait_def_id)
                    .path
                    .ends_with(".Iterable")
            })
    }

    fn iterable_element_ty(&mut self, ty: &Ty, span: Span) -> Option<Ty> {
        if let Some(elem_ty) = elem_of(ty) {
            return Some(elem_ty);
        }
        if ty.is_error() {
            return None;
        }
        let Ty::Named(type_def_id, _) = ty else {
            self.emit_not_iterable(ty, span);
            return None;
        };
        let Some(impl_sig) = self.iterable_impl_for_type(*type_def_id) else {
            self.emit_not_iterable(ty, span);
            return None;
        };
        let Some(next_method) = impl_sig.methods.iter().find(|method| method.name == "next") else {
            self.sink.emit(
                Diagnostic::error(
                    "type.iterable_next_missing",
                    format!(
                        "`{}` implements `Iterable` but has no `next` method",
                        ty.display()
                    ),
                )
                .with_label(Label::primary(self.file_id, span, "iterated here"))
                .with_action("add `mut func next() -> optional[T]` to the Iterable implementation"),
            );
            return None;
        };
        let Some(next_sig) = self
            .func_sigs
            .iter()
            .find(|sig| sig.def_id == next_method.func_def_id)
        else {
            return None;
        };
        let self_ty = Ty::Named(*type_def_id, Vec::new());
        let valid_params =
            next_sig.params.len() == 1 && next_sig.params[0].is_assignable_to(&self_ty);
        if !next_sig.is_mut || !valid_params {
            self.sink.emit(
                Diagnostic::error(
                    "type.iterable_next_signature",
                    "`Iterable.next` must be `mut func next() -> optional[T]`",
                )
                .with_label(Label::primary(self.file_id, span, "iterated here"))
                .with_action("make `next` mutable and leave only the implicit `self` parameter"),
            );
            return None;
        }
        match &next_sig.return_ty {
            Ty::Optional(inner) => Some(*inner.clone()),
            other => {
                self.sink.emit(
                    Diagnostic::error(
                        "type.iterable_next_signature",
                        format!(
                            "`Iterable.next` must return `optional[T]`, found `{}`",
                            other.display()
                        ),
                    )
                    .with_label(Label::primary(self.file_id, span, "iterated here"))
                    .with_action("return `optional[T]` from `next`, using `some(value)` or `none`"),
                );
                None
            }
        }
    }

    fn emit_not_iterable(&mut self, ty: &Ty, span: Span) {
        self.sink.emit(
            Diagnostic::error(
                "type.not_iterable",
                format!("`for` needs an iterable value, found `{}`", ty.display()),
            )
            .with_label(Label::primary(self.file_id, span, "not iterable"))
            .with_action(
                "use a list, set, map, range, string, bytes, or implement `core.Iterable`",
            ),
        );
    }

    fn trait_methods_for_type(
        &self,
        type_def_id: DefId,
        method: &str,
    ) -> Vec<crate::resolve::TraitMethodSig> {
        let mut matches = Vec::new();
        for impl_sig in self
            .impl_sigs
            .iter()
            .filter(|sig| sig.type_def_id == type_def_id)
        {
            if let Some(method_sig) = self
                .trait_sig(impl_sig.trait_def_id)
                .and_then(|trait_sig| trait_sig.methods.iter().find(|sig| sig.name == method))
            {
                matches.push(method_sig.clone());
            }
        }
        matches
    }

    fn trait_method_for_type_param(
        &self,
        param_index: u32,
        param_name: &SmolStr,
        method: &str,
    ) -> Option<crate::resolve::TraitMethodSig> {
        let self_ty = Ty::Param {
            index: param_index,
            name: param_name.clone(),
        };
        let mut matches = Vec::new();
        for constraint in self.current_where_constraints.iter().filter(|constraint| {
            !constraint.negative
                && constraint.param_index == param_index
                && constraint.param_name == *param_name
        }) {
            let Some(method_sig) = self
                .trait_sig(constraint.trait_def_id)
                .and_then(|trait_sig| trait_sig.methods.iter().find(|sig| sig.name == method))
            else {
                continue;
            };
            let mut method_sig = method_sig.clone();
            method_sig.params = method_sig
                .params
                .iter()
                .map(|ty| substitute_trait_self(ty, constraint.trait_def_id, &self_ty))
                .collect();
            method_sig.return_ty =
                substitute_trait_self(&method_sig.return_ty, constraint.trait_def_id, &self_ty);
            matches.push(method_sig);
        }
        (matches.len() == 1).then(|| matches.remove(0))
    }

    pub fn check_file(&mut self, file: &SourceFile) {
        self.aliases.clear();
        self.used_aliases.clear();
        // Build the canonical alias map first — this is the map used for
        // resolution throughout the rest of type-checking.
        self.aliases = import_aliases(file, self.reexports);
        for import in &file.imports {
            if import.selected.is_empty() {
                continue;
            }
            let import_path = import.path.to_string();
            for item in &import.selected {
                let alias = selected_import_alias(item);
                let direct = format!("{}.{}", import_path, item.name.text);
                if self.def_map.lookup(&direct).is_some() {
                    self.aliases.insert(alias, smol_str::SmolStr::new(direct));
                }
            }
        }
        // Collect local top-level definition names for conflict detection
        let local_names: Vec<SmolStr> = file
            .items
            .iter()
            .filter_map(|i| match &i.item {
                Item::Func(f) => Some(f.name.text.clone()),
                Item::Struct(s) => Some(s.name.text.clone()),
                Item::Enum(e) => Some(e.name.text.clone()),
                Item::Const(c) => Some(c.name.text.clone()),
                Item::Var(v) => Some(v.name.text.clone()),
                _ => None,
            })
            .collect();
        // Check for duplicate aliases and shadows against local names.
        // We use a separate seen-set so that the diagnostic loop detects
        // duplicates independently of the canonical map (which silently
        // overwrites on duplicate keys).
        let mut seen_aliases: HashSet<SmolStr> = HashSet::new();
        let mut invalid_aliases: Vec<SmolStr> = Vec::new();
        for import in &file.imports {
            if !import.selected.is_empty() {
                self.check_selected_import_members(import);
            }
            for (alias, alias_span) in import_visible_bindings(import) {
                if !seen_aliases.insert(alias.clone()) {
                    self.sink.emit(
                        Diagnostic::error(
                            "bind.duplicate_alias",
                            format!(
                                "imported name `{}` is already used by another import",
                                alias
                            ),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            alias_span,
                            "duplicate imported name here",
                        ))
                        .with_action("rename one of the import aliases"),
                    );
                } else if local_names.contains(&alias) {
                    invalid_aliases.push(alias.clone());
                    self.sink.emit(
                        Diagnostic::error(
                            "bind.alias_shadows_local",
                            format!(
                                "imported name `{}` conflicts with a local definition of the same name",
                                alias
                            ),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            alias_span,
                            "imported name defined here",
                        ))
                        .with_action("rename the import alias or the local definition"),
                    );
                } else if is_reserved_type_alias_name(alias.as_str()) {
                    invalid_aliases.push(alias.clone());
                    self.sink.emit(
                        Diagnostic::error(
                            "bind.alias_shadows_builtin_type",
                            format!(
                                "imported name `{}` conflicts with a built-in type name",
                                alias
                            ),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            alias_span,
                            "imported name defined here",
                        ))
                        .with_action("use a plural alias such as `maps`, `sets`, or `tasks`"),
                    );
                }
            }
        }
        for alias in invalid_aliases {
            self.aliases.remove(&alias);
        }
        for item in &file.items {
            self.check_item_attrs(item);
            match &item.item {
                Item::Func(f) => self.check_func(f, &[], None),
                Item::Const(c) => {
                    let expected = self.lower(&c.ty, &[]);
                    self.check_collection_runtime_limits(&expected, c.ty.span());
                    self.check_expr_assignable_to(&c.value, &expected);
                }
                Item::Var(v) => {
                    let expected = self.lower(&v.ty, &[]);
                    self.check_collection_runtime_limits(&expected, v.ty.span());
                    self.check_expr_assignable_to(&v.value, &expected);
                }
                Item::Struct(s) => {
                    let tp: Vec<SmolStr> =
                        s.type_params.iter().map(|p| p.name.text.clone()).collect();
                    for field in &s.fields {
                        let expected = self.lower(&field.ty, &tp);
                        self.check_collection_runtime_limits(&expected, field.ty.span());
                        if let Some(contract) = field.contract.as_deref() {
                            self.push_scope();
                            self.bind(SmolStr::new("it"), expected);
                            let actual = self.infer_expr(contract);
                            self.expect_bool(&actual, contract.span());
                            self.pop_scope();
                        }
                    }
                    let previous_self = self
                        .aliases
                        .insert(SmolStr::new("Self"), s.name.text.clone());
                    let implicit_self_ty = self
                        .resolve_def_id(&s.name.text)
                        .map(|def_id| Ty::Named(def_id, Vec::new()));
                    for m in &s.methods {
                        self.check_func(m, &tp, implicit_self_ty.clone());
                    }
                    restore_alias(&mut self.aliases, "Self", previous_self);
                }
                Item::Apply(apply) => {
                    self.check_apply_decl(apply);
                    let tp: Vec<SmolStr> = apply
                        .type_params
                        .iter()
                        .map(|p| p.name.text.clone())
                        .collect();
                    let previous_self = self.aliases.insert(
                        SmolStr::new("Self"),
                        SmolStr::new(apply.for_type.to_string()),
                    );
                    let implicit_self_ty = self
                        .resolve_def_id(&apply.for_type.to_string())
                        .map(|def_id| Ty::Named(def_id, Vec::new()));
                    for member in &apply.free_members {
                        if let ApplyMember::Method(m) = member {
                            self.check_func(m, &tp, implicit_self_ty.clone());
                        }
                    }
                    for use_sec in &apply.uses {
                        for member in &use_sec.members {
                            if let ApplyMember::Method(m) = member {
                                self.check_func(m, &tp, implicit_self_ty.clone());
                            }
                        }
                    }
                    restore_alias(&mut self.aliases, "Self", previous_self);
                }
                Item::Trait(t) => {
                    let mut tp: Vec<SmolStr> =
                        t.type_params.iter().map(|p| p.name.text.clone()).collect();
                    for m in &t.members {
                        if let ori_ast::item::TraitMember::Type(name) = m {
                            tp.push(name.text.clone());
                        }
                    }
                    let previous_self = self
                        .aliases
                        .insert(SmolStr::new("Self"), t.name.text.clone());
                    let implicit_self_ty = self
                        .resolve_def_id(&t.name.text)
                        .map(|def_id| Ty::Named(def_id, Vec::new()));
                    for m in &t.members {
                        if let ori_ast::item::TraitMember::Default(func) = m {
                            self.check_func(func, &tp, implicit_self_ty.clone());
                        }
                    }
                    restore_alias(&mut self.aliases, "Self", previous_self);
                }
                Item::Extern(ext) => self.check_extern(ext),
                _ => {}
            }
        }
        // Emit warnings for unused imports
        for import in &file.imports {
            if import.visibility.is_public() {
                continue;
            }
            for (alias, alias_span) in import_visible_bindings(import) {
                if self.used_aliases.contains(&alias) {
                    continue;
                }
                self.sink.emit(
                    Diagnostic::warning(
                        "bind.unused_import",
                        format!(
                            "imported name `{}` from `{}` is never used",
                            alias, import.path
                        ),
                    )
                    .with_label(Label::primary(self.file_id, alias_span, "unused import"))
                    .with_action("remove this import item or use it"),
                );
            }
        }
    }

    fn check_selected_import_members(&mut self, import: &ImportDecl) {
        let _import_path = import.path.to_string();
        for item in &import.selected {
            let target = selected_import_target(import, item);
            let resolved = target.to_string();

            if let Some(def_id) = self.def_map.lookup(resolved.as_str()) {
                self.check_visibility(def_id, item.name.span);
                continue;
            }
            if stdlib_func_sig(resolved.as_str()).is_some()
                || stdlib_const_ty(resolved.as_str()).is_some()
                || stdlib_named_ty_exists(resolved.as_str())
            {
                continue;
            }
            self.sink.emit(
                Diagnostic::error(
                    "bind.import_member_unknown",
                    format!(
                        "module `{}` has no exported member `{}`",
                        import.path, item.name.text
                    ),
                )
                .with_label(Label::primary(
                    self.file_id,
                    item.name.span,
                    "selected import item here",
                ))
                .with_action("check the member name or import the module with an alias"),
            );
        }
    }

    fn check_extern(&mut self, ext: &ExternBlock) {
        for member in &ext.members {
            match member {
                ori_ast::item::ExternMember::Func {
                    params,
                    return_ty,
                    span,
                    ..
                } => {
                    for param in params {
                        let ty = self.lower(&param.ty, &[]);
                        self.check_extern_ffi_ty(&ty, param.ty.span(), "extern function parameter");
                    }
                    if let Some(return_ty) = return_ty {
                        let ty = self.lower(return_ty, &[]);
                        self.check_extern_ffi_ty(&ty, return_ty.span(), "extern function return");
                    } else {
                        let ty = Ty::Void;
                        self.check_extern_ffi_ty(&ty, *span, "extern function return");
                    }
                }
                ori_ast::item::ExternMember::Var { ty, span, .. } => {
                    let ty = self.lower(ty, &[]);
                    self.check_extern_ffi_ty(&ty, *span, "extern variable");
                }
            }
        }
    }

    fn check_extern_ffi_ty(&mut self, ty: &Ty, span: Span, position: &str) {
        if !ty.is_runtime_managed() {
            return;
        }
        self.sink.emit(
            Diagnostic::error(
                "extern.managed_type_in_ffi",
                format!("{position} uses managed type `{}`", ty.display()),
            )
            .with_label(Label::primary(self.file_id, span, "managed FFI type here"))
            .with_why("managed Ori values have ARC ownership and runtime layout that cannot cross raw FFI directly")
            .with_action("use primitive numeric types, bool, or an explicit unmanaged handle at the FFI boundary"),
        );
    }

    fn check_item_attrs(&mut self, item: &ItemWithAttrs) {
        let target = item_target_name(&item.item);
        let mut seen: HashMap<&str, ori_diagnostics::Span> = HashMap::new();

        for attr in &item.attrs {
            let name = attr.name.text.as_str();
            if !is_known_attr(name) {
                self.sink.emit(
                    Diagnostic::error(
                        "attr.unknown",
                        format!("unknown attribute `@{}`", attr.name.text),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        attr.name.span,
                        "unknown attribute",
                    ))
                    .with_action(
                        "use one of `@test`, `@deprecated`, `@inline`, `@no_inline`, `@cfg`, `@repr`, or `@c_export`",
                    ),
                );
                continue;
            }

            if let Some(_first_span) = seen.insert(name, attr.name.span) {
                self.sink.emit(
                    Diagnostic::warning(
                        "attr.duplicate",
                        format!("attribute `@{}` is repeated", attr.name.text),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        attr.name.span,
                        "repeated here",
                    ))
                    .with_note("only one attribute with the same name is needed")
                    .with_action("remove the repeated attribute"),
                );
            }

            if !attr_applies_to(name, target) {
                self.sink.emit(
                    Diagnostic::error(
                        "attr.invalid_target",
                        format!(
                            "attribute `@{}` cannot be used on {}",
                            attr.name.text, target
                        ),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        attr.name.span,
                        "invalid target",
                    ))
                    .with_action(attr_target_action(name)),
                );
            }

            if !attr_args_valid(name, attr) {
                self.sink.emit(
                    Diagnostic::error(
                        "attr.invalid_arg",
                        format!("attribute `@{}` has invalid arguments", attr.name.text),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        attr.span,
                        "invalid attribute arguments",
                    ))
                    .with_action(attr_arg_action(name)),
                );
            }

            if name == "c_export" {
                self.check_c_export_attr(item, attr);
            }
        }
    }

    /// Validate `@c_export` on a free function: must be `public` with FFI-safe types.
    fn check_c_export_attr(&mut self, item: &ItemWithAttrs, attr: &Attr) {
        let Item::Func(func) = &item.item else {
            return;
        };
        if func.visibility != Visibility::Public {
            self.sink.emit(
                Diagnostic::error(
                    "attr.c_export_not_public",
                    "`@c_export` requires a `public` function",
                )
                .with_label(Label::primary(self.file_id, attr.name.span, "export here"))
                .with_action("add `public` to the function declaration"),
            );
        }
        if func.is_async {
            self.sink.emit(
                Diagnostic::error(
                    "attr.c_export_async",
                    "`@c_export` cannot be used on async functions",
                )
                .with_label(Label::primary(self.file_id, attr.name.span, "async export"))
                .with_action("export a synchronous wrapper instead"),
            );
        }
        if !func.type_params.is_empty() {
            self.sink.emit(
                Diagnostic::error(
                    "attr.c_export_generic",
                    "`@c_export` cannot be used on generic functions",
                )
                .with_label(Label::primary(
                    self.file_id,
                    attr.name.span,
                    "generic export",
                )),
            );
        }
        // Signature check uses resolved types from a lightweight lower of annotations.
        for param in &func.params {
            let ty = self.lower(&param.ty, &[]);
            if !is_c_export_ffi_ty(&ty) {
                self.sink.emit(
                    Diagnostic::error(
                        "attr.c_export_bad_type",
                        format!(
                            "`@c_export` parameter `{}` has non-FFI-safe type `{}`",
                            param.name.text,
                            ty.display()
                        ),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        param.name.span,
                        "parameter type",
                    ))
                    .with_action(
                        "use `int`, `float`, `bool`, or `void` return; phase-2: ptr+len for strings",
                    ),
                );
            }
        }
        if let Some(ret) = &func.return_ty {
            let ty = self.lower(ret, &[]);
            if !is_c_export_ffi_ty(&ty) && !matches!(ty, Ty::Void) {
                self.sink.emit(
                    Diagnostic::error(
                        "attr.c_export_bad_type",
                        format!("`@c_export` return type `{}` is not FFI-safe", ty.display()),
                    )
                    .with_label(Label::primary(self.file_id, ret.span(), "return type"))
                    .with_action("use `int`, `float`, `bool`, or omit for void"),
                );
            }
        }
    }

    fn check_func(&mut self, func: &FuncDecl, outer_tp: &[SmolStr], implicit_self_ty: Option<Ty>) {
        let mut tp = outer_tp.to_vec();
        tp.extend(func.type_params.iter().map(|p| p.name.text.clone()));
        let has_implicit_self = implicit_self_ty.is_some();
        self.push_scope();
        if !has_explicit_self_param(&func.params) {
            if let Some(self_ty) = implicit_self_ty {
                let self_name = Name::new("self", func.name.span);
                self.bind_checked(&self_name, self_ty, func.is_mut, false);
            }
        }
        let param_tys: Vec<Ty> = func
            .params
            .iter()
            .map(|param| param_binding_ty(param, self.lower(&param.ty, &tp)))
            .collect();
        for (param, ty) in func.params.iter().zip(param_tys.iter()) {
            self.check_collection_runtime_limits(ty, param.ty.span());
        }
        // Bind parameters into scope
        for (param, ty) in func.params.iter().zip(param_tys.iter()) {
            let mutable_self = func.is_mut && param.name.text == "self";
            self.bind_checked(&param.name, ty.clone(), mutable_self, false);
        }
        for (param, expected) in func.params.iter().zip(param_tys.iter()) {
            if let Some(default) = param_default_expr(&param.kind) {
                let actual = self.infer_expr(default);
                self.expect_assignable(&actual, expected, default.span());
            }
        }
        for (param, expected) in func.params.iter().zip(param_tys.iter()) {
            if let Some(contract) = param_contract_expr(&param.kind) {
                self.push_scope();
                self.bind(SmolStr::new("it"), expected.clone());
                self.bind(param.name.text.clone(), expected.clone());
                let actual = self.infer_expr(contract);
                self.expect_bool(&actual, contract.span());
                self.pop_scope();
            }
        }
        let expected_ret = func
            .return_ty
            .as_ref()
            .map(|t| self.lower(t, &tp))
            .unwrap_or(Ty::Void);
        if let Some(return_ty) = &func.return_ty {
            self.check_collection_runtime_limits(&expected_ret, return_ty.span());
        }
        let prev_ret_ty = self.current_return_ty.take();
        let prev_func_def_id = self.current_func_def_id;
        let prev_func_is_generic = self.current_func_is_generic;
        let prev_async_depth = self.current_async_depth;
        let current_where_constraints = self.lower_current_where_constraints(func, &tp);
        let prev_where_constraints = std::mem::replace(
            &mut self.current_where_constraints,
            current_where_constraints,
        );
        self.current_return_ty = Some(expected_ret.clone());
        self.current_func_def_id = if !has_implicit_self && !func.type_params.is_empty() {
            self.resolve_def_id(func.name.text.as_str())
        } else {
            None
        };
        self.current_func_is_generic = !func.type_params.is_empty();
        if func.is_async {
            self.current_async_depth += 1;
        }
        self.check_block(&func.body, &expected_ret, &tp);
        self.current_async_depth = prev_async_depth;
        if expected_ret != Ty::Void && !block_definitely_returns(&func.body) {
            self.sink.emit(
                Diagnostic::error(
                    "type.missing_return",
                    format!(
                        "function `{}` may finish without returning `{}`",
                        func.name.text,
                        expected_ret.display()
                    ),
                )
                .with_label(Label::primary(
                    self.file_id,
                    func.body.span,
                    "not all paths return",
                ))
                .with_action("return a value on every path or change the return type to `void`"),
            );
        }
        self.current_return_ty = prev_ret_ty;
        self.current_func_def_id = prev_func_def_id;
        self.current_func_is_generic = prev_func_is_generic;
        self.current_where_constraints = prev_where_constraints;
        self.pop_scope();
    }

    fn lower_current_where_constraints(
        &mut self,
        func: &FuncDecl,
        type_params: &[SmolStr],
    ) -> Vec<WhereConstraintSig> {
        let Some(where_clause) = &func.where_clause else {
            return Vec::new();
        };
        where_clause
            .constraints
            .iter()
            .filter_map(|constraint| {
                let (param, bound, negative) = match constraint {
                    WhereConstraint::Is { param, bound, .. } => (param, bound, false),
                    WhereConstraint::IsNot { param, bound, .. } => (param, bound, true),
                };
                let param_index = type_params
                    .iter()
                    .position(|name| name == &param.text)
                    .map(|index| index as u32)?;
                let mut trait_def_id = self.resolve_def_id(&bound.to_string())?;
                // Follow type aliases to find the actual trait.
                // e.g. `type MyEq = ori.core.Equatable` → resolve to Equatable.
                trait_def_id = self.resolve_trait_through_aliases(trait_def_id)?;
                if self.def_map.get(trait_def_id).kind != DefKind::Trait {
                    return None;
                }
                Some(WhereConstraintSig {
                    param_index,
                    param_name: param.text.clone(),
                    trait_def_id,
                    negative,
                })
            })
            .collect()
    }

    /// Follow a chain of type aliases to find the underlying trait DefId.
    /// Returns None if the chain doesn't resolve to a trait.
    fn resolve_trait_through_aliases(&self, mut def_id: DefId) -> Option<DefId> {
        // Limit chain depth to prevent infinite loops (max 16 hops).
        for _ in 0..16 {
            let def = self.def_map.get(def_id);
            if def.kind == DefKind::Trait {
                return Some(def_id);
            }
            if def.kind != DefKind::TypeAlias {
                return None;
            }
            // Look up the alias's underlying type.
            let (arity, underlying) = self.type_alias_map.get(&def_id)?;
            match underlying {
                Ty::Named(target_id, args) if args.is_empty() || *arity > 0 => {
                    // For generic aliases with args, the where constraint should
                    // already have concrete types. For non-generic aliases, just
                    // follow to the target.
                    if *arity == 0 {
                        def_id = *target_id;
                    } else {
                        // Generic alias used without type args in where clause.
                        // This is invalid — where clause needs concrete trait.
                        return None;
                    }
                }
                _ => return None,
            }
        }
        None // chain too long
    }

    fn check_apply_decl(&mut self, apply: &ApplyDecl) {
        let type_name = apply.for_type.to_string();
        let Some(type_def_id) = self.resolve_def_id(&type_name) else {
            self.sink.emit(
                Diagnostic::error(
                    "impl.type_not_found",
                    format!("type `{}` was not found", type_name),
                )
                .with_label(Label::primary(
                    self.file_id,
                    apply.for_type.span,
                    "type used here",
                ))
                .with_action("define or import this type before applying traits to it"),
            );
            return;
        };
        if !matches!(
            self.def_map.get(type_def_id).kind,
            DefKind::Struct | DefKind::Enum | DefKind::TypeAlias
        ) {
            self.sink.emit(
                Diagnostic::error(
                    "impl.type_not_found",
                    format!("`{}` is not a concrete type", type_name),
                )
                .with_label(Label::primary(
                    self.file_id,
                    apply.for_type.span,
                    "not a type",
                ))
                .with_action("apply traits to a struct, enum, or type alias"),
            );
            return;
        }

        let self_ty = Ty::Named(type_def_id, Vec::new());
        for use_sec in &apply.uses {
            self.check_apply_use_section(apply, use_sec, type_def_id, &self_ty, &type_name);
        }
    }

    fn check_apply_use_section(
        &mut self,
        apply: &ApplyDecl,
        use_sec: &ori_ast::item::ApplyUseSection,
        type_def_id: DefId,
        self_ty: &Ty,
        type_name: &str,
    ) {
        let trait_name = use_sec.trait_name.to_string();
        let Some(trait_def_id) = self.resolve_def_id(&trait_name) else {
            self.sink.emit(
                Diagnostic::error(
                    "impl.trait_not_found",
                    format!("trait `{}` was not found", trait_name),
                )
                .with_label(Label::primary(
                    self.file_id,
                    use_sec.trait_name.span,
                    "trait used here",
                ))
                .with_action("define or import this trait before using it in `apply`"),
            );
            return;
        };
        if self.def_map.get(trait_def_id).kind != DefKind::Trait {
            self.sink.emit(
                Diagnostic::error(
                    "impl.trait_not_found",
                    format!("`{}` is not a trait", trait_name),
                )
                .with_label(Label::primary(
                    self.file_id,
                    use_sec.trait_name.span,
                    "not a trait",
                ))
                .with_action("use the name of a trait in `use Trait`"),
            );
            return;
        }

        let Some(trait_sig) = self.trait_sig(trait_def_id).cloned() else {
            return;
        };
        let _ = type_def_id;

        for expected in trait_sig.methods {
            let provided = use_sec
                .members
                .iter()
                .find(|m| m.slot_name().text == expected.name);
            let Some(member) = provided else {
                if !expected.has_default {
                    self.sink.emit(
                        Diagnostic::error(
                            "impl.missing_method",
                            format!(
                                "apply `{}` / use `{}` is missing method `{}`",
                                type_name, trait_name, expected.name
                            ),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            use_sec.span,
                            "use section here",
                        ))
                        .with_action(format!(
                            "add `{}` (inline body or `{} = freeFunction` bind)",
                            expected.name, expected.name
                        )),
                    );
                }
                continue;
            };

            match member {
                ApplyMember::Method(method) => {
                    if method.is_mut != expected.is_mut {
                        self.sink.emit(
                            Diagnostic::error(
                                "impl.mut_mismatch",
                                format!(
                                    "method `{}` has a different mutability than the trait requires",
                                    expected.name
                                ),
                            )
                            .with_label(Label::primary(
                                self.file_id,
                                method.span,
                                "implemented here",
                            ))
                            .with_label(Label::primary(
                                self.file_id,
                                expected.span,
                                "trait method declared here",
                            ))
                            .with_action("make both declarations use the same `mut` form"),
                        );
                    }

                    let (actual_params, actual_return) =
                        self.lower_apply_method_signature(method, apply);
                    let expected_params: Vec<Ty> = expected
                        .params
                        .iter()
                        .map(|ty| substitute_trait_self(ty, trait_def_id, self_ty))
                        .collect();
                    let expected_return =
                        substitute_trait_self(&expected.return_ty, trait_def_id, self_ty);
                    if actual_params != expected_params || actual_return != expected_return {
                        self.sink.emit(
                            Diagnostic::error(
                                "impl.wrong_signature",
                                format!(
                                    "method `{}` does not match the trait signature",
                                    expected.name
                                ),
                            )
                            .with_label(Label::primary(
                                self.file_id,
                                method.span,
                                "implemented signature here",
                            ))
                            .with_label(Label::primary(
                                self.file_id,
                                expected.span,
                                "trait signature here",
                            ))
                            .with_why(format!(
                                "expected `({}) -> {}`, found `({}) -> {}`",
                                display_tys(&expected_params),
                                expected_return.display(),
                                display_tys(&actual_params),
                                actual_return.display(),
                            ))
                            .with_action(
                                "change the implementation method signature to match the trait",
                            ),
                        );
                    }
                }
                ApplyMember::Bind { slot, target, span } => {
                    // Signature check against the free function target.
                    let target_path = format!("{}.{}", self.namespace, target.text);
                    let Some(target_def_id) = self.def_map.lookup(&target_path) else {
                        // resolve already emitted name.undefined
                        continue;
                    };
                    let Some(sig) = self.func_sig(target_def_id) else {
                        continue;
                    };
                    let expected_params: Vec<Ty> = expected
                        .params
                        .iter()
                        .map(|ty| substitute_trait_self(ty, trait_def_id, self_ty))
                        .collect();
                    let expected_return =
                        substitute_trait_self(&expected.return_ty, trait_def_id, self_ty);
                    if sig.params != expected_params || sig.return_ty != expected_return {
                        self.sink.emit(
                            Diagnostic::error(
                                "impl.wrong_signature",
                                format!(
                                    "bind `{} = {}` does not match the trait signature",
                                    slot.text, target.text
                                ),
                            )
                            .with_label(Label::primary(self.file_id, *span, "bind here"))
                            .with_label(Label::primary(
                                self.file_id,
                                expected.span,
                                "trait signature here",
                            ))
                            .with_why(format!(
                                "expected `({}) -> {}`, found `({}) -> {}`",
                                display_tys(&expected_params),
                                expected_return.display(),
                                display_tys(&sig.params),
                                sig.return_ty.display(),
                            ))
                            .with_action(
                                "bind to a free function whose signature matches the trait method",
                            ),
                        );
                    }
                    if sig.is_mut != expected.is_mut {
                        self.sink.emit(
                            Diagnostic::error(
                                "impl.mut_mismatch",
                                format!(
                                    "bind target `{}` has a different mutability than the trait requires",
                                    target.text
                                ),
                            )
                            .with_label(Label::primary(self.file_id, *span, "bind here"))
                            .with_action("bind to a free function with matching `mut`"),
                        );
                    }
                }
            }
        }
    }

    fn lower_apply_method_signature(
        &mut self,
        method: &FuncDecl,
        apply: &ApplyDecl,
    ) -> (Vec<Ty>, Ty) {
        let mut tp: Vec<SmolStr> = apply
            .type_params
            .iter()
            .map(|p| p.name.text.clone())
            .collect();
        tp.extend(method.type_params.iter().map(|p| p.name.text.clone()));
        let previous_self = self.aliases.insert(
            SmolStr::new("Self"),
            SmolStr::new(apply.for_type.to_string()),
        );
        let mut params: Vec<Ty> = method
            .params
            .iter()
            .map(|p| self.lower(&p.ty, &tp))
            .collect();
        if !has_explicit_self_param(&method.params) {
            let self_ty = self
                .resolve_def_id(&apply.for_type.to_string())
                .map(|def_id| Ty::Named(def_id, Vec::new()))
                .unwrap_or(Ty::Infer(0));
            params.insert(0, self_ty);
        }
        let return_ty = method
            .return_ty
            .as_ref()
            .map(|ty| self.lower(ty, &tp))
            .unwrap_or(Ty::Void);
        restore_alias(&mut self.aliases, "Self", previous_self);
        (params, return_ty)
    }

    fn check_block(&mut self, block: &Block, expected_ret: &Ty, tp: &[SmolStr]) {
        self.push_scope();
        for stmt in &block.stmts {
            self.check_stmt(stmt, expected_ret, tp);
        }
        self.pop_scope();
    }

    /// Resolve the type of a local `const`/`var` binding.
    ///
    /// With an explicit annotation, behave as before. Without one (`0.3.1` +
    /// option B): accept RHS whose type is obvious on the same line — literals,
    /// typed struct/list/map forms, **field/index on a typed receiver**, and
    /// **calls (including `|>`) with a known return type**. Reject `try`, empty
    /// collections, bare `none`, `void` results, and non-concrete types.
    fn resolve_local_binding_ty(
        &mut self,
        name: &Name,
        ann: Option<&ori_ast::ty::Type>,
        value: &Expr,
        tp: &[SmolStr],
    ) -> Ty {
        if let Some(ast_ty) = ann {
            let ann_ty = self.lower(ast_ty, tp);
            self.check_collection_runtime_limits(&ann_ty, ast_ty.span());
            self.check_expr_assignable_to(value, &ann_ty);
            return ann_ty;
        }

        if expr_blocks_local_inference(value) {
            self.emit_local_inference_failed(
                name,
                value.span(),
                "this expression is not a candidate for local type inference",
            );
            let _ = self.infer_expr(value);
            return Ty::Error;
        }

        let inferred = if expr_needs_expected_context(value) {
            // Anon struct / empty collection / bare `.Variant` need a written type.
            self.emit_local_inference_failed(
                name,
                value.span(),
                "this value needs an expected type; write an explicit annotation",
            );
            let _ = self.infer_expr(value);
            return Ty::Error;
        } else {
            self.infer_expr(value)
        };

        if !ty_is_locally_inferable(&inferred) {
            self.emit_local_inference_failed(
                name,
                value.span(),
                &format!(
                    "inferred type `{}` is not obvious enough to omit the annotation",
                    inferred.display()
                ),
            );
            return Ty::Error;
        }

        self.check_collection_runtime_limits(&inferred, value.span());
        inferred
    }

    fn emit_local_inference_failed(&mut self, name: &Name, span: Span, why: &str) {
        self.sink.emit(
            Diagnostic::error(
                "type.local_inference_failed",
                format!(
                    "cannot infer type for local binding `{}`; write an explicit type annotation",
                    name.text
                ),
            )
            .with_label(Label::primary(self.file_id, span, "value here"))
            .with_why(why.to_string())
            .with_action(format!(
                "write `const {}: T = ...` or `var {}: T = ...` with an explicit type",
                name.text, name.text
            )),
        );
    }

    fn check_stmt(&mut self, stmt: &Stmt, expected_ret: &Ty, tp: &[SmolStr]) {
        match stmt {
            Stmt::Const(c) => {
                let ty = self.resolve_local_binding_ty(&c.name, c.ty.as_ref(), &c.value, tp);
                self.bind_checked(&c.name, ty, false, false);
            }
            Stmt::Var(v) => {
                let ty = self.resolve_local_binding_ty(&v.name, v.ty.as_ref(), &v.value, tp);
                self.bind_checked(&v.name, ty, true, false);
            }
            Stmt::Return(r) => {
                let ret_ty = r.value.as_ref().map_or(Ty::Void, |e| {
                    if expr_needs_expected_context(e) {
                        self.check_expr_assignable_to(e, expected_ret)
                    } else {
                        self.infer_expr(e)
                    }
                });
                if !self.unify(&ret_ty, expected_ret) {
                    let span = r.value.as_ref().map(|e| e.span()).unwrap_or(r.span);
                    self.sink.emit(
                        Diagnostic::error(
                            "type.return_mismatch",
                            format!(
                                "return type `{}` does not match declared `{}`",
                                ret_ty.display(),
                                expected_ret.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, span, "returned here"))
                        .with_why(format!(
                            "function declares return type `{}`",
                            expected_ret.display()
                        ))
                        .with_action("adjust the returned expression or the function return type"),
                    );
                }
            }
            Stmt::If(i) => {
                let cond_ty = self.infer_expr(&i.condition);
                self.expect_bool(&cond_ty, i.condition.span());
                self.check_block(&i.then_block, expected_ret, tp);
                for (cond, block) in &i.else_ifs {
                    let c = self.infer_expr(cond);
                    self.expect_bool(&c, cond.span());
                    self.check_block(block, expected_ret, tp);
                }
                if let Some(eb) = &i.else_block {
                    self.check_block(eb, expected_ret, tp);
                }
            }
            Stmt::While(w) => {
                let cond_ty = self.infer_expr(&w.condition);
                self.expect_bool(&cond_ty, w.condition.span());
                self.loop_depth += 1;
                self.check_block(&w.body, expected_ret, tp);
                self.loop_depth -= 1;
            }
            Stmt::For(f) => {
                let iter_ty = self.infer_expr(&f.iterable);
                let elem_ty = self
                    .iterable_element_ty(&iter_ty, f.iterable.span())
                    .unwrap_or(Ty::Error);
                let second_ty = if elem_of(&iter_ty).is_none() && !elem_ty.is_error() {
                    Ty::Int
                } else {
                    for_second_binding_ty(&iter_ty)
                };
                self.push_scope();
                self.bind_checked(&f.binding, elem_ty, false, false);
                if let Some(idx) = &f.second_binding {
                    self.bind_checked(idx, second_ty, false, false);
                }
                self.loop_depth += 1;
                self.check_block(&f.body, expected_ret, tp);
                self.loop_depth -= 1;
                self.pop_scope();
            }
            Stmt::Loop(l) => {
                self.loop_depth += 1;
                self.check_block(&l.body, expected_ret, tp);
                self.loop_depth -= 1;
            }
            Stmt::Repeat(r) => {
                let count_ty = self.infer_expr(&r.count);
                if !count_ty.is_error() && !count_ty.contains_infer() && count_ty != Ty::Int {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.repeat_count_not_int",
                            format!("repeat count must be `int`, found `{}`", count_ty.display()),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            r.count.span(),
                            "expected int here",
                        ))
                        .with_action("use an integer expression for the repeat count"),
                    );
                }
                self.loop_depth += 1;
                self.check_block(&r.body, expected_ret, tp);
                self.loop_depth -= 1;
            }
            Stmt::Match(m) => {
                let scr_ty = self.infer_expr(&m.scrutinee);
                self.check_match_duplicate_cases(&scr_ty, &m.cases);
                self.check_match_unreachable_cases(&scr_ty, &m.cases);
                for case in &m.cases {
                    match case {
                        ori_ast::stmt::MatchCase::Pattern {
                            pattern,
                            guard,
                            body,
                            ..
                        } => {
                            self.push_scope();
                            self.check_pattern_type(pattern, &scr_ty);
                            if let Some(guard) = guard {
                                let guard_ty = self.infer_expr(guard);
                                self.expect_bool(&guard_ty, guard.span());
                            }
                            for s in body {
                                self.check_stmt(s, expected_ret, tp);
                            }
                            self.pop_scope();
                        }
                        ori_ast::stmt::MatchCase::Else { body, .. } => {
                            self.push_scope();
                            for s in body {
                                self.check_stmt(s, expected_ret, tp);
                            }
                            self.pop_scope();
                        }
                    }
                }
                self.check_match_exhaustiveness(&scr_ty, &m.cases, m.span);
            }
            Stmt::Check(c) => {
                let cond_ty = self.infer_expr(&c.condition);
                self.expect_bool(&cond_ty, c.condition.span());
            }
            Stmt::Expr(e) => {
                let ty = self.infer_expr(e);
                self.warn_unused_result(&ty, e.span());
            }
            Stmt::IfSome(s) => {
                let val_ty = self.infer_expr(&s.value);
                let inner_ty = match &val_ty {
                    Ty::Optional(t) => *t.clone(),
                    _ if val_ty.is_error() || val_ty.contains_infer() => Ty::Infer(0),
                    _ => {
                        self.sink.emit(
                            Diagnostic::error(
                                "type.ifsome_not_optional",
                                format!(
                                    "`if some` requires an `optional[T]`, found `{}`",
                                    val_ty.display()
                                ),
                            )
                            .with_label(Label::primary(
                                self.file_id,
                                s.value.span(),
                                "expected optional here",
                            ))
                            .with_action("change the expression to return `optional[T]`"),
                        );
                        Ty::Error
                    }
                };
                self.push_scope();
                self.bind_checked(&s.binding, inner_ty, false, false);
                self.check_block(&s.then_block, expected_ret, tp);
                self.pop_scope();
                if let Some(eb) = &s.else_block {
                    self.check_block(eb, expected_ret, tp);
                }
            }
            Stmt::WhileSome(s) => {
                let val_ty = self.infer_expr(&s.value);
                let inner_ty = match &val_ty {
                    Ty::Optional(t) => *t.clone(),
                    _ if val_ty.is_error() || val_ty.contains_infer() => Ty::Infer(0),
                    _ => {
                        self.sink.emit(
                            Diagnostic::error(
                                "type.whilesome_not_optional",
                                format!(
                                    "`while some` requires an `optional[T]`, found `{}`",
                                    val_ty.display()
                                ),
                            )
                            .with_label(Label::primary(
                                self.file_id,
                                s.value.span(),
                                "expected optional here",
                            ))
                            .with_action("change the expression to return `optional[T]`"),
                        );
                        Ty::Error
                    }
                };
                self.push_scope();
                self.bind_checked(&s.binding, inner_ty, false, false);
                self.loop_depth += 1;
                self.check_block(&s.body, expected_ret, tp);
                self.loop_depth -= 1;
                self.pop_scope();
            }
            Stmt::Using(u) => {
                // Native async codegen emits `dispose()` via `emit_scope_cleanup_calls_from`
                // and `emit_async_frame_dispose_live_values` on all terminal paths (return,
                // propagate, cancel, fail). See `emit_async_terminal_cleanup` in native_backend.
                let ann_ty = self.lower(&u.ty, tp);
                self.check_collection_runtime_limits(&ann_ty, u.ty.span());
                // S3 context-typed `{…}` / `.Variant` need expected type on the RHS.
                self.check_expr_assignable_to(&u.value, &ann_ty);
                self.check_disposable_using(&ann_ty, u.name.span);
                self.bind_checked(&u.name, ann_ty, false, true);
            }
            Stmt::Assign(a) => {
                self.check_lvalue_mutable(&a.lvalue);
                let lhs_ty = self.infer_lvalue_ty(&a.lvalue);
                // Push expected type so context-typed struct/enum shorthand works on assign.
                self.check_expr_assignable_to(&a.value, &lhs_ty);
            }
            Stmt::CompoundAssign(c) => {
                let rhs_ty = self.infer_expr(&c.value);
                self.check_lvalue_mutable(&c.lvalue);
                let lhs_ty = self.infer_lvalue_ty(&c.lvalue);
                // Both sides should be the same numeric type
                self.expect_assignable(&rhs_ty, &lhs_ty, c.value.span());
            }
            Stmt::Break(span) => self.check_loop_control("break", *span),
            Stmt::Continue(span) => self.check_loop_control("continue", *span),
        }
    }

    // ── Expression type inference ─────────────────────────────────────────────

    fn check_loop_control(&mut self, keyword: &str, span: ori_diagnostics::Span) {
        if self.loop_depth > 0 {
            return;
        }
        self.sink.emit(
            Diagnostic::error(
                "control.loop_required",
                format!("`{keyword}` can only be used inside a loop"),
            )
            .with_label(Label::primary(
                self.file_id,
                span,
                format!("`{keyword}` used here"),
            ))
            .with_action(format!(
                "move this `{keyword}` inside a `while`, `for`, `repeat`, or `loop` block"
            )),
        );
    }

    fn emit_numeric_literal_error(
        &mut self,
        span: ori_diagnostics::Span,
        message: &str,
        kind: NumericLiteralErrorKind,
    ) {
        let code = match kind {
            NumericLiteralErrorKind::Invalid => "type.numeric_literal_invalid",
            NumericLiteralErrorKind::OutOfRange => "type.numeric_literal_out_of_range",
        };
        self.sink.emit(
            Diagnostic::error(code, message)
                .with_label(Label::primary(self.file_id, span, "numeric literal here"))
                .with_action("use a value that fits the literal suffix or target type"),
        );
    }

    fn check_expr_assignable_to(&mut self, expr: &Expr, expected: &Ty) -> Ty {
        match expr {
            Expr::AnonStructLit { fields, span } => {
                self.check_anon_struct_literal(fields, expected, *span)
            }
            Expr::StructLit { ty, fields, span } => {
                let actual = self.check_struct_literal(ty, fields, *span);
                self.expect_assignable(&actual, expected, *span);
                actual
            }
            Expr::EnumVariantUnit { ty, variant, span } => {
                let actual = self.check_enum_variant_unit(ty.as_ref(), variant, expected, *span);
                self.expect_assignable(&actual, expected, *span);
                actual
            }
            Expr::EnumVariantNamed {
                ty,
                variant,
                fields,
                span,
            } => {
                let actual =
                    self.check_enum_variant_named(ty.as_ref(), variant, fields, expected, *span);
                self.expect_assignable(&actual, expected, *span);
                actual
            }
            Expr::IfExpr {
                condition,
                then_expr,
                else_expr,
                ..
            } if expr_needs_expected_context(then_expr)
                || expr_needs_expected_context(else_expr)
                || expr_needs_expected_context(expr) =>
            {
                let cond_ty = self.infer_expr(condition);
                self.expect_bool(&cond_ty, condition.span());
                self.check_expr_assignable_to(then_expr, expected);
                self.check_expr_assignable_to(else_expr, expected);
                expected.clone()
            }
            // Push the expected type into every arm, so arms that need context
            // (anonymous struct literals, bare enum variants, `none`, …) get it.
            Expr::MatchExpr {
                scrutinee,
                arms,
                span,
            } => {
                let scr_ty = self.infer_expr(scrutinee);
                let cases = match_expr_arms_as_cases(arms);
                self.check_match_duplicate_cases(&scr_ty, &cases);
                self.check_match_unreachable_cases(&scr_ty, &cases);
                for arm in arms {
                    self.push_scope();
                    if let Some(pattern) = &arm.pattern {
                        self.check_pattern_type(pattern, &scr_ty);
                    }
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.infer_expr(guard);
                        self.expect_bool(&guard_ty, guard.span());
                    }
                    self.check_expr_assignable_to(&arm.body, expected);
                    self.pop_scope();
                }
                self.check_match_exhaustiveness(&scr_ty, &cases, *span);
                expected.clone()
            }
            _ => {
                let actual = self.infer_expr(expr);
                self.expect_assignable(&actual, expected, expr.span());
                actual
            }
        }
    }

    fn check_anon_struct_literal(
        &mut self,
        fields: &[ori_ast::expr::FieldInit],
        expected: &Ty,
        span: ori_diagnostics::Span,
    ) -> Ty {
        let Some(expected_fields) = self.expected_struct_fields(expected) else {
            self.emit_anon_struct_type_unknown(expected, span);
            for field in fields {
                self.infer_expr(&field.value);
            }
            return Ty::Error;
        };

        let mut provided: HashMap<SmolStr, ori_diagnostics::Span> = HashMap::new();

        for field in fields {
            if provided
                .insert(field.name.text.clone(), field.name.span)
                .is_some()
            {
                self.sink.emit(
                    Diagnostic::error(
                        "type.anon_struct_field_mismatch",
                        format!("anonymous struct repeats field `{}`", field.name.text),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        field.name.span,
                        "duplicate field",
                    ))
                    .with_action("keep each field only once"),
                );
                self.infer_expr(&field.value);
                continue;
            }

            if let Some((_, field_ty)) = expected_fields
                .iter()
                .find(|(name, _)| name == &field.name.text)
            {
                self.check_expr_assignable_to(&field.value, field_ty);
            } else {
                self.sink.emit(
                    Diagnostic::error(
                        "type.anon_struct_field_mismatch",
                        format!("anonymous struct target has no field `{}`", field.name.text),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        field.name.span,
                        "unknown field",
                    ))
                    .with_action("use a field declared on the expected struct"),
                );
                self.infer_expr(&field.value);
            }
        }

        for (name, _) in expected_fields {
            if !provided.contains_key(&name) {
                self.sink.emit(
                    Diagnostic::error(
                        "type.anon_struct_field_mismatch",
                        format!("anonymous struct is missing field `{}`", name),
                    )
                    .with_label(Label::primary(self.file_id, span, "anonymous struct here"))
                    .with_action(format!(
                        "provide `{}` in the anonymous struct literal",
                        name
                    )),
                );
            }
        }

        expected.clone()
    }

    fn check_struct_literal(
        &mut self,
        ty_name: &ori_ast::common::QualifiedName,
        fields: &[ori_ast::expr::FieldInit],
        span: ori_diagnostics::Span,
    ) -> Ty {
        let path = ty_name.to_string();
        let Some(def_id) = self.resolve_def_id(&path) else {
            self.sink.emit(
                Diagnostic::error("type.undefined_name", format!("undefined type `{path}`"))
                    .with_label(Label::primary(self.file_id, ty_name.span, "type used here"))
                    .with_action("use a struct type that is defined or imported in this namespace"),
            );
            for field in fields {
                self.infer_expr(&field.value);
            }
            return Ty::Error;
        };
        self.check_visibility(def_id, ty_name.span);
        let def = self.def_map.get(def_id);
        if def.kind != DefKind::Struct {
            self.sink.emit(
                Diagnostic::error(
                    "type.type_mismatch",
                    format!("`{path}` is not a struct type"),
                )
                .with_label(Label::primary(self.file_id, ty_name.span, "not a struct"))
                .with_action("use a struct type before `{{ … }}`"),
            );
            for field in fields {
                self.infer_expr(&field.value);
            }
            return Ty::Error;
        }

        let field_sig = self
            .struct_sigs
            .iter()
            .find(|s| s.def_id == def_id)
            .map(|s| s.fields.clone())
            .unwrap_or_default();
        let mut provided = HashSet::new();
        for field in fields {
            if !provided.insert(field.name.text.clone()) {
                self.sink.emit(
                    Diagnostic::error(
                        "type.anon_struct_field_mismatch",
                        format!("struct literal repeats field `{}`", field.name.text),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        field.name.span,
                        "duplicate field",
                    ))
                    .with_action("keep each field only once"),
                );
                self.infer_expr(&field.value);
                continue;
            }
            if let Some((_, expected)) = field_sig.iter().find(|(name, _)| name == &field.name.text)
            {
                self.check_expr_assignable_to(&field.value, expected);
            } else {
                self.infer_expr(&field.value);
                self.sink.emit(
                    Diagnostic::error(
                        "type.no_such_field",
                        format!("struct has no field `{}`", field.name.text),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        field.name.span,
                        "unknown field",
                    ))
                    .with_action("use a field declared on the struct"),
                );
            }
        }
        for (name, _) in &field_sig {
            if !provided.contains(name) {
                self.sink.emit(
                    Diagnostic::error(
                        "type.missing_struct_field",
                        format!("struct construction is missing field `{name}`"),
                    )
                    .with_label(Label::primary(self.file_id, span, "struct literal here"))
                    .with_action(format!("provide `{name}` in the struct literal")),
                );
            }
        }
        Ty::Named(def_id, Vec::new())
    }

    fn check_enum_variant_unit(
        &mut self,
        ty_name: Option<&ori_ast::common::QualifiedName>,
        variant: &ori_ast::common::Name,
        expected: &Ty,
        span: ori_diagnostics::Span,
    ) -> Ty {
        if let Some(qn) = ty_name {
            let mut path = qn.clone();
            path.parts.push(variant.clone());
            path.span = path.span.cover(variant.span);
            if let Some((def_id, _)) = self.resolve_enum_variant(&path) {
                return Ty::Named(def_id, Vec::new());
            }
            self.sink.emit(
                Diagnostic::error(
                    "type.undefined_name",
                    format!("undefined enum variant `{}.{}`", qn, variant.text),
                )
                .with_label(Label::primary(
                    self.file_id,
                    span,
                    "variant used here",
                )),
            );
            return Ty::Error;
        }
        // Shorthand `.Variant` — need expected enum type from context.
        if let Some(sig) = self.enum_variant_sig_for_name(expected, variant) {
            let _ = sig;
            return expected.clone();
        }
        if expected.contains_infer() {
            // Infer path used without expected context.
            return Ty::Infer(0);
        }
        self.sink.emit(
            Diagnostic::error(
                "type.type_mismatch",
                format!(
                    "shorthand enum variant `.{}` needs an expected enum type, found {}",
                    variant.text,
                    expected.display()
                ),
            )
            .with_label(Label::primary(self.file_id, span, "enum variant here"))
            .with_action("annotate the binding or use `Enum.Variant`"),
        );
        Ty::Error
    }

    fn check_enum_variant_named(
        &mut self,
        ty_name: Option<&ori_ast::common::QualifiedName>,
        variant: &ori_ast::common::Name,
        fields: &[ori_ast::expr::FieldInit],
        expected: &Ty,
        span: ori_diagnostics::Span,
    ) -> Ty {
        let resolved = if let Some(qn) = ty_name {
            let mut path = qn.clone();
            path.parts.push(variant.clone());
            path.span = path.span.cover(variant.span);
            self.resolve_enum_variant(&path)
        } else if let Some(_sig) = self.enum_variant_sig_for_name(expected, variant) {
            if let Ty::Named(def_id, _) = expected {
                Some((*def_id, variant.text.clone()))
            } else {
                None
            }
        } else {
            None
        };

        let Some((def_id, _)) = resolved else {
            for field in fields {
                self.infer_expr(&field.value);
            }
            if ty_name.is_none() && expected.contains_infer() {
                return Ty::Infer(0);
            }
            self.sink.emit(
                Diagnostic::error(
                    "type.undefined_name",
                    format!("undefined enum variant `{}`", variant.text),
                )
                .with_label(Label::primary(
                    self.file_id,
                    span,
                    "variant used here",
                )),
            );
            return Ty::Error;
        };

        let variant_fields = self
            .enum_sig(def_id)
            .and_then(|sig| {
                sig.variants
                    .iter()
                    .find(|v| v.name == variant.text)
                    .map(|v| v.fields.clone())
            })
            .unwrap_or_default();

        let mut provided = HashSet::new();
        for field in fields {
            if !provided.insert(field.name.text.clone()) {
                self.sink.emit(
                    Diagnostic::error(
                        "type.anon_struct_field_mismatch",
                        format!(
                            "enum variant `{}` repeats field `{}`",
                            variant.text, field.name.text
                        ),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        field.name.span,
                        "duplicate field",
                    ))
                    .with_action("keep each field only once"),
                );
                self.infer_expr(&field.value);
                continue;
            }
            if let Some((_, field_ty)) = variant_fields
                .iter()
                .find(|(name, _)| name == &field.name.text)
            {
                self.check_expr_assignable_to(&field.value, field_ty);
            } else {
                self.infer_expr(&field.value);
                self.sink.emit(
                    Diagnostic::error(
                        "type.no_such_field",
                        format!(
                            "enum variant `{}` has no field `{}`",
                            variant.text, field.name.text
                        ),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        field.name.span,
                        "unknown field",
                    )),
                );
            }
        }
        for (name, _) in &variant_fields {
            if !provided.contains(name) {
                self.sink.emit(
                    Diagnostic::error(
                        "type.enum_variant_named_fields_required",
                        format!("enum variant `{}` is missing field `{name}`", variant.text),
                    )
                    .with_label(Label::primary(self.file_id, span, "variant construction"))
                    .with_action(format!("provide `{name}`")),
                );
            }
        }
        Ty::Named(def_id, Vec::new())
    }

    fn expected_struct_fields(&self, expected: &Ty) -> Option<Vec<(SmolStr, Ty)>> {
        let Ty::Named(def_id, args) = expected else {
            return None;
        };
        let def = self.def_map.all_defs().get(def_id.0 as usize)?;
        if def.kind != DefKind::Struct {
            return None;
        }
        self.struct_sigs
            .iter()
            .find(|sig| sig.def_id == *def_id)
            .map(|sig| {
                sig.fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), substitute_ty_params(ty, args)))
                    .collect()
            })
    }

    fn emit_anon_struct_type_unknown(&mut self, expected: &Ty, span: ori_diagnostics::Span) {
        let found = if expected.contains_infer() {
            "an unknown type".to_string()
        } else {
            format!("`{}`", expected.display())
        };
        self.sink.emit(
            Diagnostic::error(
                "type.anon_struct_type_unknown",
                format!("anonymous struct literal needs an expected struct type, found {found}"),
            )
            .with_label(Label::primary(self.file_id, span, "anonymous struct here"))
            .with_action("add a struct type annotation or use an explicit struct constructor"),
        );
    }

    pub fn infer_expr(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::BoolLit(..) => Ty::Bool,
            Expr::IntLit { raw, span } => match parse_int_literal(raw) {
                Ok(parsed) => parsed.ty,
                Err(err) => {
                    self.emit_numeric_literal_error(*span, &err.message, err.kind);
                    Ty::Error
                }
            },
            Expr::FloatLit { raw, span } => match parse_float_literal(raw) {
                Ok(parsed) => parsed.ty,
                Err(err) => {
                    self.emit_numeric_literal_error(*span, &err.message, err.kind);
                    Ty::Error
                }
            },
            Expr::StrLit { .. } => Ty::String,
            Expr::FStrLit { parts, .. } => {
                for part in parts {
                    if let FStrPart::Interpolated(expr) = part {
                        let part_ty = self.infer_expr(expr);
                        if !self.supports_string_conversion_ty(&part_ty)
                            && !part_ty.is_error()
                            && !part_ty.contains_infer()
                        {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.arg_type_mismatch",
                                    format!(
                                        "`f-string` interpolation expects `int`, `float`, `bool`, `string`, or a `Displayable` value, found `{}`",
                                        part_ty.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    expr.span(),
                                    "interpolated value here",
                                ))
                                .with_action(
                                    "interpolate a scalar/string value or implement `ori.core.Displayable`",
                                ),
                            );
                        }
                    }
                }
                Ty::String
            }
            Expr::BytesLit { .. } => Ty::Bytes,
            Expr::None(_) => Ty::Optional(Box::new(Ty::Infer(0))),
            Expr::SelfExpr(span) => self.lookup_self(*span),
            Expr::Ident(n) => self.lookup_var(&n.text, n.span),
            Expr::QualifiedIdent(q) => {
                // Single-segment names may be local variables — check scope first
                if q.is_single() {
                    let name = q.last().as_str();
                    if let Some((ty, scope_idx, mutable)) = self.lookup_local_var_binding(name) {
                        self.check_closure_var_capture(name, q.span, scope_idx, mutable, &ty);
                        return ty;
                    }
                }
                if let Some((def_id, _variant)) = self.resolve_enum_variant(q) {
                    return Ty::Named(def_id, Vec::new());
                }
                // Fall back to global def_map
                let path = q.to_string();
                let expanded_path = self.expand_alias(&path);
                if let Some(ty) = stdlib_const_ty(&expanded_path) {
                    return ty;
                }
                if let Some(id) = self.resolve_def_id(&path) {
                    self.check_visibility(id, q.span);
                    let def = self.def_map.get(id);
                    match def.kind {
                        crate::def::DefKind::Const | crate::def::DefKind::Var => {
                            self.value_ty(id).unwrap_or(Ty::Infer(id.0))
                        }
                        crate::def::DefKind::Func | crate::def::DefKind::Extern => self
                            .func_sig(id)
                            .map(|sig| Ty::Func {
                                params: sig.params,
                                ret: Box::new(sig.return_ty),
                            })
                            .unwrap_or(Ty::Infer(id.0)),
                        _ => Ty::Infer(id.0),
                    }
                } else if let Some(first) = q.parts.first() {
                    if let Some((mut ty, scope_idx, mutable)) =
                        self.lookup_local_var_binding(&first.text)
                    {
                        self.check_closure_var_capture(
                            &first.text,
                            first.span,
                            scope_idx,
                            mutable,
                            &ty,
                        );
                        for field in q.parts.iter().skip(1) {
                            ty = self.infer_field_access(Some(first), ty, field);
                            if ty.is_error() {
                                break;
                            }
                        }
                        ty
                    } else {
                        self.emit_undefined_name(&q.to_string(), q.span);
                        Ty::Error
                    }
                } else {
                    self.emit_undefined_name(&q.to_string(), q.span);
                    Ty::Error
                }
            }
            Expr::Range { start, end, span } => {
                let start_ty = self.infer_expr(start);
                let end_ty = self.infer_expr(end);
                if !start_ty.is_error() && !start_ty.contains_infer() && start_ty != Ty::Int {
                    self.emit_invalid_range_endpoint("start", start.span(), &start_ty);
                }
                if !end_ty.is_error() && !end_ty.contains_infer() && end_ty != Ty::Int {
                    self.emit_invalid_range_endpoint("end", end.span(), &end_ty);
                }
                let _ = *span;
                Ty::Range(Box::new(Ty::Int))
            }
            Expr::List { elements, .. } => {
                if elements.is_empty() {
                    Ty::List(Box::new(Ty::Infer(0)))
                } else {
                    let first_ty = self.infer_expr(&elements[0]);
                    for elem in elements.iter().skip(1) {
                        let elem_ty = self.infer_expr(elem);
                        if !elem_ty.is_error()
                            && !first_ty.is_error()
                            && !elem_ty.contains_infer()
                            && !first_ty.contains_infer()
                            && !elem_ty.is_assignable_to(&first_ty)
                        {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.list_element_mismatch",
                                    format!(
                                        "list element type `{}` does not match first element `{}`",
                                        elem_ty.display(),
                                        first_ty.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    elem.span(),
                                    "mismatched element",
                                ))
                                .with_action(format!(
                                    "all list elements should be `{}`",
                                    first_ty.display()
                                )),
                            );
                        }
                    }
                    Ty::List(Box::new(first_ty))
                }
            }
            Expr::Map { entries, span } => {
                if entries.is_empty() {
                    Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0)))
                } else {
                    let (first_key, first_value) = &entries[0];
                    let first_key_ty = self.infer_expr(first_key);
                    let first_value_ty = self.infer_expr(first_value);
                    for (key, value) in entries.iter().skip(1) {
                        let key_ty = self.infer_expr(key);
                        if !key_ty.is_error()
                            && !first_key_ty.is_error()
                            && !key_ty.contains_infer()
                            && !first_key_ty.contains_infer()
                            && !key_ty.is_assignable_to(&first_key_ty)
                        {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.map_key_mismatch",
                                    format!(
                                        "map key type `{}` does not match first key `{}`",
                                        key_ty.display(),
                                        first_key_ty.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    key.span(),
                                    "mismatched key",
                                ))
                                .with_action(format!(
                                    "all map keys should be `{}`",
                                    first_key_ty.display()
                                )),
                            );
                        }
                        let value_ty = self.infer_expr(value);
                        if !value_ty.is_error()
                            && !first_value_ty.is_error()
                            && !value_ty.contains_infer()
                            && !first_value_ty.contains_infer()
                            && !value_ty.is_assignable_to(&first_value_ty)
                        {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.map_value_mismatch",
                                    format!(
                                        "map value type `{}` does not match first value `{}`",
                                        value_ty.display(),
                                        first_value_ty.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    value.span(),
                                    "mismatched value",
                                ))
                                .with_action(format!(
                                    "all map values should be `{}`",
                                    first_value_ty.display()
                                )),
                            );
                        }
                    }
                    let ty = Ty::Map(Box::new(first_key_ty), Box::new(first_value_ty));
                    self.check_collection_runtime_limits(&ty, *span);
                    ty
                }
            }
            Expr::Set { elements, span } => {
                if elements.is_empty() {
                    Ty::Set(Box::new(Ty::Infer(0)))
                } else {
                    let first_ty = self.infer_expr(&elements[0]);
                    for elem in elements.iter().skip(1) {
                        let elem_ty = self.infer_expr(elem);
                        if !elem_ty.is_error()
                            && !first_ty.is_error()
                            && !elem_ty.contains_infer()
                            && !first_ty.contains_infer()
                            && !elem_ty.is_assignable_to(&first_ty)
                        {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.set_element_mismatch",
                                    format!(
                                        "set element type `{}` does not match first element `{}`",
                                        elem_ty.display(),
                                        first_ty.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    elem.span(),
                                    "mismatched element",
                                ))
                                .with_action(format!(
                                    "all set elements should be `{}`",
                                    first_ty.display()
                                )),
                            );
                        }
                    }
                    let ty = Ty::Set(Box::new(first_ty));
                    self.check_collection_runtime_limits(&ty, *span);
                    ty
                }
            }
            Expr::Tuple { elements, .. } => {
                Ty::Tuple(elements.iter().map(|e| self.infer_expr(e)).collect())
            }
            Expr::AnonStructLit { fields, span } => {
                for field in fields {
                    self.infer_expr(&field.value);
                }
                self.emit_anon_struct_type_unknown(&Ty::Infer(0), *span);
                Ty::Error
            }
            Expr::StructLit { ty, fields, span } => self.check_struct_literal(ty, fields, *span),
            Expr::EnumVariantUnit { ty, variant, span } => {
                self.check_enum_variant_unit(ty.as_ref(), variant, &Ty::Infer(0), *span)
            }
            Expr::EnumVariantNamed {
                ty,
                variant,
                fields,
                span,
            } => self.check_enum_variant_named(ty.as_ref(), variant, fields, &Ty::Infer(0), *span),
            Expr::Unary { op, operand, span } => {
                let t = self.infer_expr(operand);
                match op {
                    UnaryOp::Neg => {
                        if !t.is_numeric() && !t.is_error() {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.unary_neg_non_numeric",
                                    format!(
                                        "unary `-` applied to non-numeric type `{}`",
                                        t.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    *span,
                                    "here",
                                )),
                            );
                            return Ty::Error;
                        }
                        t
                    }
                    UnaryOp::Not => {
                        self.expect_bool(&t, operand.span());
                        Ty::Bool
                    }
                }
            }
            Expr::Binary { op, lhs, rhs, span } => {
                let lt = self.infer_expr(lhs);
                let rt = self.infer_expr(rhs);
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    self.expect_bool(&lt, lhs.span());
                    self.expect_bool(&rt, rhs.span());
                    return Ty::Bool;
                }
                self.infer_binary(*op, &lt, &rt, *span)
            }
            Expr::Field { object, field, .. } => {
                let obj_ty = self.infer_expr(object);
                self.infer_field_access(expr_root_name(object), obj_ty, field)
            }
            Expr::Call { callee, args, .. } => {
                // Handle method-like calls on built-in types: optional.or(), result.or(), etc.
                if let Expr::Field {
                    object,
                    field,
                    span,
                } = callee.as_ref()
                {
                    let method = field.text.as_str();
                    if method == "or" && args.len() == 1 {
                        let obj_ty = self.infer_expr(object);
                        let fallback = &args[0];
                        let fallback_expr = match &fallback.value {
                            ArgValue::Expr(e) | ArgValue::Spread(e) => e,
                        };
                        let fallback_ty = self.infer_expr(fallback_expr);
                        return match &obj_ty {
                            Ty::Optional(inner) => {
                                if !inner.is_assignable_to(&fallback_ty) {
                                    self.sink.emit(
                                        Diagnostic::error("type.type_mismatch",
                                            format!("`.or()` fallback type `{}` does not match optional inner type `{}`",
                                                fallback_ty.display(), inner.display()))
                                        .with_label(Label::primary(self.file_id, *span, "fallback type mismatch")),
                                    );
                                }
                                *inner.clone()
                            }
                            Ty::Result(ok, _err) => {
                                if !ok.is_assignable_to(&fallback_ty) {
                                    self.sink.emit(
                                        Diagnostic::error("type.type_mismatch",
                                            format!("`.or()` fallback type `{}` does not match result ok type `{}`",
                                                fallback_ty.display(), ok.display()))
                                        .with_label(Label::primary(self.file_id, *span, "fallback type mismatch")),
                                    );
                                }
                                *ok.clone()
                            }
                            _ => {
                                self.sink.emit(
                                    Diagnostic::error("type.type_mismatch",
                                        format!("`.or()` can only be called on `optional[T]` or `result[T,E]`, got `{}`",
                                            obj_ty.display()))
                                    .with_label(Label::primary(self.file_id, *span, "invalid `.or()` receiver")),
                                );
                                Ty::Error
                            }
                        };
                    }
                    if method == "or_wrap" && args.len() == 1 {
                        let obj_ty = self.infer_expr(object);
                        let context = &args[0];
                        let context_expr = match &context.value {
                            ArgValue::Expr(e) | ArgValue::Spread(e) => e,
                        };
                        let context_ty = self.infer_expr(context_expr);
                        if !context_ty.is_assignable_to(&Ty::String) && !context_ty.is_error() {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.type_mismatch",
                                    format!(
                                        "`.or_wrap()` context must be `string`, got `{}`",
                                        context_ty.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    *span,
                                    "context type mismatch",
                                )),
                            );
                        }
                        return match &obj_ty {
                            Ty::Result(_, err) if err.is_assignable_to(&Ty::String) => obj_ty,
                            Ty::Result(_, err) => {
                                self.sink.emit(
                                    Diagnostic::error(
                                        "type.type_mismatch",
                                        format!(
                                            "`.or_wrap()` currently requires `result[T, string]`, got error type `{}`",
                                            err.display()
                                        ),
                                    )
                                    .with_label(Label::primary(
                                        self.file_id,
                                        *span,
                                        "unsupported result error type",
                                    )),
                                );
                                Ty::Error
                            }
                            _ => {
                                self.sink.emit(
                                    Diagnostic::error(
                                        "type.type_mismatch",
                                        format!(
                                            "`.or_wrap()` can only be called on `result[T, string]`, got `{}`",
                                            obj_ty.display()
                                        ),
                                    )
                                    .with_label(Label::primary(
                                        self.file_id,
                                        *span,
                                        "invalid `.or_wrap()` receiver",
                                    )),
                                );
                                Ty::Error
                            }
                        };
                    }
                    if method == "or_return" && args.is_empty() {
                        let obj_ty = self.infer_expr(object);
                        let cur_ret = self.current_return_ty.clone().unwrap_or(Ty::Infer(0));
                        return match &obj_ty {
                            Ty::Optional(inner) => {
                                // .or_return() has the same propagation semantics as `try obj`
                                // In a function returning T (not optional[T]), this is an error
                                if let Ty::Optional(ret_inner) = &cur_ret {
                                    if !inner.is_assignable_to(ret_inner) {
                                        self.sink.emit(
                                            Diagnostic::error("type.type_mismatch",
                                                format!("`.or_return()` inner type `{}` does not match function's optional return inner type `{}`",
                                                    inner.display(), ret_inner.display()))
                                            .with_label(Label::primary(self.file_id, *span, "type mismatch")),
                                        );
                                    }
                                    *inner.clone()
                                } else {
                                    *inner.clone() // will be caught by try-propagation logic
                                }
                            }
                            Ty::Result(ok, err) => {
                                // .or_return() has the same propagation semantics as `try obj`
                                if let Ty::Result(ret_ok, ret_err) = &cur_ret {
                                    if !ok.is_assignable_to(ret_ok) {
                                        self.sink.emit(
                                            Diagnostic::error("type.type_mismatch",
                                                format!("`.or_return()` ok type `{}` does not match function's result ok type `{}`",
                                                    ok.display(), ret_ok.display()))
                                            .with_label(Label::primary(self.file_id, *span, "type mismatch")),
                                        );
                                    }
                                    if !err.is_assignable_to(ret_err) && !err.is_error() {
                                        self.sink.emit(
                                            Diagnostic::error("type.type_mismatch",
                                                format!("`.or_return()` error type `{}` does not match function's result error type `{}`",
                                                    err.display(), ret_err.display()))
                                            .with_label(Label::primary(self.file_id, *span, "error type mismatch")),
                                        );
                                    }
                                }
                                *ok.clone()
                            }
                            _ => {
                                self.sink.emit(
                                    Diagnostic::error("type.type_mismatch",
                                        format!("`.or_return()` can only be called on `optional[T]` or `result[T,E]`, got `{}`",
                                            obj_ty.display()))
                                    .with_label(Label::primary(self.file_id, *span, "invalid `.or_return()` receiver")),
                                );
                                Ty::Error
                            }
                        };
                    }
                }

                // If callee is a named function, look up its return type
                if let Expr::QualifiedIdent(q) = callee.as_ref() {
                    if q.is_single() {
                        if let Some(ret) =
                            self.infer_never_form_call(q.last().as_str(), args, expr.span())
                        {
                            return ret;
                        }
                        if let Some(ret) =
                            self.infer_wrapper_form_call(q.last().as_str(), args, expr.span())
                        {
                            return ret;
                        }
                    }
                    if let Some(ret) = self.infer_qualified_trait_method_call(q, args, expr.span())
                    {
                        return ret;
                    }
                    let path = q.to_string();
                    if let Some(def_id) = self.resolve_def_id(&path) {
                        self.check_visibility(def_id, q.span);
                        let def = self.def_map.get(def_id);
                        if def.kind == DefKind::Func || def.kind == DefKind::Extern {
                            if let Some(sig) = self.func_sig(def_id) {
                                let mut params = sig.params.clone();
                                let mut ret = self
                                    .func_return_ty(def_id)
                                    .unwrap_or_else(|| sig.return_ty.clone());
                                let mut subst = HashMap::new();
                                if params.iter().any(contains_generic_param)
                                    || contains_generic_param(&ret)
                                {
                                    subst = self.infer_generic_call_substitutions(args, &sig);
                                    if self.is_generic_self_call_without_concrete_instantiation(
                                        def_id, &subst,
                                    ) {
                                        self.emit_generic_circular_instantiation(q, expr.span());
                                        return Ty::Error;
                                    }
                                    if !subst.is_empty() {
                                        params = params
                                            .iter()
                                            .map(|ty| substitute_generic_params(ty, &subst))
                                            .collect();
                                        ret = substitute_generic_params(&ret, &subst);
                                    }
                                }
                                self.check_where_constraints(
                                    &sig.where_constraints,
                                    &subst,
                                    expr.span(),
                                );
                                self.check_call_args_with_defaults(
                                    args,
                                    &params,
                                    &sig.param_names,
                                    &sig.param_defaults,
                                    &sig.param_variadic,
                                    expr.span(),
                                );
                                return ret;
                            }
                        } else if def.kind == DefKind::Struct {
                            // S3: `Type(...)` struct construction is removed.
                            // Still check fields for cascade diagnostics, but poison the type
                            // so recovery does not look like a successful construction.
                            self.sink.emit(
                                Diagnostic::error(
                                    "parse.removed_struct_call_literal",
                                    format!(
                                        "struct construction via `{}(...)` is removed; write `{} {{ field: value }}` or `{{ field: value }}`",
                                        path, path
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    expr.span(),
                                    "use braced struct literal",
                                ))
                                .with_action(format!(
                                    "write `{} {{ … }}` or context-typed `{{ … }}`",
                                    path
                                )),
                            );
                            self.check_struct_constructor_args(def_id, args);
                            return Ty::Error;
                        }
                    }
                    if let Some((def_id, _variant)) = self.resolve_enum_variant(q) {
                        self.check_enum_variant_args(args);
                        return Ty::Named(def_id, Vec::new());
                    }
                    let expanded_path = self.expand_alias(&path);
                    if let Some(ret) = self.infer_stdlib_call(&expanded_path, args, expr.span()) {
                        return ret;
                    }
                }
                let callee_ty = self.infer_expr(callee);
                if callee_ty.is_error() {
                    for a in args {
                        let expr = match &a.value {
                            ArgValue::Expr(e) | ArgValue::Spread(e) => e.as_ref(),
                        };
                        self.infer_expr(expr);
                    }
                    return Ty::Error;
                }
                if let Ty::Func { params, ret } = callee_ty {
                    self.check_call_args(args, &params, expr.span());
                    return *ret;
                }
                for a in args {
                    let expr = match &a.value {
                        ArgValue::Expr(e) | ArgValue::Spread(e) => e.as_ref(),
                    };
                    self.infer_expr(expr);
                }
                self.sink.emit(
                    Diagnostic::error("type.type_mismatch", "called value is not a function")
                        .with_label(Label::primary(
                            self.file_id,
                            expr.span(),
                            "called value does not have a callable type",
                        )),
                );
                Ty::Error
            }
            Expr::Try { expr, span, .. } => {
                let inner = self.infer_expr(expr);
                if inner.is_error() {
                    return Ty::Error;
                }
                match &inner {
                    Ty::Result(ok, err) => {
                        let cur_ret = self.current_return_ty.clone().unwrap_or(Ty::Infer(0));
                        if cur_ret.contains_infer() {
                            return *ok.clone();
                        }
                        if let Ty::Result(_, cur_err) = &cur_ret {
                            if !err.is_assignable_to(cur_err) && !err.is_error() {
                                self.sink.emit(Diagnostic::error(
                                    "type.propagate_err_mismatch",
                                    format!("cannot propagate error type `{}` in a function that returns error type `{}`", err.display(), cur_err.display())
                                ).with_label(Label::primary(self.file_id, *span, "propagated here")));
                            }
                        } else {
                            self.sink.emit(Diagnostic::error(
                                "type.propagate_return_mismatch",
                                format!("cannot use `try` propagation on a `result` in a function that returns `{}`", cur_ret.display())
                            ).with_label(Label::primary(self.file_id, *span, "propagated here")));
                        }
                        *ok.clone()
                    }
                    Ty::Optional(ok) => {
                        let cur_ret = self.current_return_ty.clone().unwrap_or(Ty::Infer(0));
                        if cur_ret.contains_infer() {
                            return *ok.clone();
                        }
                        if !matches!(&cur_ret, Ty::Optional(_)) {
                            self.sink.emit(Diagnostic::error(
                                "type.propagate_return_mismatch",
                                format!("cannot use `try` propagation on an `optional` in a function that returns `{}`", cur_ret.display())
                            ).with_label(Label::primary(self.file_id, *span, "propagated here")));
                        }
                        *ok.clone()
                    }
                    _ => {
                        self.sink.emit(Diagnostic::error(
                            "type.propagate_not_result_or_optional",
                            format!("`try` propagation can only be applied to `result` or `optional`, found `{}`", inner.display())
                        ).with_label(Label::primary(self.file_id, *span, "cannot propagate here")));
                        Ty::Error
                    }
                }
            }
            Expr::Await { expr, span } => {
                let inner = self.infer_expr(expr);
                if self.current_async_depth == 0 {
                    self.sink.emit(
                        Diagnostic::error(
                            "async.await_outside_async",
                            "`await` can only be used inside `async func`",
                        )
                        .with_label(Label::primary(self.file_id, *span, "`await` used here"))
                        .with_action("move this code into an `async func` or use `task.block_on`"),
                    );
                }
                match inner {
                    Ty::Future(value_ty) => *value_ty,
                    Ty::Error => Ty::Error,
                    other if other.contains_infer() => Ty::Infer(0),
                    other => {
                        self.sink.emit(
                            Diagnostic::error(
                                "async.await_non_future",
                                format!("`await` expects `future[T]`, found `{}`", other.display()),
                            )
                            .with_label(Label::primary(self.file_id, expr.span(), "not a future"))
                            .with_action("await only expressions that return `future[T]`"),
                        );
                        Ty::Error
                    }
                }
            }
            Expr::IfExpr {
                condition,
                then_expr,
                else_expr,
                span,
            } => {
                let cond_ty = self.infer_expr(condition);
                self.expect_bool(&cond_ty, condition.span());
                let then_ty = self.infer_expr(then_expr);
                let else_ty = self.infer_expr(else_expr);
                if then_ty.is_never() {
                    return else_ty;
                }
                if else_ty.is_never() {
                    return then_ty;
                }
                if then_ty != else_ty && !then_ty.is_error() && !else_ty.is_error() {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.if_branch_mismatch",
                            format!(
                                "`if` branches have different types: `{}` vs `{}`",
                                then_ty.display(),
                                else_ty.display()
                            ),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            *span,
                            "branches diverge",
                        )),
                    );
                    return Ty::Error;
                }
                then_ty
            }
            Expr::MatchExpr {
                scrutinee,
                arms,
                span,
            } => {
                let scr_ty = self.infer_expr(scrutinee);
                let cases = match_expr_arms_as_cases(arms);
                self.check_match_duplicate_cases(&scr_ty, &cases);
                self.check_match_unreachable_cases(&scr_ty, &cases);

                let mut result: Option<Ty> = None;
                for arm in arms {
                    self.push_scope();
                    if let Some(pattern) = &arm.pattern {
                        self.check_pattern_type(pattern, &scr_ty);
                    }
                    if let Some(guard) = &arm.guard {
                        let guard_ty = self.infer_expr(guard);
                        self.expect_bool(&guard_ty, guard.span());
                    }
                    let arm_ty = self.infer_expr(&arm.body);
                    self.pop_scope();

                    // `never` arms (a `return` inside the arm) do not constrain
                    // the result type — the same rule `if` expressions use.
                    if arm_ty.is_never() || arm_ty.is_error() {
                        continue;
                    }
                    match &result {
                        None => result = Some(arm_ty),
                        Some(previous) if previous != &arm_ty => {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.match_arm_mismatch",
                                    format!(
                                        "`match` arms have different types: `{}` vs `{}`",
                                        previous.display(),
                                        arm_ty.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    arm.body.span(),
                                    "this arm diverges from the earlier arms",
                                ))
                                .with_action(
                                    "make every arm produce the same type, \
                                     or use a `match` statement instead",
                                ),
                            );
                            return Ty::Error;
                        }
                        Some(_) => {}
                    }
                }

                self.check_match_exhaustiveness(&scr_ty, &cases, *span);
                result.unwrap_or(Ty::Never)
            }
            Expr::Index {
                object,
                index,
                span,
            } => {
                let obj_ty = self.infer_expr(object);
                match index {
                    ori_ast::expr::IndexExpr::Single(idx_expr) => {
                        let idx_ty = self.infer_expr(idx_expr);
                        match &obj_ty {
                            Ty::List(elem) => {
                                if !idx_ty.is_assignable_to(&Ty::Int) && !idx_ty.is_error() {
                                    self.sink.emit(
                                        Diagnostic::error(
                                            "type.index_not_int",
                                            format!(
                                                "list index must be `int`, found `{}`",
                                                idx_ty.display()
                                            ),
                                        )
                                        .with_label(Label::primary(
                                            self.file_id,
                                            *span,
                                            "index here",
                                        ))
                                        .with_action("use an integer index"),
                                    );
                                }
                                *elem.clone()
                            }
                            Ty::Map(key, val) => {
                                if !idx_ty.is_assignable_to(key) && !idx_ty.is_error() {
                                    self.sink.emit(
                                        Diagnostic::error(
                                            "type.map_key_mismatch",
                                            format!(
                                                "map key type is `{}`, found `{}`",
                                                key.display(),
                                                idx_ty.display()
                                            ),
                                        )
                                        .with_label(Label::primary(self.file_id, *span, "key here"))
                                        .with_action(format!("use a `{}` key", key.display())),
                                    );
                                }
                                *val.clone()
                            }
                            Ty::String => {
                                if !idx_ty.is_assignable_to(&Ty::Int) && !idx_ty.is_error() {
                                    self.sink.emit(
                                        Diagnostic::error(
                                            "type.index_not_int",
                                            format!(
                                                "string index must be `int`, found `{}`",
                                                idx_ty.display()
                                            ),
                                        )
                                        .with_label(Label::primary(
                                            self.file_id,
                                            *span,
                                            "index here",
                                        ))
                                        .with_action("use an integer index"),
                                    );
                                }
                                Ty::String
                            }
                            Ty::Tuple(elems) => {
                                // Tuple index: try to resolve a constant int index
                                if let Expr::IntLit { raw, .. } = idx_expr.as_ref() {
                                    let i = raw.parse::<usize>().unwrap_or(usize::MAX);
                                    if i < elems.len() {
                                        elems[i].clone()
                                    } else {
                                        self.sink.emit(
                                            Diagnostic::error("type.tuple_index_out_of_bounds",
                                                format!("tuple has {} elements, index {} is out of bounds", elems.len(), i))
                                                .with_label(Label::primary(self.file_id, *span, "out of bounds")));
                                        Ty::Error
                                    }
                                } else {
                                    Ty::Infer(0)
                                }
                            }
                            _ if obj_ty.is_error() || obj_ty.contains_infer() => Ty::Infer(0),
                            _ => {
                                self.sink.emit(
                                    Diagnostic::error(
                                        "type.not_indexable",
                                        format!(
                                            "type `{}` does not support indexing",
                                            obj_ty.display()
                                        ),
                                    )
                                    .with_label(Label::primary(self.file_id, *span, "indexed here"))
                                    .with_action(
                                        "only list, map, string, and tuple values can be indexed",
                                    ),
                                );
                                Ty::Error
                            }
                        }
                    }
                    ori_ast::expr::IndexExpr::Range { start, end } => {
                        if let Some(s) = start {
                            self.infer_expr(s);
                        }
                        if let Some(e) = end {
                            self.infer_expr(e);
                        }
                        // Slicing returns same collection type
                        match &obj_ty {
                            Ty::List(_) | Ty::String => obj_ty,
                            _ if obj_ty.is_error() || obj_ty.contains_infer() => Ty::Infer(0),
                            _ => {
                                self.sink.emit(
                                    Diagnostic::error(
                                        "type.not_sliceable",
                                        format!(
                                            "type `{}` does not support slicing",
                                            obj_ty.display()
                                        ),
                                    )
                                    .with_label(Label::primary(self.file_id, *span, "sliced here")),
                                );
                                Ty::Error
                            }
                        }
                    }
                }
            }
            Expr::TupleIndex {
                object,
                index,
                span,
            } => {
                let obj_ty = self.infer_expr(object);
                if let Ty::Tuple(elems) = &obj_ty {
                    let i = *index as usize;
                    if i < elems.len() {
                        elems[i].clone()
                    } else {
                        self.sink.emit(
                            Diagnostic::error(
                                "type.tuple_index_out_of_bounds",
                                format!(
                                    "tuple has {} elements, index {} is out of bounds",
                                    elems.len(),
                                    i
                                ),
                            )
                            .with_label(Label::primary(
                                self.file_id,
                                *span,
                                "out of bounds",
                            )),
                        );
                        Ty::Error
                    }
                } else if obj_ty.is_error() || obj_ty.contains_infer() {
                    Ty::Infer(0)
                } else {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.tuple_index_on_non_tuple",
                            format!("cannot use tuple index on `{}`", obj_ty.display()),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            *span,
                            "not a tuple",
                        )),
                    );
                    Ty::Error
                }
            }
            Expr::IsCheck { value, ty, span } => {
                self.infer_expr(value);
                self.check_is_target(ty, *span);
                Ty::Bool
            }
            Expr::Closure(closure) => {
                let param_tys: Vec<Ty> = closure
                    .params
                    .iter()
                    .map(|param| {
                        param
                            .ty
                            .as_ref()
                            .map(|ty| self.lower(ty, &[]))
                            .unwrap_or(Ty::Infer(0))
                    })
                    .collect();
                let declared_ret = closure.return_ty.as_ref().map(|ty| self.lower(ty, &[]));

                self.push_scope();
                let closure_scope_root = self.scopes.len() - 1;
                self.closure_scope_roots.push(closure_scope_root);
                for (param, ty) in closure.params.iter().zip(param_tys.iter()) {
                    self.bind_checked(&param.name, ty.clone(), false, false);
                }

                let prev_loop_depth = std::mem::replace(&mut self.loop_depth, 0);
                let ret_ty = match &closure.body {
                    ClosureBody::Expr(expr) => {
                        let actual = self.infer_expr(expr);
                        if let Some(expected) = &declared_ret {
                            self.expect_assignable(&actual, expected, expr.span());
                            expected.clone()
                        } else {
                            actual
                        }
                    }
                    ClosureBody::Block(block) => {
                        let expected = declared_ret.clone().unwrap_or(Ty::Void);
                        let prev_ret = self.current_return_ty.take();
                        self.current_return_ty = Some(expected.clone());
                        self.check_block(block, &expected, &[]);
                        if expected != Ty::Void && !block_definitely_returns(block) {
                            self.sink.emit(
                                Diagnostic::error(
                                    "type.missing_return",
                                    format!(
                                        "closure may finish without returning `{}`",
                                        expected.display()
                                    ),
                                )
                                .with_label(Label::primary(
                                    self.file_id,
                                    block.span,
                                    "not all paths return",
                                ))
                                .with_action("return a value on every path"),
                            );
                        }
                        self.current_return_ty = prev_ret;
                        expected
                    }
                };
                self.loop_depth = prev_loop_depth;
                self.closure_scope_roots.pop();
                self.pop_scope();

                Ty::Func {
                    params: param_tys,
                    ret: Box::new(ret_ty),
                }
            }
            // `value |> func` desugars to `func(value)` for typing (matches HIR lower).
            Expr::Pipe { value, func, span } => {
                let call = Expr::Call {
                    callee: func.clone(),
                    args: vec![Arg {
                        label: None,
                        value: ArgValue::Expr(value.clone()),
                        span: value.span(),
                    }],
                    span: *span,
                };
                self.infer_expr(&call)
            }
            _ => Ty::Infer(0),
        }
    }

    fn infer_binary(&mut self, op: BinaryOp, lt: &Ty, rt: &Ty, span: ori_diagnostics::Span) -> Ty {
        use BinaryOp::*;
        match op {
            Add | Sub | Mul | Div | Rem => {
                if lt.is_numeric() && lt == rt {
                    lt.clone()
                } else if op == Add && lt == &Ty::String && rt == &Ty::String {
                    Ty::String
                } else if matches!(op, Add | Sub | Mul | Div)
                    && self
                        .operator_trait_method_sig(
                            lt,
                            operator_trait_name(op),
                            operator_method_name(op),
                        )
                        .is_some()
                    && self.same_comparison_type(lt, rt)
                {
                    lt.clone()
                } else if lt.is_error() || rt.is_error() {
                    Ty::Error
                } else {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.arithmetic_type_mismatch",
                            format!(
                        "arithmetic operator requires matching numeric types, got `{}` and `{}`",
                        lt.display(), rt.display()
                    ),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            span,
                            "here",
                        )),
                    );
                    Ty::Error
                }
            }
            Eq | Ne => self.infer_equality(op, lt, rt, span),
            Lt | Le | Gt | Ge => self.infer_ordering(op, lt, rt, span),
            And | Or => Ty::Bool,
        }
    }

    fn infer_equality(
        &mut self,
        op: BinaryOp,
        lt: &Ty,
        rt: &Ty,
        span: ori_diagnostics::Span,
    ) -> Ty {
        if lt.is_error() || rt.is_error() {
            return Ty::Bool;
        }
        if !self.same_comparison_type(lt, rt) {
            self.emit_comparison_type_mismatch(lt, rt, span);
            return Ty::Bool;
        }
        if !self.supports_generic_equality(lt)
            && self
                .operator_trait_method_sig(lt, "Equatable", "equals")
                .is_none()
        {
            if let Some((field, field_ty)) = self.unsupported_struct_equality_field(lt) {
                self.emit_equality_unsupported_field(&field, &field_ty, span);
                return Ty::Error;
            }
            self.emit_comparison_not_supported(op, lt, span);
            return Ty::Error;
        }
        Ty::Bool
    }

    fn infer_ordering(
        &mut self,
        op: BinaryOp,
        lt: &Ty,
        rt: &Ty,
        span: ori_diagnostics::Span,
    ) -> Ty {
        if lt.is_error() || rt.is_error() {
            return Ty::Bool;
        }
        if !self.same_comparison_type(lt, rt) {
            self.emit_comparison_type_mismatch(lt, rt, span);
            return Ty::Bool;
        }
        if !self.supports_builtin_ordering(lt)
            && self
                .operator_trait_method_sig(lt, "Comparable", "compare")
                .is_none()
        {
            self.emit_comparison_not_supported(op, lt, span);
            return Ty::Error;
        }
        Ty::Bool
    }

    fn same_comparison_type(&self, lt: &Ty, rt: &Ty) -> bool {
        lt.is_assignable_to(rt) && rt.is_assignable_to(lt)
    }

    fn supports_builtin_equality(&self, ty: &Ty) -> bool {
        self.supports_builtin_equality_inner(ty, &mut Vec::new())
    }

    fn supports_builtin_equality_inner(&self, ty: &Ty, visiting_named: &mut Vec<DefId>) -> bool {
        if ty.is_numeric()
            || matches!(
                ty,
                Ty::Bool | Ty::String | Ty::Infer(_) | Ty::Never | Ty::Any(_)
            )
        {
            return true;
        }
        // Structural equality for compound types whose elements support equality.
        match ty {
            Ty::Optional(inner) => self.supports_generic_equality_inner(inner, visiting_named),
            Ty::Result(ok, err) => {
                self.supports_generic_equality_inner(ok, visiting_named)
                    && self.supports_generic_equality_inner(err, visiting_named)
            }
            Ty::Tuple(elements) => elements
                .iter()
                .all(|e| self.supports_generic_equality_inner(e, visiting_named)),
            Ty::List(inner) => self.supports_generic_equality_inner(inner, visiting_named),
            Ty::Set(inner) => {
                self.supports_runtime_collection_key_equality(inner)
                    && self.supports_generic_equality_inner(inner, visiting_named)
            }
            Ty::Map(key, value) => {
                self.supports_runtime_collection_key_equality(key)
                    && self.supports_generic_equality_inner(value, visiting_named)
            }
            Ty::Bytes => true,
            Ty::Named(def_id, args) => {
                self.supports_structural_struct_equality(*def_id, args, visiting_named)
            }
            Ty::Opaque { kind, args } if kind.is_list_backed_collection() => {
                if let Some(elem_ty) = args.first() {
                    self.supports_generic_equality_inner(elem_ty, visiting_named)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn supports_generic_equality(&self, ty: &Ty) -> bool {
        if self.supports_builtin_equality(ty) || self.user_type_has_equatable(ty) {
            return true;
        }
        if let Ty::Param { index, .. } = ty {
            return self.param_implements_core_trait(*index, "Equatable");
        }
        false
    }

    fn supports_generic_equality_inner(&self, ty: &Ty, visiting_named: &mut Vec<DefId>) -> bool {
        if self.supports_builtin_equality_inner(ty, visiting_named)
            || self.user_type_has_equatable(ty)
        {
            return true;
        }
        if let Ty::Param { index, .. } = ty {
            return self.param_implements_core_trait(*index, "Equatable");
        }
        false
    }

    fn supports_structural_struct_equality(
        &self,
        def_id: DefId,
        args: &[Ty],
        visiting_named: &mut Vec<DefId>,
    ) -> bool {
        if visiting_named.contains(&def_id) {
            return false;
        }
        let Some(def) = self.def_map.all_defs().get(def_id.0 as usize) else {
            return false;
        };
        if def.kind != DefKind::Struct {
            return false;
        }
        let Some(sig) = self.struct_sigs.iter().find(|sig| sig.def_id == def_id) else {
            return false;
        };
        visiting_named.push(def_id);
        let supported = sig.fields.iter().all(|(_, ty)| {
            let substituted = substitute_ty_params(ty, args);
            self.supports_generic_equality_inner(&substituted, visiting_named)
        });
        visiting_named.pop();
        supported
    }

    fn unsupported_struct_equality_field(&self, ty: &Ty) -> Option<(SmolStr, Ty)> {
        self.unsupported_struct_equality_field_inner(ty, &mut Vec::new())
    }

    fn unsupported_struct_equality_field_inner(
        &self,
        ty: &Ty,
        visiting_named: &mut Vec<DefId>,
    ) -> Option<(SmolStr, Ty)> {
        let Ty::Named(def_id, args) = ty else {
            return None;
        };
        if visiting_named.contains(def_id) {
            return None;
        }
        let Some(def) = self.def_map.all_defs().get(def_id.0 as usize) else {
            return None;
        };
        if def.kind != DefKind::Struct {
            return None;
        }
        let Some(sig) = self.struct_sigs.iter().find(|sig| sig.def_id == *def_id) else {
            return None;
        };
        visiting_named.push(*def_id);
        let unsupported = sig
            .fields
            .iter()
            .find(|(_, field_ty)| {
                let substituted = substitute_ty_params(field_ty, args);
                !self.supports_generic_equality_inner(&substituted, visiting_named)
            })
            .map(|(name, field_ty)| (name.clone(), substitute_ty_params(field_ty, args)));
        visiting_named.pop();
        unsupported
    }

    fn emit_equality_unsupported_field(
        &mut self,
        field: &str,
        field_ty: &Ty,
        span: ori_diagnostics::Span,
    ) {
        self.sink.emit(
            Diagnostic::error(
                "type.equality_unsupported_field",
                format!(
                    "field `{}` has type `{}` which does not support equality",
                    field,
                    field_ty.display()
                ),
            )
            .with_label(Label::primary(
                self.file_id,
                span,
                "struct equality comparison here",
            ))
            .with_why("struct equality compares every field; one field cannot be compared safely")
            .with_action(
                "implement `ori.core.Equatable` for the type or compare supported fields manually",
            ),
        );
    }

    fn supports_runtime_collection_key_equality(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::String | Ty::Infer(_) | Ty::Never)
            || is_current_integer_hash_supported(ty)
            || self.supports_generic_equality(ty)
    }

    fn supports_builtin_ordering(&self, ty: &Ty) -> bool {
        ty.is_numeric() || matches!(ty, Ty::Infer(_) | Ty::Never)
    }

    fn supports_iter_sort(&self, ty: &Ty) -> bool {
        ty.is_numeric()
            || matches!(ty, Ty::String | Ty::Infer(_) | Ty::Never)
            || self.user_type_implements_core_trait(ty, "Comparable")
    }

    fn emit_comparison_type_mismatch(&mut self, lt: &Ty, rt: &Ty, span: ori_diagnostics::Span) {
        self.sink.emit(
            Diagnostic::error(
                "type.comparison_type_mismatch",
                format!(
                    "comparison between `{}` and `{}`",
                    lt.display(),
                    rt.display()
                ),
            )
            .with_label(Label::primary(self.file_id, span, "here")),
        );
    }

    fn emit_comparison_not_supported(
        &mut self,
        op: BinaryOp,
        ty: &Ty,
        span: ori_diagnostics::Span,
    ) {
        let op_text = comparison_op_text(op);
        let reason = unsupported_comparison_reason(ty);
        self.sink.emit(
            Diagnostic::error(
                "type.comparison_not_supported",
                format!(
                    "operator `{}` is not supported for `{}`: {}",
                    op_text,
                    ty.display(),
                    reason
                ),
            )
            .with_label(Label::primary(self.file_id, span, "comparison here"))
            .with_action(comparison_action(ty)),
        );
    }

    fn check_call_args(&mut self, args: &[Arg], params: &[Ty], span: ori_diagnostics::Span) {
        let param_variadic = vec![false; params.len()];
        self.check_call_args_with_defaults(args, params, &[], &[], &param_variadic, span);
    }

    fn check_call_args_with_defaults(
        &mut self,
        args: &[Arg],
        params: &[Ty],
        param_names: &[SmolStr],
        param_defaults: &[bool],
        param_variadic: &[bool],
        span: ori_diagnostics::Span,
    ) {
        let has_spread = args
            .iter()
            .any(|arg| matches!(arg.value, ArgValue::Spread(_)));
        let variadic_index = param_variadic.iter().position(|variadic| *variadic);
        let fixed_count = variadic_index.unwrap_or(params.len());
        let min_args = min_required_arg_count(fixed_count, param_defaults);
        let spread_into_variadic = has_spread && variadic_index.is_some();
        if !spread_into_variadic
            && (args.len() < min_args || (variadic_index.is_none() && args.len() > params.len()))
        {
            let expected = if variadic_index.is_some() {
                format!("at least {min_args}")
            } else if min_args == params.len() {
                params.len().to_string()
            } else {
                format!("{min_args} to {}", params.len())
            };
            self.sink.emit(
                Diagnostic::error(
                    "type.arg_count_mismatch",
                    format!(
                        "function expects {} argument(s), got {}",
                        expected,
                        args.len()
                    ),
                )
                .with_label(Label::primary(self.file_id, span, "called here"))
                .with_action("pass the expected number of arguments"),
            );
        }
        if param_names.is_empty() || args.iter().all(|arg| arg.label.is_none()) {
            for (index, (arg, expected)) in args
                .iter()
                .take(fixed_count)
                .zip(params.iter().take(fixed_count))
                .enumerate()
            {
                self.check_single_call_arg(arg, expected, index);
            }
            if let Some(index) = variadic_index {
                if let Some(expected) = params.get(index) {
                    for arg in args.iter().skip(index) {
                        self.check_variadic_call_arg(arg, expected, index);
                    }
                }
            } else {
                for arg in args.iter().skip(params.len()) {
                    self.infer_call_arg(arg);
                }
            }
        } else {
            let (slots, extras) = self.order_call_arg_slots(args, param_names);
            for (index, (arg, expected)) in slots
                .iter()
                .take(fixed_count)
                .zip(params.iter().take(fixed_count))
                .enumerate()
            {
                if let Some(arg) = arg {
                    self.check_single_call_arg(arg, expected, index);
                }
            }
            if let Some(index) = variadic_index {
                if let Some(expected) = params.get(index) {
                    if let Some(Some(arg)) = slots.get(index) {
                        self.check_variadic_call_arg(arg, expected, index);
                    }
                    for arg in extras {
                        self.check_variadic_call_arg(arg, expected, index);
                    }
                }
            } else {
                for arg in extras {
                    self.infer_call_arg(arg);
                }
            }
        }
    }

    fn order_call_arg_slots<'b>(
        &mut self,
        args: &'b [Arg],
        param_names: &[SmolStr],
    ) -> (Vec<Option<&'b Arg>>, Vec<&'b Arg>) {
        let mut slots: Vec<Option<&Arg>> = vec![None; param_names.len()];
        let mut extras = Vec::new();
        let mut next_positional = 0usize;
        let mut seen_named = false;

        for arg in args {
            if let Some(label) = &arg.label {
                seen_named = true;
                let Some(index) = param_names.iter().position(|name| name == &label.text) else {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.unknown_arg_label",
                            format!("function has no parameter named `{}`", label.text),
                        )
                        .with_label(Label::primary(self.file_id, label.span, "unknown label"))
                        .with_action("use one of the function parameter names"),
                    );
                    extras.push(arg);
                    continue;
                };
                if slots[index].is_some() {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.duplicate_arg_label",
                            format!("argument `{}` was passed more than once", label.text),
                        )
                        .with_label(Label::primary(self.file_id, label.span, "duplicate label"))
                        .with_action("remove one of the duplicated arguments"),
                    );
                    extras.push(arg);
                } else {
                    slots[index] = Some(arg);
                }
            } else {
                if seen_named {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.positional_after_named_arg",
                            "positional argument cannot appear after a named argument",
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            arg.span,
                            "positional argument here",
                        ))
                        .with_action("use a named argument here too"),
                    );
                }
                while next_positional < slots.len() && slots[next_positional].is_some() {
                    next_positional += 1;
                }
                if next_positional < slots.len() {
                    slots[next_positional] = Some(arg);
                    next_positional += 1;
                } else {
                    extras.push(arg);
                }
            }
        }

        (slots, extras)
    }

    fn infer_generic_call_substitutions(
        &mut self,
        args: &[Arg],
        sig: &FuncSig,
    ) -> HashMap<u32, Ty> {
        let mut subst = HashMap::new();
        let variadic_index = sig.param_variadic.iter().position(|variadic| *variadic);
        let fixed_count = variadic_index.unwrap_or(sig.params.len());

        if sig.param_names.is_empty() || args.iter().all(|arg| arg.label.is_none()) {
            for (arg, expected) in args
                .iter()
                .take(fixed_count)
                .zip(sig.params.iter().take(fixed_count))
            {
                self.infer_generic_single_arg_substitution(arg, expected, &mut subst);
            }
            if let Some(index) = variadic_index {
                if let Some(expected) = sig.params.get(index) {
                    for arg in args.iter().skip(index) {
                        self.infer_generic_variadic_arg_substitution(arg, expected, &mut subst);
                    }
                }
            }
        } else {
            let mut slots: Vec<Option<&Arg>> = vec![None; sig.param_names.len()];
            let mut extras = Vec::new();
            let mut next_positional = 0usize;

            for arg in args {
                if let Some(label) = &arg.label {
                    if let Some(index) = sig.param_names.iter().position(|name| name == &label.text)
                    {
                        if slots[index].is_none() {
                            slots[index] = Some(arg);
                        } else {
                            extras.push(arg);
                        }
                    } else {
                        extras.push(arg);
                    }
                } else {
                    while next_positional < slots.len() && slots[next_positional].is_some() {
                        next_positional += 1;
                    }
                    if next_positional < slots.len() {
                        slots[next_positional] = Some(arg);
                        next_positional += 1;
                    } else {
                        extras.push(arg);
                    }
                }
            }

            for (arg, expected) in slots
                .iter()
                .take(fixed_count)
                .zip(sig.params.iter().take(fixed_count))
            {
                if let Some(arg) = arg {
                    self.infer_generic_single_arg_substitution(arg, expected, &mut subst);
                }
            }
            if let Some(index) = variadic_index {
                if let Some(expected) = sig.params.get(index) {
                    if let Some(Some(arg)) = slots.get(index) {
                        self.infer_generic_variadic_arg_substitution(arg, expected, &mut subst);
                    }
                    for arg in extras {
                        self.infer_generic_variadic_arg_substitution(arg, expected, &mut subst);
                    }
                }
            }
        }

        subst
    }

    fn is_generic_self_call_without_concrete_instantiation(
        &self,
        def_id: DefId,
        subst: &HashMap<u32, Ty>,
    ) -> bool {
        self.current_func_is_generic && self.current_func_def_id == Some(def_id) && subst.is_empty()
    }

    fn infer_generic_single_arg_substitution(
        &mut self,
        arg: &Arg,
        expected: &Ty,
        subst: &mut HashMap<u32, Ty>,
    ) {
        let ArgValue::Expr(expr) = &arg.value else {
            return;
        };
        let actual = self.infer_expr(expr);
        infer_generic_substitution(expected, &actual, subst);
    }

    fn infer_generic_variadic_arg_substitution(
        &mut self,
        arg: &Arg,
        expected: &Ty,
        subst: &mut HashMap<u32, Ty>,
    ) {
        let actual = match &arg.value {
            ArgValue::Expr(expr) | ArgValue::Spread(expr) => self.infer_expr(expr),
        };
        let expected = match (&arg.value, expected) {
            (ArgValue::Expr(_), Ty::List(elem_ty)) => elem_ty.as_ref(),
            _ => expected,
        };
        infer_generic_substitution(expected, &actual, subst);
    }

    fn check_single_call_arg(&mut self, arg: &Arg, expected: &Ty, index: usize) {
        let expr = match &arg.value {
            ArgValue::Expr(e) => e.as_ref(),
            ArgValue::Spread(e) => {
                let actual = self.infer_expr(e);
                let code = if matches!(actual, Ty::List(_)) {
                    "type.spread_non_variadic"
                } else {
                    "type.spread_non_list"
                };
                self.sink.emit(
                    Diagnostic::error(code, "spread arguments require a variadic parameter")
                        .with_label(Label::primary(self.file_id, arg.span, "spread here"))
                        .with_action(
                            "use `..expr` only when passing values to a variadic parameter",
                        ),
                );
                return;
            }
        };
        if expr_needs_expected_context(expr) {
            let actual = self.check_expr_assignable_to(expr, expected);
            if !actual.is_error() {
                let _ = self.unify(&actual, expected);
            }
            return;
        }
        let actual = self.infer_expr(expr);
        if !self.unify(&actual, expected) {
            self.sink.emit(
                Diagnostic::error(
                    "type.arg_type_mismatch",
                    format!(
                        "argument {} expects `{}`, found `{}`",
                        index + 1,
                        expected.display(),
                        actual.display(),
                    ),
                )
                .with_label(Label::primary(self.file_id, arg.span, "this argument"))
                .with_action(format!(
                    "change the argument to produce `{}`",
                    expected.display()
                )),
            );
        }
    }

    fn check_variadic_call_arg(&mut self, arg: &Arg, elem_ty: &Ty, index: usize) {
        match &arg.value {
            ArgValue::Expr(expr) => {
                let actual = self.infer_expr(expr);
                if !self.unify(&actual, elem_ty) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.arg_type_mismatch",
                            format!(
                                "variadic argument {} expects `{}`, found `{}`",
                                index + 1,
                                elem_ty.display(),
                                actual.display(),
                            ),
                        )
                        .with_label(Label::primary(self.file_id, arg.span, "this argument"))
                        .with_action(format!(
                            "change the argument to produce `{}`",
                            elem_ty.display()
                        )),
                    );
                }
            }
            ArgValue::Spread(expr) => {
                let actual = self.infer_expr(expr);
                let expected = Ty::List(Box::new(elem_ty.clone()));
                if !self.unify(&actual, &expected) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.arg_type_mismatch",
                            format!(
                                "spread argument expects `{}`, found `{}`",
                                expected.display(),
                                actual.display(),
                            ),
                        )
                        .with_label(Label::primary(self.file_id, arg.span, "this argument"))
                        .with_action(format!("spread a `{}` value", expected.display())),
                    );
                }
            }
        }
    }

    fn infer_call_arg(&mut self, arg: &Arg) {
        let expr = match &arg.value {
            ArgValue::Expr(e) | ArgValue::Spread(e) => e.as_ref(),
        };
        self.infer_expr(expr);
    }

    fn infer_qualified_trait_method_call(
        &mut self,
        q: &QualifiedName,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        let trait_path = qualified_prefix(q)?;
        let method_name = q.last().as_str();
        let trait_def_id = self.resolve_def_id(&trait_path)?;
        if self.def_map.get(trait_def_id).kind != DefKind::Trait {
            return None;
        }

        let Some(method_sig) = self
            .trait_sig(trait_def_id)
            .and_then(|sig| sig.methods.iter().find(|method| method.name == method_name))
            .cloned()
        else {
            self.sink.emit(
                Diagnostic::error(
                    "type.no_such_method",
                    format!("trait `{}` has no method `{}`", trait_path, method_name),
                )
                .with_label(Label::primary(self.file_id, q.span, "trait method call"))
                .with_action("call a method declared by the trait"),
            );
            return Some(Ty::Error);
        };

        let Some(first_arg) = args.first() else {
            self.check_call_args(args, &method_sig.params, span);
            return Some(Ty::Error);
        };
        let first_expr = match &first_arg.value {
            ArgValue::Expr(expr) | ArgValue::Spread(expr) => expr.as_ref(),
        };
        let self_ty = self.infer_expr(first_expr);
        let Ty::Named(type_def_id, _) = &self_ty else {
            self.sink.emit(
                Diagnostic::error(
                    "type.arg_type_mismatch",
                    format!(
                        "qualified trait call expects a concrete receiver, found `{}`",
                        self_ty.display()
                    ),
                )
                .with_label(Label::primary(
                    self.file_id,
                    first_arg.span,
                    "receiver here",
                ))
                .with_action("pass a value whose type implements the trait"),
            );
            return Some(Ty::Error);
        };

        if !self.named_type_implements_trait(*type_def_id, trait_def_id) {
            self.sink.emit(
                Diagnostic::error(
                    "generic.constraint_not_satisfied",
                    format!(
                        "`{}` does not implement `{}`",
                        self_ty.display(),
                        self.def_map.get(trait_def_id).path
                    ),
                )
                .with_label(Label::primary(
                    self.file_id,
                    first_arg.span,
                    "receiver here",
                ))
                .with_action("implement the trait for this type before calling its method"),
            );
            return Some(Ty::Error);
        }

        let mut params = method_sig.params.clone();
        if let Some(first) = params.first_mut() {
            *first = self_ty.clone();
        }
        self.check_call_args(args, &params, span);
        Some(substitute_trait_self(
            &method_sig.return_ty,
            trait_def_id,
            &self_ty,
        ))
    }

    fn infer_stdlib_call(
        &mut self,
        path: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        let canonical_path = crate::stdlib::canonical_stdlib_path(path).unwrap_or(path);
        if let Some(ret) = self.infer_math_overload_call(canonical_path, args, span) {
            return Some(ret);
        }
        if let Some(ret) = self.infer_string_conversion_call(canonical_path, args, span) {
            return Some(ret);
        }
        if let Some(ret) = self.infer_primitive_conversion_call(canonical_path, args, span) {
            return Some(ret);
        }
        if let Some(ret) = self.infer_test_assert_equality_call(canonical_path, args, span) {
            return Some(ret);
        }
        if let Some(ret) = self.infer_iter_stdlib_call(canonical_path, args, span) {
            return Some(ret);
        }
        if let Some(ret) = self.infer_task_spawn_call(canonical_path, args, span) {
            return Some(ret);
        }
        let (params, ret) = stdlib_func_sig(path)?;
        let params: Vec<Ty> = params
            .into_iter()
            .map(|p| self.replace_json_placeholder(p))
            .collect();
        let ret = self.replace_json_placeholder(ret);
        // Warn if this stdlib function lacks native codegen support.
        if !crate::stdlib::stdlib_native_codegen_available(path) {
            self.sink.emit(
                Diagnostic::warning(
                    "bind.stdlib_module_unavailable",
                    format!("`{}` is not yet available in the native runtime", path,),
                )
                .with_label(Label::primary(self.file_id, span, "used here"))
                .with_action("use an alternative function or wait for native runtime support"),
            );
        }
        let (params, mut ret) = freshen_stdlib_infers(params, ret, span.start as u32);
        self.check_call_args(args, &params, span);
        let first_arg_ty = args.first().and_then(|arg| match &arg.value {
            ArgValue::Expr(expr) | ArgValue::Spread(expr) => Some(self.infer_expr(expr)),
        });
        let first_list_backed_collection_elem = first_arg_ty
            .as_ref()
            .and_then(Ty::list_backed_collection_elem)
            .cloned();
        let first_tree_elem = first_arg_ty.as_ref().and_then(|ty| match ty {
            Ty::Opaque {
                kind: OpaqueTy::Tree,
                args,
            } => args.first().cloned(),
            _ => None,
        });
        let first_hash_table_args = first_arg_ty.as_ref().and_then(|ty| match ty {
            Ty::Opaque {
                kind: OpaqueTy::HashTable,
                args,
            } if args.len() == 2 => Some((args[0].clone(), args[1].clone())),
            _ => None,
        });
        let first_graph_elem = first_arg_ty.as_ref().and_then(|ty| match ty {
            Ty::Opaque {
                kind: OpaqueTy::Graph,
                args,
            } => args.first().cloned(),
            _ => None,
        });
        let first_heap_elem = first_arg_ty.as_ref().and_then(|ty| match ty {
            Ty::Opaque {
                kind: OpaqueTy::Heap,
                args,
            } => args.first().cloned(),
            _ => None,
        });
        match (canonical_path, first_arg_ty.as_ref()) {
            ("ori.list.get", Some(Ty::List(elem))) => ret = *elem.clone(),
            ("ori.list.pop", Some(Ty::List(elem))) => ret = *elem.clone(),
            ("ori.list.try_get" | "ori.list.try_pop", Some(Ty::List(elem))) => {
                ret = Ty::Optional(elem.clone())
            }
            (
                "ori.list.slice" | "ori.list.clone" | "ori.list.to_list" | "ori.list.from_list",
                Some(Ty::List(elem)),
            ) => ret = Ty::List(elem.clone()),
            ("ori.list.push", Some(Ty::List(elem))) => {
                self.check_stdlib_arg_assignable(args, 1, elem);
            }
            ("ori.list.set" | "ori.list.insert", Some(Ty::List(elem))) => {
                self.check_stdlib_arg_assignable(args, 2, elem);
            }
            ("ori.list.contains" | "ori.list.index_of", Some(Ty::List(elem))) => {
                self.check_stdlib_arg_assignable(args, 1, elem);
            }
            (
                "ori.deque.push_front"
                | "ori.deque.push_back"
                | "ori.queue.enqueue"
                | "ori.stack.push"
                | "ori.linked_list.push_front"
                | "ori.linked_list.push_back"
                | "ori.doubly_linked_list.push_front"
                | "ori.doubly_linked_list.push_back",
                _,
            ) => {
                if let Some(elem) = first_list_backed_collection_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, elem);
                }
            }
            (
                "ori.linked_list.insert_after"
                | "ori.doubly_linked_list.insert_after"
                | "ori.doubly_linked_list.insert_before",
                _,
            ) => {
                if let Some(elem) = first_list_backed_collection_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 2, elem);
                }
            }
            (
                "ori.deque.pop_front"
                | "ori.deque.pop_back"
                | "ori.deque.front"
                | "ori.deque.back"
                | "ori.queue.dequeue"
                | "ori.queue.peek"
                | "ori.stack.pop"
                | "ori.stack.peek"
                | "ori.linked_list.pop_front"
                | "ori.linked_list.front"
                | "ori.doubly_linked_list.pop_front"
                | "ori.doubly_linked_list.pop_back"
                | "ori.doubly_linked_list.front"
                | "ori.doubly_linked_list.back",
                _,
            ) => {
                if let Some(elem) = first_list_backed_collection_elem.as_ref() {
                    ret = Ty::Optional(Box::new(elem.clone()));
                }
            }
            (
                "ori.linked_list.value_at"
                | "ori.linked_list.remove_at"
                | "ori.doubly_linked_list.value_at"
                | "ori.doubly_linked_list.remove_at",
                _,
            ) => {
                if let Some(elem) = first_list_backed_collection_elem.as_ref() {
                    ret = Ty::Optional(Box::new(elem.clone()));
                }
            }
            (
                "ori.linked_list.cursor_front"
                | "ori.linked_list.cursor_back"
                | "ori.linked_list.find"
                | "ori.doubly_linked_list.cursor_front"
                | "ori.doubly_linked_list.cursor_back"
                | "ori.doubly_linked_list.find",
                _,
            ) => {
                if path == "ori.linked_list.find" || path == "ori.doubly_linked_list.find" {
                    if let Some(elem) = first_list_backed_collection_elem.as_ref() {
                        self.check_stdlib_arg_assignable(args, 1, elem);
                    }
                }
                ret = Ty::Optional(Box::new(Ty::Int));
            }
            (
                "ori.deque.to_list"
                | "ori.queue.to_list"
                | "ori.stack.to_list"
                | "ori.linked_list.to_list"
                | "ori.doubly_linked_list.to_list",
                _,
            ) => {
                if let Some(elem) = first_list_backed_collection_elem.as_ref() {
                    ret = Ty::List(Box::new(elem.clone()));
                }
            }
            (
                "ori.deque.clone"
                | "ori.queue.clone"
                | "ori.stack.clone"
                | "ori.linked_list.clone"
                | "ori.doubly_linked_list.clone",
                _,
            ) => {
                if let Some(first) = first_arg_ty.as_ref() {
                    ret = first.clone();
                }
            }
            ("ori.tree.value", _) => {
                if let Some(elem) = first_tree_elem.as_ref() {
                    ret = elem.clone();
                }
            }
            ("ori.tree.try_value", _) => {
                if let Some(elem) = first_tree_elem.as_ref() {
                    ret = Ty::Optional(Box::new(elem.clone()));
                }
            }
            ("ori.tree.add_child" | "ori.tree.set_value", _) => {
                if let Some(elem) = first_tree_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 2, elem);
                }
            }
            ("ori.tree.find", _) => {
                if let Some(elem) = first_tree_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, elem);
                }
                ret = Ty::Optional(Box::new(Ty::Opaque {
                    kind: OpaqueTy::NodeId,
                    args: vec![],
                }));
            }
            (
                "ori.tree.children"
                | "ori.tree.pre_order"
                | "ori.tree.post_order"
                | "ori.tree.breadth_first",
                _,
            ) => {
                ret = Ty::List(Box::new(Ty::Opaque {
                    kind: OpaqueTy::NodeId,
                    args: vec![],
                }));
            }
            ("ori.tree.parent", _) => {
                ret = Ty::Optional(Box::new(Ty::Opaque {
                    kind: OpaqueTy::NodeId,
                    args: vec![],
                }));
            }
            ("ori.tree.clone" | "ori.tree.clone_subtree", _) => {
                if let Some(first) = first_arg_ty.as_ref() {
                    ret = first.clone();
                }
            }
            ("ori.map.set", Some(Ty::Map(key, value))) => {
                self.check_stdlib_arg_assignable(args, 1, key);
                self.check_stdlib_arg_assignable(args, 2, value);
            }
            ("ori.map.get", Some(Ty::Map(key, value))) => {
                self.check_stdlib_arg_assignable(args, 1, key);
                ret = *value.clone();
            }
            ("ori.map.try_get" | "ori.map.try_remove", Some(Ty::Map(key, value))) => {
                self.check_stdlib_arg_assignable(args, 1, key);
                ret = Ty::Optional(value.clone());
            }
            ("ori.map.contains" | "ori.map.remove", Some(Ty::Map(key, _))) => {
                self.check_stdlib_arg_assignable(args, 1, key);
            }
            ("ori.map.clone", Some(Ty::Map(_, _))) => {
                ret = first_arg_ty.as_ref().unwrap().clone();
            }
            ("ori.map.keys", Some(Ty::Map(key, _))) => ret = Ty::List(Box::new(*key.clone())),
            ("ori.map.values", Some(Ty::Map(_, value))) => {
                ret = Ty::List(Box::new(*value.clone()));
            }
            ("ori.map.entries", Some(Ty::Map(key, value))) => {
                ret = Ty::List(Box::new(Ty::Tuple(vec![*key.clone(), *value.clone()])));
            }
            ("ori.hash_table.set", _) => {
                if let Some((key, value)) = first_hash_table_args.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, key);
                    self.check_stdlib_arg_assignable(args, 2, value);
                }
            }
            ("ori.hash_table.get" | "ori.hash_table.remove", _) => {
                if let Some((key, value)) = first_hash_table_args.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, key);
                    ret = Ty::Optional(Box::new(value.clone()));
                }
            }
            ("ori.hash_table.contains", _) => {
                if let Some((key, _)) = first_hash_table_args.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, key);
                }
            }
            ("ori.hash_table.keys", _) => {
                if let Some((key, _)) = first_hash_table_args.as_ref() {
                    ret = Ty::List(Box::new(key.clone()));
                }
            }
            ("ori.hash_table.values", _) => {
                if let Some((_, value)) = first_hash_table_args.as_ref() {
                    ret = Ty::List(Box::new(value.clone()));
                }
            }
            ("ori.hash_table.entries", _) => {
                if let Some((key, value)) = first_hash_table_args.as_ref() {
                    ret = Ty::List(Box::new(Ty::Tuple(vec![key.clone(), value.clone()])));
                }
            }
            ("ori.hash_table.clone", _) => {
                if let Some(first) = first_arg_ty.as_ref() {
                    ret = first.clone();
                }
            }
            ("ori.graph.add_node" | "ori.graph.remove_node" | "ori.graph.has_node", _) => {
                if let Some(elem) = first_graph_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, elem);
                }
            }
            (
                "ori.graph.add_edge"
                | "ori.graph.remove_edge"
                | "ori.graph.has_edge"
                | "ori.graph.edge_weight"
                | "ori.graph.shortest_path"
                | "ori.graph.shortest_weighted_path",
                _,
            ) => {
                if let Some(elem) = first_graph_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, elem);
                    self.check_stdlib_arg_assignable(args, 2, elem);
                }
                if path == "ori.graph.edge_weight" {
                    ret = Ty::Optional(Box::new(Ty::Int));
                }
                if path == "ori.graph.shortest_path" || path == "ori.graph.shortest_weighted_path" {
                    if let Some(elem) = first_graph_elem.as_ref() {
                        ret = Ty::Optional(Box::new(Ty::List(Box::new(elem.clone()))));
                    }
                }
            }
            ("ori.graph.add_weighted_edge", _) => {
                if let Some(elem) = first_graph_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, elem);
                    self.check_stdlib_arg_assignable(args, 2, elem);
                }
                self.check_stdlib_arg_assignable(args, 3, &Ty::Int);
            }
            ("ori.graph.neighbors" | "ori.graph.bfs" | "ori.graph.dfs", _) => {
                if let Some(elem) = first_graph_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, elem);
                    ret = Ty::List(Box::new(elem.clone()));
                }
            }
            ("ori.graph.nodes" | "ori.graph.topological_sort", _) => {
                if let Some(elem) = first_graph_elem.as_ref() {
                    ret = Ty::List(Box::new(elem.clone()));
                }
            }
            ("ori.graph.try_topological_sort", _) => {
                if let Some(elem) = first_graph_elem.as_ref() {
                    ret = Ty::Optional(Box::new(Ty::List(Box::new(elem.clone()))));
                }
            }
            ("ori.graph.components" | "ori.graph.strongly_connected_components", _) => {
                if let Some(elem) = first_graph_elem.as_ref() {
                    ret = Ty::List(Box::new(Ty::List(Box::new(elem.clone()))));
                }
            }
            ("ori.graph.transitive_closure" | "ori.graph.clone", _) => {
                if let Some(first) = first_arg_ty.as_ref() {
                    ret = first.clone();
                }
            }
            ("ori.graph.edges", _) => {
                if let Some(elem) = first_graph_elem.as_ref() {
                    ret = Ty::List(Box::new(Ty::Tuple(vec![elem.clone(), elem.clone()])));
                }
            }
            ("ori.heap.push", _) => {
                if let Some(elem) = first_heap_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, elem);
                }
            }
            ("ori.heap.pop" | "ori.heap.peek", _) => {
                if let Some(elem) = first_heap_elem.as_ref() {
                    ret = Ty::Optional(Box::new(elem.clone()));
                }
            }
            ("ori.heap.clone" | "ori.heap.merge", _) => {
                if let Some(first) = first_arg_ty.as_ref() {
                    ret = first.clone();
                }
            }
            ("ori.heap.to_list" | "ori.heap.into_sorted_list", _) => {
                if let Some(elem) = first_heap_elem.as_ref() {
                    ret = Ty::List(Box::new(elem.clone()));
                }
            }
            ("ori.heap.from_list", _) => {
                if let Some(arg) = args.first() {
                    let arg_ty = match &arg.value {
                        ArgValue::Expr(expr) | ArgValue::Spread(expr) => self.infer_expr(expr),
                    };
                    if let Ty::List(elem) = arg_ty {
                        ret = Ty::Opaque {
                            kind: OpaqueTy::Heap,
                            args: vec![*elem],
                        };
                    }
                }
            }
            ("ori.heap.remove", _) => {
                if let Some(elem) = first_heap_elem.as_ref() {
                    self.check_stdlib_arg_assignable(args, 1, elem);
                }
            }
            (
                "ori.set.add" | "ori.set.contains" | "ori.set.remove" | "ori.set.try_remove",
                Some(Ty::Set(elem)),
            ) => {
                self.check_stdlib_arg_assignable(args, 1, elem);
            }
            ("ori.set.clone", Some(Ty::Set(_))) => {
                ret = first_arg_ty.as_ref().unwrap().clone();
            }
            ("ori.set.to_list", Some(Ty::Set(elem))) => {
                ret = Ty::List(elem.clone());
            }
            ("ori.set.union" | "ori.set.intersection" | "ori.set.difference", Some(Ty::Set(_))) => {
                self.check_stdlib_arg_assignable(args, 1, first_arg_ty.as_ref().unwrap());
            }
            (
                "ori.lazy.once",
                Some(Ty::Func {
                    params,
                    ret: thunk_ret,
                }),
            ) => {
                if params.is_empty() {
                    ret = Ty::Lazy(thunk_ret.clone());
                }
            }
            ("ori.lazy.force", Some(Ty::Lazy(inner))) => ret = *inner.clone(),
            ("ori.task.join", Some(Ty::TaskJob(inner))) => {
                ret = Ty::Result(inner.clone(), Box::new(Ty::TaskJoinError));
            }
            ("ori.task.detach", Some(Ty::TaskJob(_))) => {}
            ("ori.task.block_on", Some(Ty::Future(inner))) => ret = *inner.clone(),
            ("ori.channel.send", Some(Ty::Channel(elem))) => {
                self.check_stdlib_arg_assignable(args, 1, elem);
                if let Some(sent) = args.get(1).map(|arg| self.infer_arg_ty(arg)) {
                    self.expect_transferable_ty(&sent, args[1].span);
                }
            }
            ("ori.channel.receive", Some(Ty::Channel(elem))) => {
                ret = Ty::Result(elem.clone(), Box::new(Ty::ChannelReceiveError));
            }
            ("ori.channel.close", Some(Ty::Channel(_))) => {}
            _ => {}
        }
        Some(ret)
    }

    fn replace_json_placeholder(&self, ty: Ty) -> Ty {
        let json_val_def_id = self.def_map.lookup("ori.json.Value");
        fn recurse(t: Ty, json_val_def_id: Option<DefId>) -> Ty {
            match t {
                Ty::Named(id, _) if id == crate::stdlib::JSON_VALUE_PLACEHOLDER => {
                    if let Some(concrete_id) = json_val_def_id {
                        Ty::Named(concrete_id, vec![])
                    } else {
                        Ty::Named(id, vec![])
                    }
                }
                Ty::Named(id, args) => {
                    let new_args = args
                        .into_iter()
                        .map(|arg| recurse(arg, json_val_def_id))
                        .collect();
                    Ty::Named(id, new_args)
                }
                Ty::Optional(inner) => Ty::Optional(Box::new(recurse(*inner, json_val_def_id))),
                Ty::Result(ok, err) => Ty::Result(
                    Box::new(recurse(*ok, json_val_def_id)),
                    Box::new(recurse(*err, json_val_def_id)),
                ),
                Ty::List(inner) => Ty::List(Box::new(recurse(*inner, json_val_def_id))),
                Ty::Map(k, v) => Ty::Map(
                    Box::new(recurse(*k, json_val_def_id)),
                    Box::new(recurse(*v, json_val_def_id)),
                ),
                Ty::Set(inner) => Ty::Set(Box::new(recurse(*inner, json_val_def_id))),
                Ty::Range(inner) => Ty::Range(Box::new(recurse(*inner, json_val_def_id))),
                Ty::Tuple(elems) => Ty::Tuple(
                    elems
                        .into_iter()
                        .map(|e| recurse(e, json_val_def_id))
                        .collect(),
                ),
                Ty::Lazy(inner) => Ty::Lazy(Box::new(recurse(*inner, json_val_def_id))),
                Ty::Future(inner) => Ty::Future(Box::new(recurse(*inner, json_val_def_id))),
                Ty::TaskJob(inner) => Ty::TaskJob(Box::new(recurse(*inner, json_val_def_id))),
                Ty::Channel(inner) => Ty::Channel(Box::new(recurse(*inner, json_val_def_id))),
                Ty::Func { params, ret } => Ty::Func {
                    params: params
                        .into_iter()
                        .map(|p| recurse(p, json_val_def_id))
                        .collect(),
                    ret: Box::new(recurse(*ret, json_val_def_id)),
                },
                Ty::Opaque { kind, args } => Ty::Opaque {
                    kind,
                    args: args
                        .into_iter()
                        .map(|a| recurse(a, json_val_def_id))
                        .collect(),
                },
                other => other,
            }
        }
        recurse(ty, json_val_def_id)
    }

    fn infer_task_spawn_call(
        &mut self,
        path: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        if path != "ori.task.spawn" {
            return None;
        }
        self.check_call_args(
            args,
            &[Ty::Func {
                params: vec![],
                ret: Box::new(Ty::Infer(0)),
            }],
            span,
        );
        let Some(arg) = args.first() else {
            return Some(Ty::TaskJob(Box::new(Ty::Infer(0))));
        };

        self.transferable_closure_depth += 1;
        let work_ty = self.infer_arg_ty(arg);
        self.transferable_closure_depth -= 1;

        match work_ty {
            Ty::Func { params, ret } => {
                self.expect_closure_params(&params, &[], arg.span);
                self.expect_transferable_ty(&ret, arg.span);
                Some(Ty::TaskJob(ret))
            }
            Ty::Error => Some(Ty::TaskJob(Box::new(Ty::Infer(0)))),
            other if other.contains_infer() => Some(Ty::TaskJob(Box::new(Ty::Infer(0)))),
            other => {
                self.sink.emit(
                    Diagnostic::error(
                        "type.arg_type_mismatch",
                        format!(
                            "`task.spawn` expects `func() -> T`, found `{}`",
                            other.display()
                        ),
                    )
                    .with_label(Label::primary(self.file_id, arg.span, "work value here"))
                    .with_action("pass a no-argument function or closure"),
                );
                Some(Ty::TaskJob(Box::new(Ty::Infer(0))))
            }
        }
    }

    fn infer_test_assert_equality_call(
        &mut self,
        path: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        if !matches!(path, "ori.test.assert_eq" | "ori.test.assert_ne") {
            return None;
        }
        self.check_call_args(args, &[Ty::Infer(0), Ty::Infer(0)], span);
        if args.len() != 2 {
            return Some(Ty::Void);
        }

        let left = self.infer_arg_ty(&args[0]);
        let right = self.infer_arg_ty(&args[1]);
        if left.is_error() || right.is_error() {
            return Some(Ty::Void);
        }
        if !self.same_comparison_type(&left, &right) {
            self.emit_comparison_type_mismatch(&left, &right, span);
            return Some(Ty::Void);
        }
        if !self.supports_generic_equality(&left) {
            self.emit_comparison_not_supported(
                if path == "ori.test.assert_eq" {
                    BinaryOp::Eq
                } else {
                    BinaryOp::Ne
                },
                &left,
                span,
            );
        }
        Some(Ty::Void)
    }

    fn infer_iter_stdlib_call(
        &mut self,
        path: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        if !path.starts_with("ori.iter.") {
            return None;
        }
        let (params, fallback_ret) = stdlib_func_sig(path)?;
        self.check_call_args(args, &params, span);
        let first_ty = args.first().map(|arg| self.infer_arg_ty(arg));
        let elem_ty = match first_ty.as_ref() {
            Some(Ty::List(elem)) => *elem.clone(),
            _ => return Some(fallback_ret),
        };

        match path {
            "ori.iter.map" => {
                let Some((closure_params, closure_ret, closure_span)) =
                    self.infer_closure_arg(args, 1)
                else {
                    return Some(Ty::List(Box::new(Ty::Infer(0))));
                };
                self.expect_closure_params(&closure_params, &[elem_ty], closure_span);
                Some(Ty::List(Box::new(closure_ret)))
            }
            "ori.iter.filter" => {
                self.expect_unary_predicate(args, 1, &elem_ty);
                Some(Ty::List(Box::new(elem_ty)))
            }
            "ori.iter.any" | "ori.iter.all" => {
                self.expect_unary_predicate(args, 1, &elem_ty);
                Some(Ty::Bool)
            }
            "ori.iter.count_where" => {
                self.expect_unary_predicate(args, 1, &elem_ty);
                Some(Ty::Int)
            }
            "ori.iter.take" | "ori.iter.skip" | "ori.iter.reverse" => {
                Some(Ty::List(Box::new(elem_ty)))
            }
            "ori.iter.reduce" => {
                let initial_ty = args.get(1).map(|arg| self.infer_arg_ty(arg))?;
                let Some((closure_params, closure_ret, closure_span)) =
                    self.infer_closure_arg(args, 2)
                else {
                    return Some(initial_ty);
                };
                self.expect_closure_params(
                    &closure_params,
                    &[initial_ty.clone(), elem_ty],
                    closure_span,
                );
                self.expect_assignable(&closure_ret, &initial_ty, closure_span);
                Some(initial_ty)
            }
            "ori.iter.find" => {
                self.expect_unary_predicate(args, 1, &elem_ty);
                Some(Ty::Optional(Box::new(elem_ty)))
            }
            "ori.iter.flat_map" => {
                let Some((closure_params, closure_ret, closure_span)) =
                    self.infer_closure_arg(args, 1)
                else {
                    return Some(Ty::List(Box::new(Ty::Infer(0))));
                };
                self.expect_closure_params(&closure_params, &[elem_ty], closure_span);
                match closure_ret {
                    Ty::List(inner) => Some(Ty::List(inner)),
                    other => {
                        self.expect_assignable(
                            &other,
                            &Ty::List(Box::new(Ty::Infer(0))),
                            closure_span,
                        );
                        Some(Ty::List(Box::new(Ty::Infer(0))))
                    }
                }
            }
            "ori.iter.sort" => {
                if !self.supports_iter_sort(&elem_ty) {
                    self.emit_comparison_not_supported(BinaryOp::Lt, &elem_ty, span);
                }
                Some(Ty::List(Box::new(elem_ty)))
            }
            "ori.iter.sort_by" => {
                let Some((closure_params, closure_ret, closure_span)) =
                    self.infer_closure_arg(args, 1)
                else {
                    return Some(Ty::List(Box::new(elem_ty)));
                };
                self.expect_closure_params(
                    &closure_params,
                    &[elem_ty.clone(), elem_ty.clone()],
                    closure_span,
                );
                self.expect_assignable(&closure_ret, &Ty::Int, closure_span);
                Some(Ty::List(Box::new(elem_ty)))
            }
            "ori.iter.unique" => {
                if !self.supports_generic_equality(&elem_ty) {
                    self.emit_comparison_not_supported(BinaryOp::Eq, &elem_ty, span);
                }
                Some(Ty::List(Box::new(elem_ty)))
            }
            "ori.iter.zip" => {
                let right_ty = args.get(1).map(|arg| self.infer_arg_ty(arg));
                match right_ty {
                    Some(Ty::List(right_elem)) => {
                        Some(Ty::List(Box::new(Ty::Tuple(vec![elem_ty, *right_elem]))))
                    }
                    _ => Some(Ty::List(Box::new(Ty::Tuple(vec![elem_ty, Ty::Infer(0)])))),
                }
            }
            "ori.iter.partition" => {
                self.expect_unary_predicate(args, 1, &elem_ty);
                Some(Ty::Tuple(vec![
                    Ty::List(Box::new(elem_ty.clone())),
                    Ty::List(Box::new(elem_ty)),
                ]))
            }
            "ori.iter.group_by" => {
                let Some((closure_params, key_ty, closure_span)) = self.infer_closure_arg(args, 1)
                else {
                    return Some(fallback_ret);
                };
                self.expect_closure_params(&closure_params, &[elem_ty.clone()], closure_span);
                if !self.is_current_map_key_supported(&key_ty) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.hash_key_not_supported",
                            format!(
                                "`iter.group_by` keys currently require `int`, `string`, or a type implementing `Hashable` and `Equatable`, found `{}`",
                                key_ty.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, closure_span, "key produced here"))
                        .with_action("return an `int`, `string`, or Hashable/Equatable value from the key function"),
                    );
                }
                Some(Ty::Map(
                    Box::new(key_ty),
                    Box::new(Ty::List(Box::new(elem_ty))),
                ))
            }
            "ori.iter.flatten" => match elem_ty {
                Ty::List(inner) => Some(Ty::List(inner)),
                other => {
                    self.expect_assignable(
                        &Ty::List(Box::new(other)),
                        &Ty::List(Box::new(Ty::List(Box::new(Ty::Infer(0))))),
                        span,
                    );
                    Some(Ty::List(Box::new(Ty::Infer(0))))
                }
            },
            _ => Some(fallback_ret),
        }
    }

    fn infer_arg_ty(&mut self, arg: &Arg) -> Ty {
        match &arg.value {
            ArgValue::Expr(expr) | ArgValue::Spread(expr) => self.infer_expr(expr),
        }
    }

    fn infer_closure_arg(
        &mut self,
        args: &[Arg],
        index: usize,
    ) -> Option<(Vec<Ty>, Ty, ori_diagnostics::Span)> {
        let arg = args.get(index)?;
        let ty = self.infer_arg_ty(arg);
        match ty {
            Ty::Func { params, ret } => Some((params, *ret, arg.span)),
            _ => None,
        }
    }

    fn expect_unary_predicate(&mut self, args: &[Arg], index: usize, elem_ty: &Ty) {
        let Some((closure_params, closure_ret, closure_span)) = self.infer_closure_arg(args, index)
        else {
            return;
        };
        self.expect_closure_params(&closure_params, &[elem_ty.clone()], closure_span);
        self.expect_assignable(&closure_ret, &Ty::Bool, closure_span);
    }

    fn expect_closure_params(
        &mut self,
        actual: &[Ty],
        expected: &[Ty],
        span: ori_diagnostics::Span,
    ) {
        if actual.len() != expected.len() {
            self.sink.emit(
                Diagnostic::error(
                    "type.arg_count_mismatch",
                    format!(
                        "closure expects {} parameter(s), got {}",
                        expected.len(),
                        actual.len()
                    ),
                )
                .with_label(Label::primary(self.file_id, span, "closure here"))
                .with_action("match the closure parameters to the iterator helper"),
            );
            return;
        }
        for (actual_ty, expected_ty) in actual.iter().zip(expected) {
            self.expect_assignable(actual_ty, expected_ty, span);
        }
    }

    fn infer_string_conversion_call(
        &mut self,
        path: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        if path != "string" {
            return None;
        }

        self.check_call_args(args, &[Ty::Infer(0)], span);
        if args.len() != 1 {
            return Some(Ty::Error);
        }

        let arg = &args[0];
        let expr = match &arg.value {
            ArgValue::Expr(expr) | ArgValue::Spread(expr) => expr.as_ref(),
        };
        let arg_ty = self.infer_expr(expr);
        if self.supports_string_conversion_ty(&arg_ty) {
            return Some(Ty::String);
        }
        if arg_ty.is_error() || arg_ty.contains_infer() {
            return Some(Ty::String);
        }

        self.sink.emit(
            Diagnostic::error(
                "type.arg_type_mismatch",
                format!(
                    "`string` expects `int`, `float`, `bool`, `string`, or a `Displayable` value, found `{}`",
                    arg_ty.display()
                ),
            )
            .with_label(Label::primary(
                self.file_id,
                arg.span,
                "value converted here",
            ))
            .with_action("pass a scalar/string value or implement `ori.core.Displayable`"),
        );
        Some(Ty::Error)
    }

    fn supports_string_conversion_ty(&self, ty: &Ty) -> bool {
        ty.is_integer()
            || ty.is_float()
            || matches!(ty, Ty::Bool | Ty::String)
            || self.user_type_implements_core_trait(ty, "Displayable")
    }

    fn infer_primitive_conversion_call(
        &mut self,
        path: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        let ret = match path {
            "int" => Ty::Int,
            "float" => Ty::Float,
            _ => return None,
        };

        self.check_call_args(args, &[Ty::Infer(0)], span);
        if args.len() != 1 {
            return Some(Ty::Error);
        }

        let arg = &args[0];
        let expr = match &arg.value {
            ArgValue::Expr(expr) | ArgValue::Spread(expr) => expr.as_ref(),
        };
        let arg_ty = self.infer_expr(expr);
        let accepted = match path {
            "int" => arg_ty.is_integer(),
            "float" => arg_ty.is_integer(),
            _ => false,
        };
        if accepted || arg_ty.is_error() || arg_ty.contains_infer() {
            return Some(ret);
        }

        self.sink.emit(
            Diagnostic::error(
                "type.arg_type_mismatch",
                format!(
                    "`{}` expects an integer value, found `{}`",
                    path,
                    arg_ty.display()
                ),
            )
            .with_label(Label::primary(
                self.file_id,
                arg.span,
                "value converted here",
            ))
            .with_action("pass an integer value to this primitive conversion"),
        );
        Some(Ty::Error)
    }

    fn infer_never_form_call(
        &mut self,
        name: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        match name {
            "panic" => {
                self.check_call_args(args, &[Ty::String], span);
                Some(Ty::Never)
            }
            "todo" | "unreachable" => {
                let param_names = [SmolStr::new("message")];
                let param_defaults = [true];
                let param_variadic = [false];
                self.check_call_args_with_defaults(
                    args,
                    &[Ty::String],
                    &param_names,
                    &param_defaults,
                    &param_variadic,
                    span,
                );
                Some(Ty::Never)
            }
            _ => None,
        }
    }

    fn infer_wrapper_form_call(
        &mut self,
        name: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        if (name == "ok" || name == "Ok") && args.is_empty() {
            if let Some(Ty::Result(ok_ty, err_ty)) = self.current_return_ty.clone() {
                if !self.unify(&Ty::Void, &ok_ty) {
                    self.sink.emit(
                        Diagnostic::error(
                            "contract.ok_void_mismatch",
                            format!(
                                "`ok()` is only valid for `result[void, E]`, found ok type `{}`",
                                ok_ty.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, span, "`ok()` used here"))
                        .with_action("pass an ok value, or change the result ok type to `void`"),
                    );
                }
                return Some(Ty::Result(ok_ty, err_ty));
            } else {
                return Some(Ty::Result(Box::new(Ty::Void), Box::new(Ty::String)));
            }
        }

        let arg_ty = match name {
            "some" | "Some" | "ok" | "Ok" | "err" | "Err" => {
                self.infer_single_wrapper_arg(args, span)
            }
            _ => return None,
        };

        Some(match name {
            "some" | "Some" => {
                if let Some(Ty::Optional(expected)) = self.current_return_ty.clone() {
                    self.expect_assignable(
                        &arg_ty,
                        &expected,
                        args.first().map_or(span, |a| a.span),
                    );
                    Ty::Optional(expected.clone())
                } else {
                    Ty::Optional(Box::new(arg_ty))
                }
            }
            "ok" | "Ok" => {
                if let Some(Ty::Result(ok_ty, err_ty)) = self.current_return_ty.clone() {
                    self.expect_assignable(&arg_ty, &ok_ty, args.first().map_or(span, |a| a.span));
                    Ty::Result(ok_ty.clone(), err_ty.clone())
                } else {
                    Ty::Result(Box::new(arg_ty), Box::new(Ty::String))
                }
            }
            "err" | "Err" => {
                if let Some(Ty::Result(ok_ty, err_ty)) = self.current_return_ty.clone() {
                    self.expect_assignable(&arg_ty, &err_ty, args.first().map_or(span, |a| a.span));
                    Ty::Result(ok_ty.clone(), err_ty.clone())
                } else {
                    Ty::Result(Box::new(Ty::Void), Box::new(arg_ty))
                }
            }
            _ => {
                // Defensive: the early match at lines 2607-2612 should prevent
                // reaching this branch, but emit an ICE diagnostic instead of
                // panicking if a future refactor breaks the guard.
                eprintln!(
                    "ori: ICE: unexpected wrapper name `{}` in infer_wrapper_form_call",
                    name
                );
                Ty::Error
            }
        })
    }

    fn infer_single_wrapper_arg(&mut self, args: &[Arg], span: ori_diagnostics::Span) -> Ty {
        if args.len() != 1 {
            self.sink.emit(
                Diagnostic::error(
                    "type.arg_count_mismatch",
                    format!("function expects 1 argument(s), got {}", args.len()),
                )
                .with_label(Label::primary(self.file_id, span, "called here"))
                .with_action("pass exactly one value"),
            );
            for arg in args {
                self.infer_call_arg(arg);
            }
            return Ty::Error;
        }

        let arg = &args[0];
        match &arg.value {
            ArgValue::Expr(expr) => self.infer_expr(expr),
            ArgValue::Spread(expr) => {
                self.infer_expr(expr);
                self.sink.emit(
                    Diagnostic::error(
                        "type.spread_non_variadic",
                        "spread arguments are not accepted here",
                    )
                    .with_label(Label::primary(self.file_id, arg.span, "spread here"))
                    .with_action("pass the wrapped value directly"),
                );
                Ty::Error
            }
        }
    }

    fn infer_math_overload_call(
        &mut self,
        path: &str,
        args: &[Arg],
        span: ori_diagnostics::Span,
    ) -> Option<Ty> {
        let arity = match path {
            "ori.math.abs" => 1,
            "ori.math.min" | "ori.math.max" => 2,
            _ => return None,
        };
        let expected = vec![Ty::Infer(0); arity];
        self.check_call_args(args, &expected, span);
        if args.len() != arity {
            return Some(Ty::Error);
        }

        let arg_tys: Vec<Ty> = args
            .iter()
            .map(|arg| {
                let expr = match &arg.value {
                    ArgValue::Expr(expr) | ArgValue::Spread(expr) => expr.as_ref(),
                };
                self.infer_expr(expr)
            })
            .collect();
        if arg_tys.iter().any(Ty::is_error) {
            return Some(Ty::Error);
        }
        if arg_tys.iter().all(Ty::is_integer) {
            return Some(Ty::Int);
        }
        if arg_tys.iter().all(Ty::is_float) {
            return Some(Ty::Float);
        }

        let expected_text = if arity == 1 {
            "`int` or `float`"
        } else {
            "matching `int` values or matching `float` values"
        };
        self.sink.emit(
            Diagnostic::error(
                "type.arg_type_mismatch",
                format!(
                    "`{}` expects {}, found `{}`",
                    path,
                    expected_text,
                    display_tys(&arg_tys)
                ),
            )
            .with_label(Label::primary(self.file_id, span, "math call here"))
            .with_action("use either all integer arguments or all floating-point arguments"),
        );
        Some(Ty::Error)
    }

    fn check_stdlib_arg_assignable(&mut self, args: &[Arg], index: usize, expected: &Ty) {
        let Some(arg) = args.get(index) else {
            return;
        };
        let expr = match &arg.value {
            ArgValue::Expr(expr) | ArgValue::Spread(expr) => expr.as_ref(),
        };
        let actual = self.infer_expr(expr);
        self.expect_assignable(&actual, expected, arg.span);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn check_is_target(&mut self, ty: &QualifiedName, span: ori_diagnostics::Span) {
        let name = ty.to_string();
        if is_builtin_type_name(ty) {
            return;
        }

        let Some(def_id) = self.resolve_def_id(&name) else {
            self.sink.emit(
                Diagnostic::error("type.undefined_name", format!("undefined type `{}`", name))
                    .with_label(Label::primary(self.file_id, ty.span, "type used here"))
                    .with_action("use a type that is defined or imported in this namespace"),
            );
            return;
        };

        self.check_visibility(def_id, ty.span);
        if !matches!(
            self.def_map.get(def_id).kind,
            DefKind::Struct | DefKind::Enum | DefKind::Trait | DefKind::TypeAlias
        ) {
            self.sink.emit(
                Diagnostic::error(
                    "type.is_target_not_type",
                    format!("`{}` cannot be used as a type check target", name),
                )
                .with_label(Label::primary(self.file_id, span, "`is` expression here"))
                .with_action("use a struct, enum, trait, alias, or primitive type after `is`"),
            );
        }
    }

    fn check_struct_constructor_args(&mut self, def_id: DefId, args: &[Arg]) {
        let fields = self
            .struct_sigs
            .iter()
            .find(|s| s.def_id == def_id)
            .map(|s| s.fields.clone())
            .unwrap_or_default();
        let mut provided = HashSet::new();

        for arg in args {
            let expr = match &arg.value {
                ArgValue::Expr(e) | ArgValue::Spread(e) => e.as_ref(),
            };
            let Some(label) = &arg.label else {
                self.infer_expr(expr);
                self.sink.emit(
                    Diagnostic::error(
                        "type.struct_literal_named_fields_required",
                        "struct construction requires named fields",
                    )
                    .with_label(Label::primary(self.file_id, arg.span, "field name missing"))
                    .with_action("write the argument as `field: value`"),
                );
                continue;
            };

            if let Some((_, expected)) = fields.iter().find(|(name, _)| name == &label.text) {
                provided.insert(label.text.clone());
                self.check_expr_assignable_to(expr, expected);
            } else {
                self.infer_expr(expr);
                self.sink.emit(
                    Diagnostic::error(
                        "type.no_such_field",
                        format!("struct has no field `{}`", label.text),
                    )
                    .with_label(Label::primary(self.file_id, label.span, "unknown field"))
                    .with_action("use a field declared on the struct"),
                );
            }
        }

        for (name, _) in fields {
            if !provided.contains(&name) {
                self.sink.emit(
                    Diagnostic::error(
                        "type.missing_struct_field",
                        format!("struct construction is missing field `{}`", name),
                    )
                    .with_action(format!("provide `{}` in the struct literal", name)),
                );
            }
        }
    }

    fn check_enum_variant_args(&mut self, args: &[Arg]) {
        let mut provided = HashSet::new();
        for arg in args {
            let expr = match &arg.value {
                ArgValue::Expr(e) | ArgValue::Spread(e) => e.as_ref(),
            };
            if let Some(label) = &arg.label {
                if !provided.insert(label.text.clone()) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.anon_struct_field_mismatch",
                            format!("enum variant repeats field `{}`", label.text),
                        )
                        .with_label(Label::primary(self.file_id, label.span, "duplicate field"))
                        .with_action("keep each field only once"),
                    );
                }
            } else {
                self.sink.emit(
                    Diagnostic::error(
                        "type.enum_variant_named_fields_required",
                        "enum variant construction requires named fields",
                    )
                    .with_label(Label::primary(self.file_id, arg.span, "field name missing"))
                    .with_action("write the argument as `field: value`"),
                );
            }
            self.infer_expr(expr);
        }
    }

    fn lower(&mut self, ty: &ori_ast::ty::Type, type_params: &[SmolStr]) -> Ty {
        self.mark_type_alias_usage(ty);
        let raw = lower_type_with_aliases(
            ty,
            self.namespace,
            type_params,
            self.def_map,
            self.file_id,
            self.sink,
            &self.aliases,
        );
        expand_ty_aliases(raw, self.def_map, &self.type_alias_map)
    }

    /// Walk an AST type and mark any alias prefix as used.
    fn mark_type_alias_usage(&mut self, ty: &ori_ast::ty::Type) {
        use ori_ast::ty::Type as T;
        match ty {
            T::Named(q) | T::Any(q, _) => {
                if let Some(first) = q.parts.first() {
                    if self.aliases.contains_key(&first.text) {
                        self.used_aliases.insert(first.text.clone());
                    }
                }
            }
            T::Generic { name, args, .. } => {
                if let Some(first) = name.parts.first() {
                    if self.aliases.contains_key(&first.text) {
                        self.used_aliases.insert(first.text.clone());
                    }
                }
                for arg in args {
                    self.mark_type_alias_usage(arg);
                }
            }
            T::Optional(inner, _)
            | T::List(inner, _)
            | T::Set(inner, _)
            | T::Range(inner, _)
            | T::Lazy(inner, _) => self.mark_type_alias_usage(inner),
            T::Result(a, b, _) | T::Map(a, b, _) => {
                self.mark_type_alias_usage(a);
                self.mark_type_alias_usage(b);
            }
            T::Tuple(elems, _) => {
                for e in elems {
                    self.mark_type_alias_usage(e);
                }
            }
            T::Func {
                params, return_ty, ..
            } => {
                for p in params {
                    self.mark_type_alias_usage(p);
                }
                if let Some(r) = return_ty {
                    self.mark_type_alias_usage(r);
                }
            }
            _ => {} // primitives
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope::default());
    }
    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind(&mut self, name: SmolStr, ty: Ty) {
        if let Some(s) = self.scopes.last_mut() {
            s.bind(name, ty);
        }
    }

    fn bind_checked(&mut self, name: &Name, ty: Ty, mutable: bool, using_binding: bool) {
        if self
            .scopes
            .last()
            .is_some_and(|scope| scope.contains(name.as_str()))
        {
            self.sink.emit(
                Diagnostic::error(
                    "bind.shadowing",
                    format!("binding `{}` already exists in this scope", name.text),
                )
                .with_label(Label::primary(self.file_id, name.span, "duplicate binding"))
                .with_action("use a different name or move the declaration to another scope"),
            );
        }
        if let Some(s) = self.scopes.last_mut() {
            s.bind_with_flags(name.text.clone(), ty, mutable, using_binding);
        }
    }

    fn lookup_binding_flags(&self, name: &str) -> Option<(bool, bool)> {
        for scope in self.scopes.iter().rev() {
            if scope.contains(name) {
                return Some((scope.is_mutable(name), scope.is_using_binding(name)));
            }
        }
        None
    }

    fn lookup_local_var_binding(&self, name: &str) -> Option<(Ty, usize, bool)> {
        for (idx, scope) in self.scopes.iter().enumerate().rev() {
            if let Some(ty) = scope.get(name) {
                return Some((ty.clone(), idx, scope.is_mutable(name)));
            }
        }
        None
    }

    fn lookup_local_var(&self, name: &str) -> Option<Ty> {
        self.lookup_local_var_binding(name).map(|(ty, _, _)| ty)
    }

    fn lookup_var(&mut self, name: &str, span: ori_diagnostics::Span) -> Ty {
        if let Some((ty, scope_idx, mutable)) = self.lookup_local_var_binding(name) {
            self.check_closure_var_capture(name, span, scope_idx, mutable, &ty);
            return ty;
        }
        self.emit_undefined_name(name, span);
        Ty::Error
    }

    fn lookup_self(&mut self, span: ori_diagnostics::Span) -> Ty {
        if let Some((ty, scope_idx, mutable)) = self.lookup_local_var_binding("self") {
            self.check_closure_var_capture("self", span, scope_idx, mutable, &ty);
            return ty;
        }
        self.emit_self_outside_method(span);
        Ty::Error
    }

    fn check_closure_var_capture(
        &mut self,
        name: &str,
        span: ori_diagnostics::Span,
        scope_idx: usize,
        mutable: bool,
        ty: &Ty,
    ) {
        let captures_outer = self
            .closure_scope_roots
            .last()
            .is_some_and(|closure_scope_root| scope_idx < *closure_scope_root);
        if mutable && captures_outer {
            self.emit_closure_captures_var(name, span);
        }
        if captures_outer && self.transferable_closure_depth > 0 && !self.is_transferable_ty(ty) {
            self.emit_capture_not_transferable(name, ty, span);
        }
    }

    fn emit_undefined_name(&mut self, name: &str, span: ori_diagnostics::Span) {
        self.sink.emit(
            Diagnostic::error("name.undefined", format!("undefined name `{}`", name))
                .with_label(Label::primary(self.file_id, span, "not in scope"))
                .with_action("declare or import the name before using it"),
        );
    }

    fn emit_self_outside_method(&mut self, span: ori_diagnostics::Span) {
        self.sink.emit(
            Diagnostic::error(
                "bind.self_outside_method",
                "`self` can only be used inside a method",
            )
            .with_label(Label::primary(
                self.file_id,
                span,
                "`self` is not in method scope",
            ))
            .with_action("move this code into a method or pass the value explicitly"),
        );
    }

    fn emit_invalid_range_endpoint(
        &mut self,
        endpoint: &str,
        span: ori_diagnostics::Span,
        ty: &Ty,
    ) {
        self.sink.emit(
            Diagnostic::error(
                "parse.invalid_range",
                format!("range {endpoint} must be `int`, found `{}`", ty.display()),
            )
            .with_label(Label::primary(self.file_id, span, "expected `int` here"))
            .with_action(format!(
                "use an integer expression for the range {endpoint}"
            )),
        );
    }

    fn emit_generic_circular_instantiation(
        &mut self,
        q: &QualifiedName,
        span: ori_diagnostics::Span,
    ) {
        self.sink.emit(
            Diagnostic::error(
                "generic.circular_instantiation",
                format!(
                    "generic function `{}` recursively instantiates itself without a concrete type",
                    q.last().text
                ),
            )
            .with_label(Label::primary(self.file_id, span, "recursive generic call here"))
            .with_action("move recursion into a non-generic helper or call the function with a concrete type"),
        );
    }

    fn emit_closure_captures_var(&mut self, name: &str, span: ori_diagnostics::Span) {
        self.sink.emit(
            Diagnostic::error(
                "mut.closure_captures_var",
                format!("closure cannot capture mutable binding `{}`", name),
            )
            .with_label(Label::primary(
                self.file_id,
                span,
                "mutable binding captured here",
            ))
            .with_action("copy the value into a const binding before creating the closure"),
        );
    }

    fn emit_capture_not_transferable(&mut self, name: &str, ty: &Ty, span: ori_diagnostics::Span) {
        self.sink.emit(
            Diagnostic::error(
                "async.capture_not_transferable",
                format!(
                    "closure passed to `task.spawn` cannot capture `{}` of type `{}`",
                    name,
                    ty.display()
                ),
            )
            .with_label(Label::primary(
                self.file_id,
                span,
                "captured by spawned work here",
            ))
            .with_why("values crossing task boundaries must satisfy `Transferable`")
            .with_action(
                "capture primitive values, strings, bytes, transferable collections, or a struct whose fields are transferable",
            ),
        );
    }

    fn expect_transferable_ty(&mut self, ty: &Ty, span: ori_diagnostics::Span) {
        if ty.is_error() || ty.contains_infer() || self.is_transferable_ty(ty) {
            return;
        }
        self.sink.emit(
            Diagnostic::error(
                "concurrency.not_transferable",
                format!("`{}` cannot cross a task or channel boundary", ty.display()),
            )
            .with_label(Label::primary(self.file_id, span, "value crosses boundary here"))
            .with_why("tasks and channels require values that can move safely between threads")
            .with_action(
                "use a primitive, string, bytes, transferable collection, or a struct whose fields are transferable",
            ),
        );
    }

    fn is_transferable_ty(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Bool
            | Ty::Int
            | Ty::Int8
            | Ty::Int16
            | Ty::Int32
            | Ty::Int64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Float
            | Ty::Float32
            | Ty::Float64
            | Ty::String
            | Ty::Bytes
            | Ty::Void
            | Ty::Never
            | Ty::AtomicInt
            | Ty::TaskJoinError
            | Ty::ChannelSendError
            | Ty::ChannelReceiveError => true,
            Ty::Optional(inner)
            | Ty::List(inner)
            | Ty::Set(inner)
            | Ty::Range(inner)
            | Ty::Future(inner)
            | Ty::TaskJob(inner)
            | Ty::Channel(inner) => self.is_transferable_ty(inner),
            Ty::Map(key, value) | Ty::Result(key, value) => {
                self.is_transferable_ty(key) && self.is_transferable_ty(value)
            }
            Ty::Opaque { args, .. } => args.iter().all(|arg| self.is_transferable_ty(arg)),
            Ty::Tuple(items) => items.iter().all(|item| self.is_transferable_ty(item)),
            Ty::Named(def_id, args) => {
                if let Some(sig) = self.struct_sigs.iter().find(|sig| sig.def_id == *def_id) {
                    return sig.fields.iter().all(|(_, field_ty)| {
                        self.is_transferable_ty(&substitute_ty_params(field_ty, args))
                    });
                }
                if self.enum_sigs.iter().any(|sig| sig.def_id == *def_id) {
                    return true;
                }
                self.user_type_implements_core_trait_id(*def_id, "Transferable")
            }
            Ty::Infer(_) | Ty::Param { .. } | Ty::Error => true,
            Ty::Func { .. } | Ty::Lazy(_) | Ty::Handle(_) | Ty::Any(_) => false,
        }
    }

    fn infer_field_access(&mut self, receiver_root: Option<&Name>, obj_ty: Ty, field: &Name) -> Ty {
        if let Ty::Named(def_id, _) = &obj_ty {
            if let Some(ty) = self.struct_field_ty(*def_id, field.as_str()) {
                return ty;
            }

            // Method fallback
            let def = self.def_map.get(*def_id);
            let method_path = format!("{}.{}", def.path, field.text);
            if let Some(m_def_id) = self.def_map.lookup(&method_path) {
                if let Some(sig) = self.func_sig(m_def_id) {
                    if sig.is_mut {
                        if let Some(root) = receiver_root {
                            self.check_mut_method_receiver_root(root, field);
                        }
                    }
                    let mut params = sig.params.clone();
                    if !params.is_empty() {
                        params.remove(0); // Remove `self`
                    }
                    return Ty::Func {
                        params,
                        ret: Box::new(sig.return_ty.clone()),
                    };
                }
            }

            let trait_methods = self.trait_methods_for_type(*def_id, field.as_str());
            if trait_methods.len() > 1 {
                self.sink.emit(
                    Diagnostic::error(
                        "type.ambiguous_method",
                        format!(
                            "method `{}` is provided by more than one trait for `{}`",
                            field.text,
                            obj_ty.display()
                        ),
                    )
                    .with_label(Label::primary(self.file_id, field.span, "ambiguous method"))
                    .with_action("call the method with trait qualification, for example `Trait.method(value)`"),
                );
                return Ty::Error;
            }

            if let Some(method) = trait_methods.into_iter().next() {
                if method.is_mut {
                    if let Some(root) = receiver_root {
                        self.check_mut_method_receiver_root(root, field);
                    }
                }
                let mut params = method.params;
                if !params.is_empty() {
                    params.remove(0);
                }
                return Ty::Func {
                    params,
                    ret: Box::new(method.return_ty),
                };
            }

            self.sink.emit(
                Diagnostic::error(
                    "type.no_such_field",
                    format!(
                        "type `{}` has no field or method `{}`",
                        obj_ty.display(),
                        field.text
                    ),
                )
                .with_label(Label::primary(
                    self.file_id,
                    field.span,
                    "unknown field/method",
                ))
                .with_action("use a field or method declared on the struct"),
            );
            return Ty::Error;
        }
        if let Ty::Any(trait_def_id) = &obj_ty {
            if let Some(trait_sig) = self.trait_sig(*trait_def_id) {
                if let Some(method) = trait_sig
                    .methods
                    .iter()
                    .find(|m| m.name == field.text)
                    .cloned()
                {
                    if method.is_mut {
                        if let Some(root) = receiver_root {
                            self.check_mut_method_receiver_root(root, field);
                        }
                    }
                    let mut params = method.params.clone();
                    if !params.is_empty() {
                        params.remove(0);
                    }
                    return Ty::Func {
                        params,
                        ret: Box::new(method.return_ty.clone()),
                    };
                }
            }

            self.sink.emit(
                Diagnostic::error(
                    "type.no_such_method",
                    format!(
                        "`{}` is not a method declared by `{}`",
                        field.text,
                        obj_ty.display()
                    ),
                )
                .with_label(Label::primary(
                    self.file_id,
                    field.span,
                    "unknown trait method",
                ))
                .with_action("call only methods declared by the trait in `any[Trait]`"),
            );
            return Ty::Error;
        }
        if let Ty::Param { index, name } = &obj_ty {
            if let Some(method) = self.trait_method_for_type_param(*index, name, field.as_str()) {
                if method.is_mut {
                    if let Some(root) = receiver_root {
                        self.check_mut_method_receiver_root(root, field);
                    }
                }
                let mut params = method.params;
                if !params.is_empty() {
                    params.remove(0);
                }
                return Ty::Func {
                    params,
                    ret: Box::new(method.return_ty),
                };
            }
        }
        if let Ty::Tuple(elems) = &obj_ty {
            if let Ok(idx) = field.text.parse::<usize>() {
                if idx < elems.len() {
                    return elems[idx].clone();
                } else {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.tuple_index_out_of_bounds",
                            format!(
                                "tuple has {} elements, index {} is out of bounds",
                                elems.len(),
                                idx
                            ),
                        )
                        .with_label(Label::primary(
                            self.file_id,
                            field.span,
                            "out of bounds",
                        )),
                    );
                    return Ty::Error;
                }
            } else {
                self.sink.emit(
                    Diagnostic::error(
                        "type.field_on_tuple_not_int",
                        format!("tuple indices must be integers, found `{}`", field.text),
                    )
                    .with_label(Label::primary(
                        self.file_id,
                        field.span,
                        "invalid tuple index",
                    )),
                );
                return Ty::Error;
            }
        }
        if matches!(obj_ty, Ty::String | Ty::Bytes) {
            let namespaces = ["ori.string", "ori.bytes"];
            for ns in namespaces {
                let stdlib_path = format!("{}.{}", ns, field.text);
                if let Some((params, ret)) = stdlib_func_sig(&stdlib_path) {
                    if let Some(first_param) = params.first() {
                        if obj_ty.is_assignable_to(first_param) {
                            let mut params = params;
                            params.remove(0);
                            return Ty::Func {
                                params,
                                ret: Box::new(ret),
                            };
                        }
                    }
                }
            }
        }
        if obj_ty.is_error() || obj_ty.contains_infer() {
            Ty::Infer(0)
        } else {
            self.sink.emit(
                Diagnostic::error(
                    "type.field_on_non_struct",
                    format!(
                        "cannot access field `{}` on `{}`",
                        field.text,
                        obj_ty.display()
                    ),
                )
                .with_label(Label::primary(
                    self.file_id,
                    field.span,
                    "field access here",
                ))
                .with_action("access fields only on struct values"),
            );
            Ty::Error
        }
    }

    fn expand_alias(&mut self, name: &str) -> String {
        let mut prefix_end = name.len();
        loop {
            let prefix = &name[..prefix_end];
            if let Some(full_ns) = self.aliases.get(prefix) {
                // Mark the matched key (short alias, reexport path like `api.util`,
                // or bare whole-module path). Also mark the first segment so
                // `api.util.answer` counts the `import app.facade = api` as used.
                self.used_aliases.insert(SmolStr::new(prefix));
                if let Some(root) = prefix.split('.').next() {
                    self.used_aliases.insert(SmolStr::new(root));
                }
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

    fn resolve_def_id(&mut self, name: &str) -> Option<DefId> {
        let expanded = self.expand_alias(name);
        self.def_map.lookup(&expanded).or_else(|| {
            self.def_map
                .lookup(&format!("{}.{}", self.namespace, expanded))
        })
    }

    fn resolve_enum_variant(&mut self, q: &QualifiedName) -> Option<(DefId, SmolStr)> {
        let enum_path = qualified_prefix(q)?;
        let def_id = self.resolve_def_id(&enum_path)?;
        if self.def_map.get(def_id).kind != DefKind::Enum {
            return None;
        }
        self.check_visibility(def_id, q.span);
        Some((def_id, q.last().text.clone()))
    }

    /// Returns the namespace prefix of a fully-qualified path (everything before the last dot).
    fn def_namespace(path: &str) -> &str {
        path.rfind('.').map(|i| &path[..i]).unwrap_or(path)
    }

    /// Emit a diagnostic if `def_id` refers to a private item in a different namespace.
    fn check_visibility(&mut self, def_id: DefId, span: ori_diagnostics::Span) {
        let def = self.def_map.get(def_id);
        if !def.is_public {
            let def_ns = Self::def_namespace(&def.path);
            if def_ns != self.namespace {
                self.sink.emit(
                    Diagnostic::error(
                        "name.private",
                        format!("`{}` is private to namespace `{}`", def.name, def_ns),
                    )
                    .with_label(Label::primary(self.file_id, span, "used here"))
                    .with_action("mark the definition as `pub` or use a public alternative"),
                );
            }
        }
        if let Some(deprecated) = self
            .deprecated_sigs
            .iter()
            .find(|deprecated| deprecated.def_id == def_id)
        {
            self.sink.emit(
                Diagnostic::warning("attr.deprecated", format!("`{}` is deprecated", def.name))
                    .with_label(Label::primary(
                        self.file_id,
                        span,
                        "deprecated item used here",
                    ))
                    .with_note(deprecated.message.to_string())
                    .with_action("use the recommended replacement when one is available"),
            );
        }
    }

    fn expect_bool(&mut self, ty: &Ty, span: ori_diagnostics::Span) {
        if ty != &Ty::Bool && !ty.is_error() {
            self.sink.emit(
                Diagnostic::error(
                    "type.expected_bool",
                    format!("expected `bool`, found `{}`", ty.display()),
                )
                .with_label(Label::primary(self.file_id, span, "this expression"))
                .with_action("use a boolean expression here"),
            );
        }
    }

    fn expect_assignable(&mut self, from: &Ty, to: &Ty, span: ori_diagnostics::Span) {
        if from.is_never() {
            return;
        }
        if !self.unify(from, to) {
            self.sink.emit(
                Diagnostic::error(
                    "type.type_mismatch",
                    format!(
                        "type mismatch: expected `{}`, found `{}`",
                        to.display(),
                        from.display()
                    ),
                )
                .with_label(Label::primary(self.file_id, span, "this expression"))
                .with_action(format!(
                    "change the expression to produce `{}`",
                    to.display()
                )),
            );
        }
    }

    fn warn_unused_result(&mut self, ty: &Ty, span: ori_diagnostics::Span) {
        if matches!(ty, Ty::Result(_, _)) {
            self.sink.emit(
                Diagnostic::warning(
                    "type.unused_result",
                    format!("result value of type `{}` is discarded", ty.display()),
                )
                .with_label(Label::primary(self.file_id, span, "result discarded here"))
                .with_action("handle the result, propagate it with `?`, or bind it explicitly"),
            );
        }
    }

    // ── Unification helpers ─────────────────────────────────────────────
    /// Attempt to make `a` assignable to `b` by solving inference variables.
    /// Returns `true` if unification succeeds.
    fn check_collection_runtime_limits(&mut self, ty: &Ty, span: ori_diagnostics::Span) {
        match ty {
            Ty::Map(key, value) => {
                if !self.is_current_map_key_supported(key) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.collection_hash_unsupported",
                            format!(
                                "`map` keys currently require `int`, `string`, or a type implementing `Hashable` and `Equatable`, found `{}`",
                                key.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, span, "map type here"))
                        .with_why(
                            "the current map runtime hashes built-in keys directly and accepts user-defined keys behind the `Hashable`/`Equatable` trait gate",
                        )
                        .with_action("use `int`, `string`, or implement both `ori.core.Hashable` and `ori.core.Equatable` for the key type"),
                    );
                }
                self.check_collection_runtime_limits(value, span);
            }
            Ty::Set(elem) => {
                if !self.is_current_set_element_supported(elem) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.collection_hash_unsupported",
                            format!(
                                "`set` elements currently require `int`, `string`, or a type implementing `Hashable` and `Equatable`, found `{}`",
                                elem.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, span, "set type here"))
                        .with_why(
                            "the current set runtime hashes built-in elements directly and accepts user-defined elements behind the `Hashable`/`Equatable` trait gate",
                        )
                        .with_action("use `set[int]`, `set[string]`, or implement both `ori.core.Hashable` and `ori.core.Equatable` for the element type"),
                    );
                }
                self.check_collection_runtime_limits(elem, span);
            }
            Ty::Opaque {
                kind: OpaqueTy::HashTable,
                args,
            } if args.len() == 2 => {
                let key = &args[0];
                let value = &args[1];
                if !self.is_current_map_key_supported(key) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.collection_hash_unsupported",
                            format!(
                                "`hash_table` keys currently require `int`, `string`, or a type implementing `Hashable` and `Equatable`, found `{}`",
                                key.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, span, "hash_table type here"))
                        .with_why(
                            "the current hash_table runtime reuses the map hashing engine and follows the same key support",
                        )
                        .with_action("use `int`, `string`, or implement both `ori.core.Hashable` and `ori.core.Equatable` for the key type"),
                    );
                }
                self.check_collection_runtime_limits(key, span);
                self.check_collection_runtime_limits(value, span);
            }
            Ty::Opaque {
                kind: OpaqueTy::Graph,
                args,
            } if args.len() == 1 => {
                let node = &args[0];
                if !self.is_current_map_key_supported(node) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.collection_hash_unsupported",
                            format!(
                                "`graph` nodes currently require `int`, `string`, or a type implementing `Hashable` and `Equatable`, found `{}`",
                                node.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, span, "graph type here"))
                        .with_why(
                            "the current graph runtime stores nodes with the same key support used by map/hash_table",
                        )
                        .with_action("use `int`, `string`, or implement both `ori.core.Hashable` and `ori.core.Equatable` for the node type"),
                    );
                }
                self.check_collection_runtime_limits(node, span);
            }
            Ty::Opaque {
                kind: OpaqueTy::Heap,
                args,
            } if args.len() == 1 => {
                let elem = &args[0];
                if !self.is_current_heap_element_supported(elem) {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.collection_comparable_unsupported",
                            format!(
                                "`heap` elements currently require `int`, `string`, or a type implementing `Comparable`, found `{}`",
                                elem.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, span, "heap type here"))
                        .with_why(
                            "the current heap runtime is a min-heap and needs a stable ordering for every element",
                        )
                        .with_action("use `int`, `string`, or implement `ori.core.Comparable` for the element type"),
                    );
                }
                self.check_collection_runtime_limits(elem, span);
            }
            Ty::Optional(inner)
            | Ty::List(inner)
            | Ty::Range(inner)
            | Ty::Lazy(inner)
            | Ty::Future(inner)
            | Ty::TaskJob(inner)
            | Ty::Channel(inner) => {
                self.check_collection_runtime_limits(inner, span);
            }
            Ty::Result(ok, err) => {
                self.check_collection_runtime_limits(ok, span);
                self.check_collection_runtime_limits(err, span);
            }
            Ty::Opaque { args, .. } => {
                for arg in args {
                    self.check_collection_runtime_limits(arg, span);
                }
            }
            Ty::Tuple(items) => {
                for item in items {
                    self.check_collection_runtime_limits(item, span);
                }
            }
            Ty::Func { params, ret } => {
                for param in params {
                    self.check_collection_runtime_limits(param, span);
                }
                self.check_collection_runtime_limits(ret, span);
            }
            Ty::Named(_, args) => {
                for arg in args {
                    self.check_collection_runtime_limits(arg, span);
                }
            }
            _ => {}
        }
    }

    fn is_current_map_key_supported(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::String)
            || is_current_integer_hash_supported(ty)
            || self.user_type_has_hash_and_eq(ty)
            || self.param_has_hash_and_eq(ty)
    }

    fn is_current_set_element_supported(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::String)
            || is_current_integer_hash_supported(ty)
            || self.user_type_has_hash_and_eq(ty)
            || self.param_has_hash_and_eq(ty)
    }

    fn is_current_heap_element_supported(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::String)
            || is_current_integer_hash_supported(ty)
            || self.user_type_implements_core_trait(ty, "Comparable")
            || self.param_implements_core_trait_by_ty(ty, "Comparable")
    }

    fn user_type_has_hash_and_eq(&self, ty: &Ty) -> bool {
        let Ty::Named(type_def_id, _) = ty else {
            return false;
        };
        self.user_type_implements_core_trait_id(*type_def_id, "Hashable")
            && self.user_type_implements_core_trait_id(*type_def_id, "Equatable")
    }

    fn param_has_hash_and_eq(&self, ty: &Ty) -> bool {
        let Ty::Param { index, .. } = ty else {
            return false;
        };
        self.param_implements_core_trait(*index, "Hashable")
            && self.param_implements_core_trait(*index, "Equatable")
    }

    fn param_implements_core_trait_by_ty(&self, ty: &Ty, trait_name: &str) -> bool {
        let Ty::Param { index, .. } = ty else {
            return false;
        };
        self.param_implements_core_trait(*index, trait_name)
    }

    fn user_type_has_equatable(&self, ty: &Ty) -> bool {
        let Ty::Named(type_def_id, _) = ty else {
            return false;
        };
        self.user_type_implements_core_trait_id(*type_def_id, "Equatable")
    }

    fn param_implements_core_trait(&self, index: u32, trait_name: &str) -> bool {
        let Some(trait_def_id) = self.def_map.lookup(&format!("ori.core.{trait_name}")) else {
            return false;
        };
        self.current_where_constraints
            .iter()
            .any(|c| c.param_index == index && c.trait_def_id == trait_def_id && !c.negative)
    }

    fn user_type_implements_core_trait(&self, ty: &Ty, trait_name: &str) -> bool {
        let Ty::Named(type_def_id, _) = ty else {
            return false;
        };
        self.user_type_implements_core_trait_id(*type_def_id, trait_name)
    }

    fn user_type_implements_core_trait_id(&self, type_def_id: DefId, trait_name: &str) -> bool {
        let Some(trait_def_id) = self.def_map.lookup(&format!("ori.core.{trait_name}")) else {
            return false;
        };
        self.named_type_implements_trait(type_def_id, trait_def_id)
    }

    fn operator_trait_method_sig(
        &self,
        ty: &Ty,
        trait_name: &str,
        method_name: &str,
    ) -> Option<FuncSig> {
        let Ty::Named(type_def_id, _) = ty else {
            return None;
        };
        let trait_def_id = self.def_map.lookup(&format!("ori.core.{trait_name}"))?;
        let impl_sig = self
            .impl_sigs
            .iter()
            .find(|sig| sig.type_def_id == *type_def_id && sig.trait_def_id == trait_def_id)?;
        let method = impl_sig
            .methods
            .iter()
            .find(|method| method.name == method_name)?;
        self.func_sig(method.func_def_id)
    }

    fn unify(&mut self, a: &Ty, b: &Ty) -> bool {
        use Ty::*;
        if a == b {
            return true;
        }
        if matches!(
            (a, b),
            (Int, Int64) | (Int64, Int) | (Float, Float64) | (Float64, Float)
        ) {
            return true;
        }
        if matches!((a, b), (Int, ty) | (ty, Int) if ty.is_node_id()) {
            return true;
        }
        match (a, b) {
            (Infer(id), _) => return self.unify_infer(*id, b),
            (_, Infer(id)) => return self.unify_infer(*id, a),
            (Optional(x), Optional(y)) => self.unify(x, y),
            (Result(ok1, err1), Result(ok2, err2)) => {
                self.unify(ok1, ok2) && self.unify(err1, err2)
            }
            (List(x), List(y))
            | (Set(x), Set(y))
            | (Range(x), Range(y))
            | (Lazy(x), Lazy(y))
            | (Future(x), Future(y))
            | (TaskJob(x), TaskJob(y))
            | (Channel(x), Channel(y)) => self.unify(x, y),
            (Map(ka, va), Map(kb, vb)) => self.unify(ka, kb) && self.unify(va, vb),
            (
                Opaque {
                    kind: kind_a,
                    args: args_a,
                },
                Opaque {
                    kind: kind_b,
                    args: args_b,
                },
            ) if kind_a == kind_b && args_a.len() == args_b.len() => {
                args_a.iter().zip(args_b).all(|(x, y)| self.unify(x, y))
            }
            (Tuple(xs), Tuple(ys)) if xs.len() == ys.len() => {
                xs.iter().zip(ys).all(|(x, y)| self.unify(x, y))
            }
            (
                Func {
                    params: pa,
                    ret: ra,
                },
                Func {
                    params: pb,
                    ret: rb,
                },
            ) if pa.len() == pb.len() => {
                pa.iter().zip(pb).all(|(x, y)| self.unify(x, y)) && self.unify(ra, rb)
            }
            (Named(id1, args1), Named(id2, args2)) if id1 == id2 && args1.len() == args2.len() => {
                args1.iter().zip(args2).all(|(x, y)| self.unify(x, y))
            }
            (Named(type_id, _), Any(trait_id)) => {
                self.named_type_implements_trait(*type_id, *trait_id)
            }
            (Any(id1), Any(id2)) => id1 == id2,
            _ => false,
        }
    }

    fn unify_infer(&mut self, id: u32, ty: &Ty) -> bool {
        if id == 0 {
            return !Self::contains_infer_id(ty, id) || matches!(ty, Ty::Infer(0));
        }
        if let Ty::Infer(other) = ty {
            if *other == id {
                return true;
            }
        }
        if let Some(bound) = self.infer.get(&id).cloned() {
            return self.unify(&bound, ty);
        }
        if Self::contains_infer_id(ty, id) {
            return false;
        } // occurs check
        self.infer.insert(id, ty.clone());
        true
    }

    /// Detect whether `ty` contains the given inference variable id (occurs check).
    fn contains_infer_id(ty: &Ty, id: u32) -> bool {
        match ty {
            Ty::Infer(i) => *i == id,
            Ty::Optional(t)
            | Ty::List(t)
            | Ty::Set(t)
            | Ty::Range(t)
            | Ty::Lazy(t)
            | Ty::Future(t)
            | Ty::TaskJob(t)
            | Ty::Channel(t) => Self::contains_infer_id(t, id),
            Ty::Result(ok, err) | Ty::Map(ok, err) => {
                Self::contains_infer_id(ok, id) || Self::contains_infer_id(err, id)
            }
            Ty::Opaque { args, .. } => args.iter().any(|arg| Self::contains_infer_id(arg, id)),
            Ty::Tuple(ts) => ts.iter().any(|t| Self::contains_infer_id(t, id)),
            Ty::Func { params, ret } => {
                params.iter().any(|p| Self::contains_infer_id(p, id))
                    || Self::contains_infer_id(ret, id)
            }
            Ty::Named(_, args) => args.iter().any(|a| Self::contains_infer_id(a, id)),
            _ => false,
        }
    }

    /// Check that a pattern is consistent with the scrutinee type.

    /// Check that a pattern is consistent with the scrutinee type.
    /// Also binds variables introduced by the pattern into the current scope.
    fn resolve_infer(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Infer(id) => {
                if let Some(bound) = self.infer.get(id) {
                    self.resolve_infer(bound)
                } else {
                    ty.clone()
                }
            }
            Ty::Optional(t) => Ty::Optional(Box::new(self.resolve_infer(t))),
            Ty::Result(ok, err) => Ty::Result(
                Box::new(self.resolve_infer(ok)),
                Box::new(self.resolve_infer(err)),
            ),
            Ty::List(t) => Ty::List(Box::new(self.resolve_infer(t))),
            Ty::Map(k, v) => Ty::Map(
                Box::new(self.resolve_infer(k)),
                Box::new(self.resolve_infer(v)),
            ),
            Ty::Set(t) => Ty::Set(Box::new(self.resolve_infer(t))),
            Ty::Range(t) => Ty::Range(Box::new(self.resolve_infer(t))),
            Ty::Lazy(t) => Ty::Lazy(Box::new(self.resolve_infer(t))),
            Ty::Future(t) => Ty::Future(Box::new(self.resolve_infer(t))),
            Ty::TaskJob(t) => Ty::TaskJob(Box::new(self.resolve_infer(t))),
            Ty::Channel(t) => Ty::Channel(Box::new(self.resolve_infer(t))),
            Ty::Opaque { kind, args } => Ty::Opaque {
                kind: *kind,
                args: args.iter().map(|arg| self.resolve_infer(arg)).collect(),
            },
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| self.resolve_infer(t)).collect()),
            Ty::Func { params, ret } => Ty::Func {
                params: params.iter().map(|p| self.resolve_infer(p)).collect(),
                ret: Box::new(self.resolve_infer(ret)),
            },
            Ty::Named(def_id, args) => Ty::Named(
                *def_id,
                args.iter().map(|arg| self.resolve_infer(arg)).collect(),
            ),
            _ => ty.clone(),
        }
    }

    fn check_pattern_type(&mut self, pat: &Pattern, scr_ty: &Ty) {
        let resolved = self.resolve_infer(scr_ty);
        let scr_ty = &resolved;
        match pat {
            Pattern::Wildcard(_) => {}
            Pattern::Binding(n) => {
                if let Some(variant) = self.enum_variant_sig_for_name(scr_ty, n) {
                    self.check_enum_variant_arity(scr_ty, n, 0, variant.fields.len());
                } else {
                    // Bind the variable with the scrutinee's type
                    self.bind(n.text.clone(), scr_ty.clone());
                }
            }
            Pattern::Literal(expr) => {
                let lit_ty = self.infer_expr(expr);
                if !lit_ty.is_error()
                    && !scr_ty.is_error()
                    && !lit_ty.contains_infer()
                    && !scr_ty.contains_infer()
                    && !lit_ty.is_assignable_to(scr_ty)
                {
                    self.sink.emit(
                        Diagnostic::error(
                            "type.pattern_mismatch",
                            format!(
                                "pattern type `{}` does not match scrutinee `{}`",
                                lit_ty.display(),
                                scr_ty.display()
                            ),
                        )
                        .with_label(Label::primary(self.file_id, expr.span(), "pattern here"))
                        .with_action(format!("expected pattern of type `{}`", scr_ty.display())),
                    );
                }
            }
            Pattern::Some(inner, _) => {
                let inner_ty = match scr_ty {
                    Ty::Optional(t) => *t.clone(),
                    _ => Ty::Infer(0),
                };
                self.check_pattern_type(inner, &inner_ty);
            }
            Pattern::None(_) => {}
            Pattern::Ok(inner, _) => {
                let ok_ty = match scr_ty {
                    Ty::Result(ok, _) => *ok.clone(),
                    _ => Ty::Infer(0),
                };
                self.check_pattern_type(inner, &ok_ty);
            }
            Pattern::Err(inner, _) => {
                let err_ty = match scr_ty {
                    Ty::Result(_, err) => *err.clone(),
                    _ => Ty::Infer(0),
                };
                self.check_pattern_type(inner, &err_ty);
            }
            Pattern::VariantUnit { name, .. } => {
                if let Some(variant) = self.check_enum_variant_pattern(scr_ty, name) {
                    self.check_enum_variant_arity(scr_ty, name, 0, variant.fields.len());
                }
            }
            Pattern::VariantNamed { name, fields, .. } => {
                let Some(variant) = self.check_enum_variant_pattern(scr_ty, name) else {
                    return;
                };
                self.check_enum_variant_arity(scr_ty, name, fields.len(), variant.fields.len());

                let mut seen = HashSet::new();
                for f in fields {
                    if !seen.insert(f.name.text.clone()) {
                        self.emit_enum_pattern_payload_error(
                            f.span,
                            format!(
                                "variant `{}` pattern repeats field `{}`",
                                name.text, f.name.text
                            ),
                            "remove the duplicate field pattern",
                        );
                        continue;
                    }

                    let field_ty = variant
                        .fields
                        .iter()
                        .find(|(fname, _)| *fname == f.name.text)
                        .map(|(_, ty)| ty.clone());
                    let Some(field_ty) = field_ty else {
                        self.emit_enum_pattern_payload_error(
                            f.span,
                            format!("variant `{}` has no field `{}`", name.text, f.name.text),
                            "use a field declared by the enum variant",
                        );
                        self.check_pattern_type(&f.pattern, &Ty::Infer(0));
                        continue;
                    };
                    self.check_pattern_type(&f.pattern, &field_ty);
                }

                for (field_name, _) in &variant.fields {
                    if !seen.contains(field_name) {
                        self.emit_enum_pattern_payload_error(
                            name.span,
                            format!(
                                "variant `{}` pattern is missing field `{}`",
                                name.text, field_name
                            ),
                            "include every payload field or use `case else`",
                        );
                    }
                }
            }
            Pattern::Tuple(pats, _) => {
                let elem_tys = match scr_ty {
                    Ty::Tuple(ts) => ts.clone(),
                    _ => vec![Ty::Infer(0); pats.len()],
                };
                for (p, t) in pats.iter().zip(elem_tys.iter()) {
                    self.check_pattern_type(p, t);
                }
            }
        }
    }

    fn check_enum_variant_pattern(
        &mut self,
        scr_ty: &Ty,
        name: &Name,
    ) -> Option<crate::resolve::EnumVariantSig> {
        let resolved = self.resolve_infer(scr_ty);
        let scr_ty = &resolved;
        if let Ty::Named(def_id, _) = scr_ty {
            if self.def_map.get(*def_id).kind != DefKind::Enum {
                return None;
            }
            let Some(enum_sig) = self.enum_sig(*def_id) else {
                return None;
            };
            let variant = enum_sig
                .variants
                .iter()
                .find(|variant| variant.name == name.text)
                .cloned();
            if variant.is_none() {
                self.sink.emit(
                    Diagnostic::error(
                        "type.unknown_enum_variant",
                        format!("enum has no variant `{}`", name.text),
                    )
                    .with_label(Label::primary(self.file_id, name.span, "unknown variant"))
                    .with_action("use a variant declared on this enum"),
                );
            }
            variant
        } else if !scr_ty.is_error() && !scr_ty.contains_infer() {
            self.sink.emit(
                Diagnostic::error(
                    "type.pattern_mismatch",
                    format!("enum variant pattern cannot match `{}`", scr_ty.display()),
                )
                .with_label(Label::primary(
                    self.file_id,
                    name.span,
                    "variant pattern here",
                ))
                .with_action("use enum variant patterns only when matching enum values"),
            );
            None
        } else {
            None
        }
    }

    fn enum_variant_sig_for_name(
        &self,
        scr_ty: &Ty,
        name: &Name,
    ) -> Option<crate::resolve::EnumVariantSig> {
        let resolved = self.resolve_infer(scr_ty);
        let scr_ty = &resolved;
        let Ty::Named(def_id, _) = scr_ty else {
            return None;
        };
        if self.def_map.get(*def_id).kind != DefKind::Enum {
            return None;
        }
        self.enum_sig(*def_id)?
            .variants
            .iter()
            .find(|variant| variant.name == name.text)
            .cloned()
    }

    fn check_enum_variant_arity(
        &mut self,
        scr_ty: &Ty,
        name: &Name,
        actual: usize,
        expected: usize,
    ) {
        if actual == expected {
            return;
        }
        let resolved = self.resolve_infer(scr_ty);
        let scr_ty = &resolved;
        if scr_ty.is_error() || scr_ty.contains_infer() {
            return;
        }

        let expected_text = if expected == 0 {
            "no payload fields".to_string()
        } else if expected == 1 {
            "1 payload field".to_string()
        } else {
            format!("{} payload fields", expected)
        };
        self.emit_enum_pattern_payload_error(
            name.span,
            format!(
                "variant `{}` pattern has {} field(s), but the variant expects {}",
                name.text, actual, expected_text
            ),
            "match the variant payload shape declared in the enum",
        );
    }

    fn emit_enum_pattern_payload_error(
        &mut self,
        span: ori_diagnostics::Span,
        message: impl Into<String>,
        action: impl Into<String>,
    ) {
        self.sink.emit(
            Diagnostic::error("type.pattern_mismatch", message.into())
                .with_label(Label::primary(self.file_id, span, "variant pattern here"))
                .with_action(action.into()),
        );
    }

    /// Infer the type of an lvalue target (variable, field, or index).
    fn infer_lvalue_ty(&mut self, lv: &LValue) -> Ty {
        match lv {
            LValue::Ident(n) => self.lookup_local_var(&n.text).unwrap_or(Ty::Infer(0)),
            LValue::Field { base, field, .. } => {
                let base_ty = self.infer_lvalue_ty(base);
                if let Ty::Named(def_id, _) = &base_ty {
                    self.struct_field_ty(*def_id, field.as_str())
                        .unwrap_or(Ty::Infer(0))
                } else {
                    Ty::Infer(0)
                }
            }
            LValue::Index { base, index, .. } => {
                let base_ty = self.infer_lvalue_ty(base);
                self.infer_expr(index);
                match &base_ty {
                    Ty::List(elem) => *elem.clone(),
                    Ty::Map(_, val) => *val.clone(),
                    _ => Ty::Infer(0),
                }
            }
        }
    }

    fn check_lvalue_mutable(&mut self, lv: &LValue) {
        let Some(root) = lvalue_root_name(lv) else {
            return;
        };
        let Some((mutable, using_binding)) = self.lookup_binding_flags(&root.text) else {
            return;
        };
        if using_binding {
            self.sink.emit(
                Diagnostic::error(
                    "mut.using_binding_mutated",
                    format!("using binding `{}` cannot be reassigned", root.text),
                )
                .with_label(Label::primary(
                    self.file_id,
                    root.span,
                    "using binding is immutable",
                ))
                .with_action("bind a separate `var` if mutable state is needed"),
            );
        } else if !mutable {
            let (code, action) = if matches!(lv, LValue::Ident(_)) {
                (
                    "bind.const_reassignment",
                    "declare it with `var` if reassignment is intended",
                )
            } else if root.text == "self" {
                (
                    "mut.field_mutation_in_func",
                    "declare this method as `mut func` before mutating `self`",
                )
            } else {
                (
                    "mut.const_mutation",
                    "declare it with `var` if reassignment is intended",
                )
            };
            self.sink.emit(
                Diagnostic::error(code, format!("`{}` is not mutable", root.text))
                    .with_label(Label::primary(self.file_id, root.span, "immutable binding"))
                    .with_action(action),
            );
        }
    }

    fn check_mut_method_receiver_root(&mut self, root: &Name, method: &Name) {
        let Some((mutable, using_binding)) = self.lookup_binding_flags(&root.text) else {
            return;
        };
        if mutable && !using_binding {
            return;
        }
        let code = if using_binding {
            "mut.using_binding_mutated"
        } else {
            "mut.const_method_call"
        };
        self.sink.emit(
            Diagnostic::error(
                code,
                format!(
                    "cannot call mut method `{}` on immutable binding `{}`",
                    method.text, root.text
                ),
            )
            .with_label(Label::primary(self.file_id, method.span, "mut method call"))
            .with_action("store the receiver in a `var` before calling a mut method"),
        );
    }

    fn check_disposable_using(&mut self, ty: &Ty, span: ori_diagnostics::Span) {
        if ty.is_error() || ty.contains_infer() {
            return;
        }
        let mut disposable_trait_ids = Vec::new();
        if let Some(core_disposable) = self.def_map.lookup("ori.core.Disposable") {
            disposable_trait_ids.push(core_disposable);
        }
        if let Some(local_disposable) = self.resolve_def_id("Disposable") {
            if !disposable_trait_ids.contains(&local_disposable) {
                disposable_trait_ids.push(local_disposable);
            }
        }
        if disposable_trait_ids.is_empty() {
            self.emit_not_disposable(ty, span);
            return;
        }
        match ty {
            Ty::Opaque {
                kind: OpaqueTy::File | OpaqueTy::Input | OpaqueTy::Output | OpaqueTy::Connection,
                ..
            } => {}
            Ty::Named(type_def_id, _)
                if disposable_trait_ids
                    .iter()
                    .any(|trait_id| self.named_type_implements_trait(*type_def_id, *trait_id)) => {}
            _ => self.emit_not_disposable(ty, span),
        }
    }

    fn emit_not_disposable(&mut self, ty: &Ty, span: ori_diagnostics::Span) {
        self.sink.emit(
            Diagnostic::error(
                "using.not_disposable",
                format!("type `{}` cannot be used with `using`", ty.display()),
            )
            .with_label(Label::primary(self.file_id, span, "not disposable"))
            .with_action("implement `Disposable` for this type before using it with `using`"),
        );
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn lvalue_root_name(lv: &LValue) -> Option<&Name> {
    match lv {
        LValue::Ident(name) => Some(name),
        LValue::Field { base, .. } | LValue::Index { base, .. } => lvalue_root_name(base),
    }
}

fn expr_root_name(expr: &Expr) -> Option<&Name> {
    match expr {
        Expr::Ident(name) => Some(name),
        Expr::QualifiedIdent(name) if name.parts.len() == 1 => name.parts.first(),
        Expr::Field { object, .. } => expr_root_name(object),
        _ => None,
    }
}

/// Adapt expression-form arms to the statement-form `MatchCase` shape, so the
/// duplicate / unreachable / exhaustiveness checkers work on both forms
/// unchanged. Only pattern, guard and span are inspected by those checks, so
/// the body is left empty rather than synthesized.
fn match_expr_arms_as_cases(arms: &[ori_ast::expr::MatchExprArm]) -> Vec<ori_ast::stmt::MatchCase> {
    arms.iter()
        .map(|arm| match &arm.pattern {
            Some(pattern) => ori_ast::stmt::MatchCase::Pattern {
                pattern: pattern.clone(),
                guard: arm.guard.clone(),
                body: Vec::new(),
                span: arm.span,
            },
            None => ori_ast::stmt::MatchCase::Else {
                body: Vec::new(),
                span: arm.span,
            },
        })
        .collect()
}

fn expr_needs_expected_context(expr: &Expr) -> bool {
    match expr {
        Expr::AnonStructLit { .. }
        | Expr::EnumVariantUnit { ty: None, .. }
        | Expr::EnumVariantNamed { ty: None, .. } => true,
        Expr::List { elements, .. } if elements.is_empty() => true,
        Expr::Map { entries, .. } if entries.is_empty() => true,
        Expr::None(_) => true,
        Expr::IfExpr {
            then_expr,
            else_expr,
            ..
        } => expr_needs_expected_context(then_expr) || expr_needs_expected_context(else_expr),
        _ => false,
    }
}

/// Expressions that must never be inferred for unannotated local bindings (`0.3.1`).
fn expr_blocks_local_inference(expr: &Expr) -> bool {
    matches!(expr, Expr::Try { .. })
}

/// Whether a fully inferred type is concrete enough to omit a local annotation.
fn ty_is_locally_inferable(ty: &Ty) -> bool {
    match ty {
        Ty::Error | Ty::Infer(_) | Ty::Never | Ty::Void => false,
        Ty::Optional(inner)
        | Ty::List(inner)
        | Ty::Set(inner)
        | Ty::Range(inner)
        | Ty::Lazy(inner)
        | Ty::Handle(inner) => ty_is_locally_inferable(inner),
        Ty::Result(ok, err) | Ty::Map(ok, err) => {
            ty_is_locally_inferable(ok) && ty_is_locally_inferable(err)
        }
        Ty::Tuple(parts) => parts.iter().all(ty_is_locally_inferable),
        Ty::Func { params, ret } => {
            params.iter().all(ty_is_locally_inferable) && ty_is_locally_inferable(ret)
        }
        Ty::Named(_, args) | Ty::Opaque { args, .. } => args.iter().all(ty_is_locally_inferable),
        Ty::Any(_) => false,
        // Primitives and fully-resolved named/opaque shapes without open args.
        _ => true,
    }
}

fn block_definitely_returns(block: &Block) -> bool {
    stmts_definitely_return(&block.stmts)
}

fn stmts_definitely_return(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_definitely_returns)
}

fn stmt_definitely_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) => true,
        Stmt::Expr(expr) => is_never_form_call_expr(expr),
        Stmt::If(if_stmt) => {
            if_stmt
                .else_block
                .as_ref()
                .is_some_and(block_definitely_returns)
                && block_definitely_returns(&if_stmt.then_block)
                && if_stmt
                    .else_ifs
                    .iter()
                    .all(|(_, block)| block_definitely_returns(block))
        }
        Stmt::IfSome(if_some) => {
            if_some
                .else_block
                .as_ref()
                .is_some_and(block_definitely_returns)
                && block_definitely_returns(&if_some.then_block)
        }
        Stmt::Match(match_stmt) => {
            let has_else = match_stmt
                .cases
                .iter()
                .any(|case| matches!(case, ori_ast::stmt::MatchCase::Else { .. }));
            has_else
                && match_stmt.cases.iter().all(|case| match case {
                    ori_ast::stmt::MatchCase::Pattern { body, .. }
                    | ori_ast::stmt::MatchCase::Else { body, .. } => stmts_definitely_return(body),
                })
        }
        _ => false,
    }
}

fn is_never_form_call_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { callee, .. }
            if matches!(
                callee.as_ref(),
                Expr::QualifiedIdent(q) if is_never_form_name(q.last().as_str())
            )
    )
}

fn is_never_form_name(name: &str) -> bool {
    matches!(name, "panic" | "todo" | "unreachable")
}

fn qualified_prefix(q: &QualifiedName) -> Option<String> {
    if q.parts.len() < 2 {
        return None;
    }
    Some(
        q.parts[..q.parts.len() - 1]
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>()
            .join("."),
    )
}

fn is_builtin_type_name(q: &QualifiedName) -> bool {
    q.is_single()
        && matches!(
            q.last().text.as_str(),
            "bool"
                | "int"
                | "int8"
                | "int16"
                | "int32"
                | "int64"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "float"
                | "float32"
                | "float64"
                | "string"
                | "bytes"
                | "void"
        )
}

fn is_reserved_type_alias_name(name: &str) -> bool {
    matches!(
        name,
        "any"
            | "bool"
            | "bytes"
            | "float"
            | "float32"
            | "float64"
            | "func"
            | "future"
            | "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "lazy"
            | "list"
            | "map"
            | "optional"
            | "range"
            | "result"
            | "set"
            | "string"
            | "tuple"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "void"
    )
}

fn item_target_name(item: &Item) -> &'static str {
    match item {
        Item::Func(_) => "func",
        Item::Struct(_) => "struct",
        Item::Enum(_) => "enum",
        Item::Trait(_) => "trait",
        Item::Apply(_) => "apply",
        Item::Alias(_) => "alias",
        Item::Const(_) => "const",
        Item::Var(_) => "var",
        Item::Extern(_) => "extern",
    }
}

fn is_known_attr(name: &str) -> bool {
    matches!(
        name,
        "test" | "deprecated" | "inline" | "no_inline" | "cfg" | "repr" | "c_export"
    )
}

fn attr_applies_to(name: &str, target: &str) -> bool {
    match name {
        "test" | "inline" | "no_inline" | "c_export" => target == "func",
        "deprecated" | "cfg" => true,
        "repr" => target == "struct",
        _ => false,
    }
}

fn attr_args_valid(name: &str, attr: &Attr) -> bool {
    match name {
        "test" | "inline" | "no_inline" => attr.args.is_empty(),
        "c_export" => matches!(attr.args.as_slice(), [] | [AttrArg::String(_, _)]),
        "deprecated" => matches!(attr.args.as_slice(), [AttrArg::String(_, _)]),
        "cfg" => matches!(
            attr.args.as_slice(),
            [AttrArg::String(_, _)] | [AttrArg::Named { .. }]
        ),
        _ => true,
    }
}

fn attr_target_action(name: &str) -> &'static str {
    match name {
        "test" => "move `@test` to a function declaration",
        "inline" | "no_inline" => "use this attribute only on function declarations",
        "c_export" => "move `@c_export` to a free `public` function",
        _ => "move the attribute to a declaration that supports it",
    }
}

fn attr_arg_action(name: &str) -> &'static str {
    match name {
        "test" | "inline" | "no_inline" => "remove the attribute arguments",
        "c_export" => "use `@c_export` or `@c_export(\"symbol_name\")`",
        "deprecated" => "use `@deprecated(\"message\")` with exactly one string message",
        "cfg" => "use `@cfg(\"condition\")` or `@cfg(key: value)`",
        _ => "use the documented argument form for this attribute",
    }
}

/// Phase-1 `@c_export` surface: scalar types only (`int`/`float`/`bool`/`void`).
fn is_c_export_ffi_ty(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Int
            | Ty::Int8
            | Ty::Int16
            | Ty::Int32
            | Ty::Int64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Float
            | Ty::Float32
            | Ty::Float64
            | Ty::Bool
            | Ty::Void
    )
}

fn stdlib_const_ty(path: &str) -> Option<Ty> {
    match path {
        "ori.math.pi" | "ori.math.e" | "ori.math.infinity" | "ori.math.nan" => Some(Ty::Float),
        _ => None,
    }
}

fn stdlib_named_ty_exists(path: &str) -> bool {
    matches!(
        path,
        "ori.Error"
            | "ori.fs.File"
            | "ori.json.Value"
            | "ori.task.Job"
            | "ori.task.JoinError"
            | "ori.task.CancelToken"
            | "ori.channel.Channel"
            | "ori.channel.SendError"
            | "ori.channel.ReceiveError"
            | "ori.atomic.AtomicInt"
            | "ori.deque.Deque"
            | "ori.queue.Queue"
            | "ori.stack.Stack"
            | "ori.linked_list.LinkedList"
            | "ori.doubly_linked_list.DoublyLinkedList"
            | "ori.tree.Tree"
            | "ori.tree.NodeId"
            | "ori.hash_table.HashTable"
            | "ori.graph.Graph"
            | "ori.heap.Heap"
            | "ori.net.Connection"
            | "ori.net.Listener"
            | "ori.net.UdpSocket"
            | "ori.io.Input"
            | "ori.io.Output"
    )
}

fn stdlib_func_sig(path: &str) -> Option<(Vec<Ty>, Ty)> {
    crate::stdlib::stdlib_func_sig(path)
}

fn import_visible_bindings(import: &ImportDecl) -> Vec<(SmolStr, Span)> {
    if !import.selected.is_empty() {
        return import
            .selected
            .iter()
            .map(|item| {
                let alias = selected_import_alias(item);
                let span = item
                    .alias
                    .as_ref()
                    .map(|a| a.span)
                    .unwrap_or(item.name.span);
                (alias, span)
            })
            .collect();
    }

    if let Some(alias) = &import.alias {
        // Explicit `import path = alias`.
        return vec![(alias.text.clone(), alias.span)];
    }

    // Bare whole-module import: only the full path is usable; track that key
    // for unused-import warnings (matches identity entry in `import_aliases`).
    vec![(SmolStr::new(import.path.to_string()), import.span)]
}

fn selected_import_alias(item: &ImportItem) -> SmolStr {
    item.alias
        .as_ref()
        .map(|a| a.text.clone())
        .unwrap_or_else(|| item.name.text.clone())
}

fn selected_import_target(import: &ImportDecl, item: &ImportItem) -> SmolStr {
    SmolStr::new(format!("{}.{}", import.path, item.name.text))
}

fn restore_alias(aliases: &mut HashMap<SmolStr, SmolStr>, name: &str, previous: Option<SmolStr>) {
    if let Some(value) = previous {
        aliases.insert(SmolStr::new(name), value);
    } else {
        aliases.remove(name);
    }
}

fn has_explicit_self_param(params: &[Param]) -> bool {
    params
        .first()
        .is_some_and(|param| param.name.text.as_str() == "self")
}

fn substitute_trait_self(ty: &Ty, trait_def_id: DefId, self_ty: &Ty) -> Ty {
    match ty {
        Ty::Named(id, args) if *id == trait_def_id && args.is_empty() => self_ty.clone(),
        Ty::Named(id, args) => Ty::Named(
            *id,
            args.iter()
                .map(|arg| substitute_trait_self(arg, trait_def_id, self_ty))
                .collect(),
        ),
        Ty::Optional(inner) => Ty::Optional(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Result(ok, err) => Ty::Result(
            Box::new(substitute_trait_self(ok, trait_def_id, self_ty)),
            Box::new(substitute_trait_self(err, trait_def_id, self_ty)),
        ),
        Ty::List(inner) => Ty::List(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Map(key, value) => Ty::Map(
            Box::new(substitute_trait_self(key, trait_def_id, self_ty)),
            Box::new(substitute_trait_self(value, trait_def_id, self_ty)),
        ),
        Ty::Set(inner) => Ty::Set(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Range(inner) => Ty::Range(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Lazy(inner) => Ty::Lazy(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Future(inner) => Ty::Future(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::TaskJob(inner) => Ty::TaskJob(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Channel(inner) => Ty::Channel(Box::new(substitute_trait_self(
            inner,
            trait_def_id,
            self_ty,
        ))),
        Ty::Opaque { kind, args } => Ty::Opaque {
            kind: *kind,
            args: args
                .iter()
                .map(|arg| substitute_trait_self(arg, trait_def_id, self_ty))
                .collect(),
        },
        Ty::Tuple(items) => Ty::Tuple(
            items
                .iter()
                .map(|item| substitute_trait_self(item, trait_def_id, self_ty))
                .collect(),
        ),
        Ty::Func { params, ret } => Ty::Func {
            params: params
                .iter()
                .map(|param| substitute_trait_self(param, trait_def_id, self_ty))
                .collect(),
            ret: Box::new(substitute_trait_self(ret, trait_def_id, self_ty)),
        },
        _ => ty.clone(),
    }
}

fn contains_generic_param(ty: &Ty) -> bool {
    match ty {
        Ty::Param { .. } => true,
        Ty::Named(def_id, _) if (def_id.0 & 0x4000_0000) != 0 => true,
        Ty::Optional(inner)
        | Ty::List(inner)
        | Ty::Set(inner)
        | Ty::Range(inner)
        | Ty::Lazy(inner)
        | Ty::Future(inner)
        | Ty::TaskJob(inner)
        | Ty::Channel(inner) => contains_generic_param(inner),
        Ty::Result(ok, err) | Ty::Map(ok, err) => {
            contains_generic_param(ok) || contains_generic_param(err)
        }
        Ty::Opaque { args, .. } => args.iter().any(contains_generic_param),
        Ty::Tuple(items) => items.iter().any(contains_generic_param),
        Ty::Func { params, ret } => {
            params.iter().any(contains_generic_param) || contains_generic_param(ret)
        }
        Ty::Named(_, args) => args.iter().any(contains_generic_param),
        _ => false,
    }
}

fn freshen_stdlib_infers(params: Vec<Ty>, ret: Ty, salt: u32) -> (Vec<Ty>, Ty) {
    let mut remap = HashMap::new();
    let base = 1_000_000_u32.saturating_add(salt.saturating_mul(16));
    let params = params
        .into_iter()
        .map(|ty| freshen_infer_ty(ty, &mut remap, base))
        .collect();
    let ret = freshen_infer_ty(ret, &mut remap, base);
    (params, ret)
}

fn freshen_infer_ty(ty: Ty, remap: &mut HashMap<u32, u32>, base: u32) -> Ty {
    match ty {
        Ty::Infer(id) => {
            let next = if let Some(existing) = remap.get(&id) {
                *existing
            } else {
                let fresh = base.saturating_add(remap.len() as u32 + 1);
                remap.insert(id, fresh);
                fresh
            };
            Ty::Infer(next)
        }
        Ty::Optional(inner) => Ty::Optional(Box::new(freshen_infer_ty(*inner, remap, base))),
        Ty::Result(ok, err) => Ty::Result(
            Box::new(freshen_infer_ty(*ok, remap, base)),
            Box::new(freshen_infer_ty(*err, remap, base)),
        ),
        Ty::List(inner) => Ty::List(Box::new(freshen_infer_ty(*inner, remap, base))),
        Ty::Map(key, value) => Ty::Map(
            Box::new(freshen_infer_ty(*key, remap, base)),
            Box::new(freshen_infer_ty(*value, remap, base)),
        ),
        Ty::Set(inner) => Ty::Set(Box::new(freshen_infer_ty(*inner, remap, base))),
        Ty::Range(inner) => Ty::Range(Box::new(freshen_infer_ty(*inner, remap, base))),
        Ty::Lazy(inner) => Ty::Lazy(Box::new(freshen_infer_ty(*inner, remap, base))),
        Ty::Future(inner) => Ty::Future(Box::new(freshen_infer_ty(*inner, remap, base))),
        Ty::TaskJob(inner) => Ty::TaskJob(Box::new(freshen_infer_ty(*inner, remap, base))),
        Ty::Channel(inner) => Ty::Channel(Box::new(freshen_infer_ty(*inner, remap, base))),
        Ty::Opaque { kind, args } => Ty::Opaque {
            kind,
            args: args
                .into_iter()
                .map(|arg| freshen_infer_ty(arg, remap, base))
                .collect(),
        },
        Ty::Tuple(items) => Ty::Tuple(
            items
                .into_iter()
                .map(|item| freshen_infer_ty(item, remap, base))
                .collect(),
        ),
        Ty::Func { params, ret } => Ty::Func {
            params: params
                .into_iter()
                .map(|param| freshen_infer_ty(param, remap, base))
                .collect(),
            ret: Box::new(freshen_infer_ty(*ret, remap, base)),
        },
        Ty::Named(id, args) => Ty::Named(
            id,
            args.into_iter()
                .map(|arg| freshen_infer_ty(arg, remap, base))
                .collect(),
        ),
        other => other,
    }
}

fn infer_generic_substitution(template: &Ty, actual: &Ty, subst: &mut HashMap<u32, Ty>) {
    match (template, actual) {
        (Ty::Param { index, .. }, actual) => {
            if actual.is_error() || actual.contains_infer() || contains_generic_param(actual) {
                return;
            }
            subst.entry(*index).or_insert_with(|| actual.clone());
        }
        (Ty::Named(def_id, args_t), actual) if (def_id.0 & 0x4000_0000) != 0 => {
            let index = def_id.0 & 0x3FFF_FFFF;
            let (ctor, args_a) = strip_type_constructor_args(actual);
            subst.insert(index, ctor);
            for (arg_t, arg_a) in args_t.iter().zip(args_a) {
                infer_generic_substitution(arg_t, &arg_a, subst);
            }
        }
        (Ty::Optional(t), Ty::Optional(a))
        | (Ty::List(t), Ty::List(a))
        | (Ty::Set(t), Ty::Set(a))
        | (Ty::Range(t), Ty::Range(a))
        | (Ty::Lazy(t), Ty::Lazy(a))
        | (Ty::Future(t), Ty::Future(a))
        | (Ty::TaskJob(t), Ty::TaskJob(a))
        | (Ty::Channel(t), Ty::Channel(a)) => infer_generic_substitution(t, a, subst),
        (Ty::Result(ok_t, err_t), Ty::Result(ok_a, err_a))
        | (Ty::Map(ok_t, err_t), Ty::Map(ok_a, err_a)) => {
            infer_generic_substitution(ok_t, ok_a, subst);
            infer_generic_substitution(err_t, err_a, subst);
        }
        (
            Ty::Opaque {
                kind: kind_t,
                args: args_t,
            },
            Ty::Opaque {
                kind: kind_a,
                args: args_a,
            },
        ) if kind_t == kind_a && args_t.len() == args_a.len() => {
            for (arg_t, arg_a) in args_t.iter().zip(args_a) {
                infer_generic_substitution(arg_t, arg_a, subst);
            }
        }
        (Ty::Tuple(items_t), Ty::Tuple(items_a)) if items_t.len() == items_a.len() => {
            for (item_t, item_a) in items_t.iter().zip(items_a) {
                infer_generic_substitution(item_t, item_a, subst);
            }
        }
        (
            Ty::Func {
                params: params_t,
                ret: ret_t,
            },
            Ty::Func {
                params: params_a,
                ret: ret_a,
            },
        ) if params_t.len() == params_a.len() => {
            for (param_t, param_a) in params_t.iter().zip(params_a) {
                infer_generic_substitution(param_t, param_a, subst);
            }
            infer_generic_substitution(ret_t, ret_a, subst);
        }
        (Ty::Named(id_t, args_t), Ty::Named(id_a, args_a))
            if id_t == id_a && args_t.len() == args_a.len() =>
        {
            for (arg_t, arg_a) in args_t.iter().zip(args_a) {
                infer_generic_substitution(arg_t, arg_a, subst);
            }
        }
        _ => {}
    }
}

fn substitute_generic_params(ty: &Ty, subst: &HashMap<u32, Ty>) -> Ty {
    match ty {
        Ty::Param { index, .. } => subst.get(index).cloned().unwrap_or_else(|| ty.clone()),
        Ty::Optional(inner) => Ty::Optional(Box::new(substitute_generic_params(inner, subst))),
        Ty::Result(ok, err) => Ty::Result(
            Box::new(substitute_generic_params(ok, subst)),
            Box::new(substitute_generic_params(err, subst)),
        ),
        Ty::List(inner) => Ty::List(Box::new(substitute_generic_params(inner, subst))),
        Ty::Map(key, value) => Ty::Map(
            Box::new(substitute_generic_params(key, subst)),
            Box::new(substitute_generic_params(value, subst)),
        ),
        Ty::Set(inner) => Ty::Set(Box::new(substitute_generic_params(inner, subst))),
        Ty::Range(inner) => Ty::Range(Box::new(substitute_generic_params(inner, subst))),
        Ty::Lazy(inner) => Ty::Lazy(Box::new(substitute_generic_params(inner, subst))),
        Ty::Future(inner) => Ty::Future(Box::new(substitute_generic_params(inner, subst))),
        Ty::TaskJob(inner) => Ty::TaskJob(Box::new(substitute_generic_params(inner, subst))),
        Ty::Channel(inner) => Ty::Channel(Box::new(substitute_generic_params(inner, subst))),
        Ty::Opaque { kind, args } => Ty::Opaque {
            kind: *kind,
            args: args
                .iter()
                .map(|arg| substitute_generic_params(arg, subst))
                .collect(),
        },
        Ty::Tuple(items) => Ty::Tuple(
            items
                .iter()
                .map(|item| substitute_generic_params(item, subst))
                .collect(),
        ),
        Ty::Func { params, ret } => Ty::Func {
            params: params
                .iter()
                .map(|param| substitute_generic_params(param, subst))
                .collect(),
            ret: Box::new(substitute_generic_params(ret, subst)),
        },
        Ty::Named(def_id, args) if (def_id.0 & 0x4000_0000) != 0 => {
            let index = def_id.0 & 0x3FFF_FFFF;
            let substituted_args: Vec<Ty> = args
                .iter()
                .map(|arg| substitute_generic_params(arg, subst))
                .collect();
            if let Some(ctor) = subst.get(&index) {
                apply_type_constructor(ctor, substituted_args)
            } else {
                Ty::Named(*def_id, substituted_args)
            }
        }
        Ty::Named(def_id, args) => Ty::Named(
            *def_id,
            args.iter()
                .map(|arg| substitute_generic_params(arg, subst))
                .collect(),
        ),
        _ => ty.clone(),
    }
}

fn apply_type_constructor(ctor: &Ty, args: Vec<Ty>) -> Ty {
    match ctor {
        Ty::Named(def_id, _) => Ty::Named(*def_id, args),
        Ty::Opaque { kind, .. } => Ty::Opaque { kind: *kind, args },
        Ty::Optional(_) if !args.is_empty() => Ty::Optional(Box::new(args[0].clone())),
        Ty::List(_) if !args.is_empty() => Ty::List(Box::new(args[0].clone())),
        Ty::Set(_) if !args.is_empty() => Ty::Set(Box::new(args[0].clone())),
        Ty::Range(_) if !args.is_empty() => Ty::Range(Box::new(args[0].clone())),
        Ty::Lazy(_) if !args.is_empty() => Ty::Lazy(Box::new(args[0].clone())),
        Ty::Future(_) if !args.is_empty() => Ty::Future(Box::new(args[0].clone())),
        Ty::TaskJob(_) if !args.is_empty() => Ty::TaskJob(Box::new(args[0].clone())),
        Ty::Channel(_) if !args.is_empty() => Ty::Channel(Box::new(args[0].clone())),
        Ty::Result(_, _) if args.len() >= 2 => {
            Ty::Result(Box::new(args[0].clone()), Box::new(args[1].clone()))
        }
        Ty::Map(_, _) if args.len() >= 2 => {
            Ty::Map(Box::new(args[0].clone()), Box::new(args[1].clone()))
        }
        _ => ctor.clone(),
    }
}

fn strip_type_constructor_args(ty: &Ty) -> (Ty, Vec<Ty>) {
    match ty {
        Ty::Named(id, args) => (Ty::Named(*id, Vec::new()), args.clone()),
        Ty::Opaque { kind, args } => (
            Ty::Opaque {
                kind: *kind,
                args: Vec::new(),
            },
            args.clone(),
        ),
        Ty::Optional(inner) => (Ty::Optional(Box::new(Ty::Infer(0))), vec![*inner.clone()]),
        Ty::List(inner) => (Ty::List(Box::new(Ty::Infer(0))), vec![*inner.clone()]),
        Ty::Set(inner) => (Ty::Set(Box::new(Ty::Infer(0))), vec![*inner.clone()]),
        Ty::Range(inner) => (Ty::Range(Box::new(Ty::Infer(0))), vec![*inner.clone()]),
        Ty::Lazy(inner) => (Ty::Lazy(Box::new(Ty::Infer(0))), vec![*inner.clone()]),
        Ty::Future(inner) => (Ty::Future(Box::new(Ty::Infer(0))), vec![*inner.clone()]),
        Ty::TaskJob(inner) => (Ty::TaskJob(Box::new(Ty::Infer(0))), vec![*inner.clone()]),
        Ty::Channel(inner) => (Ty::Channel(Box::new(Ty::Infer(0))), vec![*inner.clone()]),
        Ty::Result(ok, err) => (
            Ty::Result(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0))),
            vec![*ok.clone(), *err.clone()],
        ),
        Ty::Map(key, val) => (
            Ty::Map(Box::new(Ty::Infer(0)), Box::new(Ty::Infer(0))),
            vec![*key.clone(), *val.clone()],
        ),
        _ => (ty.clone(), Vec::new()),
    }
}

fn param_default_expr(kind: &ParamKind) -> Option<&Expr> {
    match kind {
        ParamKind::Default(expr) | ParamKind::DefaultAndContract(expr, _) => Some(expr),
        _ => None,
    }
}

fn param_binding_ty(param: &ori_ast::item::Param, ty: Ty) -> Ty {
    if matches!(param.kind, ParamKind::Variadic) {
        Ty::List(Box::new(ty))
    } else {
        ty
    }
}

fn comparison_op_text(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        _ => "<comparison>",
    }
}

fn operator_trait_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "Addable",
        BinaryOp::Sub => "Subtractable",
        BinaryOp::Mul => "Multiplicable",
        BinaryOp::Div => "Divisible",
        BinaryOp::Eq | BinaryOp::Ne => "Equatable",
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => "Comparable",
        _ => "",
    }
}

fn operator_method_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "subtract",
        BinaryOp::Mul => "multiply",
        BinaryOp::Div => "divide",
        BinaryOp::Eq | BinaryOp::Ne => "equals",
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => "compare",
        _ => "",
    }
}

fn unsupported_comparison_reason(ty: &Ty) -> &'static str {
    match ty {
        Ty::Func { .. } => "function values are not comparable",
        Ty::Any(_) => "dynamic dispatch values do not define equality",
        Ty::Bytes => "byte value equality is not implemented yet",
        Ty::Bool => "ordering is only implemented for numeric types",
        Ty::String => "ordering is only implemented for numeric types",
        Ty::Named(_, _)
        | Ty::Tuple(_)
        | Ty::Optional(_)
        | Ty::Result(_, _)
        | Ty::List(_)
        | Ty::Map(_, _)
        | Ty::Set(_)
        | Ty::Range(_)
        | Ty::Lazy(_)
        | Ty::Future(_)
        | Ty::TaskJob(_)
        | Ty::Channel(_)
        | Ty::AtomicInt
        | Ty::TaskJoinError
        | Ty::ChannelSendError
        | Ty::ChannelReceiveError
        | Ty::Opaque { .. } => "structural comparison is not implemented yet",
        _ => "this type has no built-in comparison",
    }
}

fn comparison_action(ty: &Ty) -> &'static str {
    match ty {
        Ty::Func { .. } => "remove the comparison or compare a separate stable value",
        Ty::Any(_) => "compare concrete values before boxing them as `any[Trait]`",
        Ty::Named(_, _)
        | Ty::Tuple(_)
        | Ty::Optional(_)
        | Ty::Result(_, _)
        | Ty::List(_)
        | Ty::Map(_, _)
        | Ty::Set(_)
        | Ty::Range(_)
        | Ty::Lazy(_)
        | Ty::Future(_)
        | Ty::TaskJob(_)
        | Ty::Channel(_)
        | Ty::AtomicInt
        | Ty::TaskJoinError
        | Ty::ChannelSendError
        | Ty::ChannelReceiveError
        | Ty::Opaque { .. } => {
            "wait for structural equality support or compare explicit fields manually"
        }
        _ => "use a supported primitive comparison or an explicit helper function",
    }
}

fn is_current_integer_hash_supported(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Int
            | Ty::Int8
            | Ty::Int16
            | Ty::Int32
            | Ty::Int64
            | Ty::U8
            | Ty::U16
            | Ty::U32
            | Ty::U64
            | Ty::Error
            | Ty::Infer(_)
    )
}

fn param_contract_expr(kind: &ParamKind) -> Option<&Expr> {
    match kind {
        ParamKind::Contract(expr) | ParamKind::DefaultAndContract(_, expr) => Some(expr),
        _ => None,
    }
}

fn min_required_arg_count(param_count: usize, param_defaults: &[bool]) -> usize {
    (0..param_count)
        .rfind(|index| !param_defaults.get(*index).copied().unwrap_or(false))
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn display_tys(types: &[Ty]) -> String {
    types
        .iter()
        .map(|ty| ty.display())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Extract the element type from a `list[T]`, `set[T]`, `range[T]`, or string.
fn elem_of(ty: &Ty) -> Option<Ty> {
    match ty {
        Ty::List(t) | Ty::Set(t) | Ty::Range(t) => Some(*t.clone()),
        Ty::Map(key, _) => Some(*key.clone()),
        Ty::String => Some(Ty::String), // string iteration yields strings (grapheme clusters)
        Ty::Bytes => Some(Ty::U8),
        Ty::Opaque { kind, args } if kind.is_list_backed_collection() => args.first().cloned(),
        Ty::Opaque {
            kind: OpaqueTy::Heap | OpaqueTy::Graph,
            args,
        } => args.first().cloned(),
        Ty::Opaque {
            kind: OpaqueTy::HashTable,
            args,
        } => args.first().cloned(),
        _ => None,
    }
}

fn for_second_binding_ty(ty: &Ty) -> Ty {
    match ty {
        Ty::Map(_, value) => *value.clone(),
        // For lists, sets, strings, bytes: second binding is always the index (int)
        Ty::List(_) | Ty::Set(_) | Ty::String | Ty::Bytes | Ty::Range(_) => Ty::Int,
        Ty::Opaque { kind, .. }
            if kind.is_list_backed_collection()
                || matches!(kind, OpaqueTy::Heap | OpaqueTy::Graph | OpaqueTy::HashTable) =>
        {
            Ty::Int
        }
        // For any other type, the second binding is an error — the type doesn't
        // support a meaningful second binding, and the type-checker will flag the
        // iterable itself as non-iterable before we ever bind this.
        _ => Ty::Error,
    }
}
