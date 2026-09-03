//! Typed conditional-compilation predicates and source filtering.
//!
//! Parsing always builds the complete source file. This module then removes
//! inactive top-level declarations before name resolution so every backend
//! observes the same active program.

use ori_ast::common::{AttrArg, CfgPredicate};
use ori_ast::item::SourceFile;
use ori_diagnostics::{Diagnostic, DiagnosticSink, FileId, Label};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionProfile {
    Standalone,
    Embedded,
}

impl ExecutionProfile {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "standalone" => Some(Self::Standalone),
            "embedded" => Some(Self::Embedded),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standalone => "standalone",
            Self::Embedded => "embedded",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CfgContext {
    pub target_triple: String,
    pub target_arch: String,
    pub target_os: String,
    pub target_family: String,
    pub execution_profile: ExecutionProfile,
    pub declared_features: BTreeSet<String>,
    pub enabled_features: BTreeSet<String>,
}

impl CfgContext {
    pub fn new(
        target_triple: impl Into<String>,
        execution_profile: ExecutionProfile,
        declared_features: BTreeSet<String>,
        enabled_features: BTreeSet<String>,
    ) -> Self {
        let target_triple = target_triple.into();
        let (target_arch, target_os, target_family) = target_facts(&target_triple);
        Self {
            target_triple,
            target_arch,
            target_os,
            target_family,
            execution_profile,
            declared_features,
            enabled_features,
        }
    }

    pub fn host_default() -> Self {
        let triple = format!(
            "{}-unknown-{}",
            std::env::consts::ARCH,
            std::env::consts::OS
        );
        Self::new(
            triple,
            ExecutionProfile::Standalone,
            BTreeSet::new(),
            BTreeSet::new(),
        )
    }
}

impl Default for CfgContext {
    fn default() -> Self {
        Self::host_default()
    }
}

/// Return whether a target triple can be represented by cfg v1 without
/// conflating an unknown operating system with an explicitly OS-free target.
pub fn is_supported_target_triple(target: &str) -> bool {
    let (arch, os, _) = target_facts(target);
    is_known_target_arch(&arch)
        && is_known_target_os(&os)
        && (os != "none" || target_explicitly_has_no_os(target))
}

fn target_explicitly_has_no_os(target: &str) -> bool {
    target.split('-').any(|component| component == "none")
        || target.ends_with("-unknown")
        || target.ends_with("-cuda")
}

/// Remove inactive declarations after parsing and before name resolution.
///
/// Invalid predicates remain active. This ensures a typo never silently hides
/// code while diagnostics are being reported.
pub fn filter_source_file(
    file: &mut SourceFile,
    file_id: FileId,
    context: &CfgContext,
    sink: &mut DiagnosticSink,
) {
    file.items.retain(|item| {
        let attrs: Vec<_> = item
            .attrs
            .iter()
            .filter(|attr| attr.name.text == "cfg")
            .collect();
        if attrs.is_empty() {
            return true;
        }
        if attrs.len() > 1 {
            let repeated = attrs[1];
            sink.emit(
                Diagnostic::error(
                    "cfg.duplicate",
                    "a declaration may have only one `@cfg` attribute",
                )
                .with_label(Label::primary(
                    file_id,
                    repeated.span,
                    "second `@cfg` is here",
                ))
                .with_action("combine predicates with `all(...)` or `any(...)`"),
            );
            return true;
        }

        let attr = attrs[0];
        let [AttrArg::Cfg(predicate)] = attr.args.as_slice() else {
            sink.emit(
                Diagnostic::error(
                    "cfg.invalid_predicate",
                    "`@cfg` requires one structured predicate",
                )
                .with_label(Label::primary(
                    file_id,
                    attr.span,
                    "invalid conditional-compilation predicate",
                ))
                .with_action(
                    "use `@cfg(target_os: linux)` or compose predicates with `all`, `any`, and `not`",
                ),
            );
            return true;
        };

        evaluate(predicate, file_id, context, sink).unwrap_or(true)
    });
}

fn evaluate(
    predicate: &CfgPredicate,
    file_id: FileId,
    context: &CfgContext,
    sink: &mut DiagnosticSink,
) -> Option<bool> {
    match predicate {
        CfgPredicate::NameValue { key, value, span } => {
            evaluate_name_value(key.as_str(), value.as_str(), *span, file_id, context, sink)
        }
        CfgPredicate::Call {
            operator,
            predicates,
            span,
        } => match operator.as_str() {
            "all" if predicates.is_empty() => {
                invalid_arity("all", "at least one", *span, file_id, sink);
                None
            }
            "any" if predicates.is_empty() => {
                invalid_arity("any", "at least one", *span, file_id, sink);
                None
            }
            "not" if predicates.len() != 1 => {
                invalid_arity("not", "exactly one", *span, file_id, sink);
                None
            }
            "all" => evaluate_many(predicates, file_id, context, sink)
                .map(|values| values.into_iter().all(std::convert::identity)),
            "any" => evaluate_many(predicates, file_id, context, sink)
                .map(|values| values.into_iter().any(std::convert::identity)),
            "not" => evaluate(&predicates[0], file_id, context, sink).map(|value| !value),
            other => {
                sink.emit(
                    Diagnostic::error(
                        "cfg.unknown_operator",
                        format!("unknown conditional-compilation operator `{other}`"),
                    )
                    .with_label(Label::primary(file_id, operator.span, "unknown operator"))
                    .with_action("use `all(...)`, `any(...)`, or `not(...)`"),
                );
                None
            }
        },
    }
}

fn evaluate_many(
    predicates: &[CfgPredicate],
    file_id: FileId,
    context: &CfgContext,
    sink: &mut DiagnosticSink,
) -> Option<Vec<bool>> {
    let mut values = Vec::with_capacity(predicates.len());
    let mut valid = true;
    for predicate in predicates {
        match evaluate(predicate, file_id, context, sink) {
            Some(value) => values.push(value),
            None => valid = false,
        }
    }
    valid.then_some(values)
}

fn evaluate_name_value(
    key: &str,
    value: &str,
    span: ori_diagnostics::Span,
    file_id: FileId,
    context: &CfgContext,
    sink: &mut DiagnosticSink,
) -> Option<bool> {
    match key {
        "target_os" if is_known_target_os(value) => Some(context.target_os == value),
        "target_os" => unknown_value(
            key,
            value,
            "use a canonical OS name such as `linux`, `windows`, `macos`, or `none`",
            span,
            file_id,
            sink,
        ),
        "target_arch" if is_known_target_arch(value) => Some(context.target_arch == value),
        "target_arch" => unknown_value(
            key,
            value,
            "use a canonical architecture name such as `x86_64`, `aarch64`, or `wasm32`",
            span,
            file_id,
            sink,
        ),
        "target_family" if matches!(value, "unix" | "windows" | "wasm" | "none") => {
            Some(context.target_family == value)
        }
        "target_family" => unknown_value(
            key,
            value,
            "use `unix`, `windows`, `wasm`, or `none`",
            span,
            file_id,
            sink,
        ),
        "execution_profile" => match ExecutionProfile::parse(value) {
            Some(profile) => Some(context.execution_profile == profile),
            None => {
                sink.emit(
                    Diagnostic::error(
                        "cfg.unknown_value",
                        format!("unknown execution profile `{value}`"),
                    )
                    .with_label(Label::primary(file_id, span, "unknown profile"))
                    .with_action("use `standalone` or `embedded`"),
                );
                None
            }
        },
        "feature" if !context.declared_features.contains(value) => {
            sink.emit(
                Diagnostic::error(
                    "cfg.unknown_feature",
                    format!("feature `{value}` is not declared by the project"),
                )
                .with_label(Label::primary(file_id, span, "undeclared feature"))
                .with_action("declare the feature under `[features]` in the project manifest"),
            );
            None
        }
        "feature" => Some(context.enabled_features.contains(value)),
        other => {
            sink.emit(
                Diagnostic::error(
                    "cfg.unknown_key",
                    format!("unknown conditional-compilation key `{other}`"),
                )
                .with_label(Label::primary(file_id, span, "unknown configuration key"))
                .with_action(
                    "use `target_os`, `target_arch`, `target_family`, `execution_profile`, or `feature`",
                ),
            );
            None
        }
    }
}

fn unknown_value(
    key: &str,
    value: &str,
    action: &'static str,
    span: ori_diagnostics::Span,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) -> Option<bool> {
    sink.emit(
        Diagnostic::error(
            "cfg.unknown_value",
            format!("unknown value `{value}` for `{key}`"),
        )
        .with_label(Label::primary(file_id, span, "unknown configuration value"))
        .with_action(action),
    );
    None
}

fn is_known_target_os(value: &str) -> bool {
    matches!(
        value,
        "linux"
            | "windows"
            | "macos"
            | "android"
            | "ios"
            | "freebsd"
            | "netbsd"
            | "openbsd"
            | "dragonfly"
            | "solaris"
            | "illumos"
            | "haiku"
            | "aix"
            | "emscripten"
            | "wasi"
            | "visionos"
            | "tvos"
            | "watchos"
            | "none"
    )
}

fn is_known_target_arch(value: &str) -> bool {
    matches!(
        value,
        "x86"
            | "x86_64"
            | "arm"
            | "aarch64"
            | "mips"
            | "mips64"
            | "powerpc"
            | "powerpc64"
            | "riscv32"
            | "riscv64"
            | "s390x"
            | "sparc"
            | "sparc64"
            | "wasm32"
            | "wasm64"
            | "loongarch64"
            | "bpf"
            | "csky"
            | "hexagon"
            | "m68k"
            | "nvptx64"
            | "avr"
            | "msp430"
            | "xtensa"
    )
}

fn invalid_arity(
    operator: &str,
    expected: &str,
    span: ori_diagnostics::Span,
    file_id: FileId,
    sink: &mut DiagnosticSink,
) {
    sink.emit(
        Diagnostic::error(
            "cfg.invalid_arity",
            format!("`{operator}` expects {expected} predicate"),
        )
        .with_label(Label::primary(file_id, span, "invalid predicate count")),
    );
}

fn target_facts(target: &str) -> (String, String, String) {
    let raw_arch = target.split('-').next().unwrap_or(target);
    let arch = if matches!(raw_arch, "i386" | "i486" | "i586" | "i686") {
        "x86"
    } else if raw_arch == "wasm32v1" {
        "wasm32"
    } else if raw_arch.starts_with("armv") || raw_arch.starts_with("thumb") {
        "arm"
    } else if raw_arch.starts_with("aarch64") || matches!(raw_arch, "arm64_32" | "arm64e") {
        "aarch64"
    } else if raw_arch.starts_with("mips64") {
        "mips64"
    } else if raw_arch.starts_with("mips") {
        "mips"
    } else if raw_arch.starts_with("powerpc64") {
        "powerpc64"
    } else if raw_arch.starts_with("powerpc") {
        "powerpc"
    } else if raw_arch.starts_with("riscv32") {
        "riscv32"
    } else if raw_arch.starts_with("riscv64") {
        "riscv64"
    } else if raw_arch.starts_with("bpf") {
        "bpf"
    } else if raw_arch == "sparcv9" {
        "sparc64"
    } else {
        raw_arch
    };
    let os = if target.contains("windows") {
        "windows"
    } else if target.contains("android") {
        "android"
    } else if target.contains("linux") {
        "linux"
    } else if target.contains("darwin") || target.contains("apple-macos") {
        "macos"
    } else if target.contains("ios") {
        "ios"
    } else if target.contains("freebsd") {
        "freebsd"
    } else if target.contains("netbsd") {
        "netbsd"
    } else if target.contains("openbsd") {
        "openbsd"
    } else if target.contains("dragonfly") {
        "dragonfly"
    } else if target.contains("solaris") {
        "solaris"
    } else if target.contains("illumos") {
        "illumos"
    } else if target.contains("haiku") {
        "haiku"
    } else if target.contains("aix") {
        "aix"
    } else if target.contains("emscripten") {
        "emscripten"
    } else if target.contains("wasi") {
        "wasi"
    } else if target.contains("visionos") {
        "visionos"
    } else if target.contains("tvos") {
        "tvos"
    } else if target.contains("watchos") {
        "watchos"
    } else {
        "none"
    };
    let family = if os == "windows" {
        "windows"
    } else if matches!(arch, "wasm32" | "wasm64") {
        "wasm"
    } else if matches!(
        os,
        "linux"
            | "android"
            | "macos"
            | "ios"
            | "freebsd"
            | "netbsd"
            | "openbsd"
            | "dragonfly"
            | "solaris"
            | "illumos"
            | "haiku"
            | "aix"
            | "visionos"
            | "tvos"
            | "watchos"
    ) {
        "unix"
    } else {
        "none"
    };
    (arch.to_string(), os.to_string(), family.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_facts_cover_desktop_and_wasm_targets() {
        assert_eq!(
            target_facts("x86_64-unknown-linux-gnu"),
            ("x86_64".into(), "linux".into(), "unix".into())
        );
        assert_eq!(
            target_facts("aarch64-pc-windows-msvc"),
            ("aarch64".into(), "windows".into(), "windows".into())
        );
        assert_eq!(
            target_facts("wasm32-unknown-unknown"),
            ("wasm32".into(), "none".into(), "wasm".into())
        );
        assert_eq!(
            target_facts("i686-unknown-linux-gnu"),
            ("x86".into(), "linux".into(), "unix".into())
        );
        assert_eq!(
            target_facts("thumbv7em-none-eabihf"),
            ("arm".into(), "none".into(), "none".into())
        );
        assert_eq!(
            target_facts("wasm32v1-none"),
            ("wasm32".into(), "none".into(), "wasm".into())
        );
        assert_eq!(
            target_facts("avr-none"),
            ("avr".into(), "none".into(), "none".into())
        );
        assert_eq!(
            target_facts("msp430-none-elf"),
            ("msp430".into(), "none".into(), "none".into())
        );
        assert_eq!(
            target_facts("aarch64-apple-tvos"),
            ("aarch64".into(), "tvos".into(), "unix".into())
        );
        assert_eq!(
            target_facts("arm64e-apple-darwin"),
            ("aarch64".into(), "macos".into(), "unix".into())
        );
        assert_eq!(
            target_facts("arm64_32-apple-watchos"),
            ("aarch64".into(), "watchos".into(), "unix".into())
        );
        assert!(is_supported_target_triple("wasm32-unknown-unknown"));
        assert!(is_supported_target_triple("nvptx64-nvidia-cuda"));
        assert!(!is_supported_target_triple("x86_64-unknown-fuchsia"));
        assert!(!is_supported_target_triple("amdgcn-amd-amdhsa"));
    }
}
