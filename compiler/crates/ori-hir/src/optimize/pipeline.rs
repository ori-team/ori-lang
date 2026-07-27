//! Mid-end pass driver.

use crate::hir::HirModule;

use super::const_fold::fold_module;
use super::dce::dce_module;
use super::inline_leafs::inline_leafs_module;
use super::strength_reduce::strength_reduce_module;

/// Optimisation aggressiveness for HIR mid-end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// No mid-end rewrites (tests / raw lower).
    None,
    /// Product default: const fold + DCE + pure-loop strength reduction.
    Default,
    /// Default passes plus monomorphic leaf inlining.
    Aggressive,
}

impl OptLevel {
    /// Resolve from `ORI_OPT` env: `none` | `default` | `aggressive`.
    pub fn from_env() -> Self {
        match std::env::var("ORI_OPT").ok().as_deref() {
            Some("none") | Some("0") => Self::None,
            Some("aggressive") | Some("2") => Self::Aggressive,
            _ => Self::Default,
        }
    }
}

/// Run the mid-end pipeline on `module` in place.
pub fn optimize_module(module: &mut HirModule, level: OptLevel) {
    match level {
        OptLevel::None => {}
        OptLevel::Default | OptLevel::Aggressive => {
            // Bounded fixed-point: fold → strength reduce → DCE.
            for _ in 0..4 {
                let before = format!("{module:?}");
                fold_module(module);
                strength_reduce_module(module);
                dce_module(module);
                let after = format!("{module:?}");
                if before == after {
                    break;
                }
            }
            if level == OptLevel::Aggressive {
                inline_leafs_module(module);
                // One more fold/DCE round after inlining exposes constants.
                fold_module(module);
                dce_module(module);
            }
        }
    }
}
