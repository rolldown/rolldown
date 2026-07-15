//! Standalone passes that run around the inclusion fixpoint: CJS bailout export
//! inclusion (per fixpoint iteration), runtime-helper collection/inclusion (after
//! the fixpoint), and `preserveModules` re-export interface preservation (once,
//! after convergence).

use rolldown_common::{IndexModules, ModuleIdx, RuntimeHelper, RuntimeModuleBrief};
// On wasm targets `rolldown_utils::rayon` shims `par_iter()` to a plain `Iterator`,
// so `zip_eq` resolves via `Itertools` there and via `IndexedParallelIterator` natively.
use crate::types::linking_metadata::LinkingMetadataVec;
#[cfg(target_family = "wasm")]
use itertools::Itertools as _;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};

use super::include_statements::{
  IncludeContext, ModuleInclusionVec, include_cjs_bailout_exports_with_core,
  include_runtime_symbol_with_core, preserve_reexported_interfaces_with_core,
};

pub(super) fn include_cjs_bailout_exports(
  context: &mut IncludeContext,
  bailout_modules: impl IntoIterator<Item = ModuleIdx>,
) {
  include_cjs_bailout_exports_with_core(context, bailout_modules);
}

/// Collects all depended runtime helpers from included modules only.
/// Eliminated modules may have runtime helpers set (for propagation to importers),
/// but we should only include the runtime if an included module actually needs it.
pub(super) fn collect_depended_runtime_helpers(
  modules: &IndexModules,
  metas: &LinkingMetadataVec,
  is_module_included_vec: &ModuleInclusionVec,
) -> RuntimeHelper {
  let iter = modules.par_iter().zip_eq(metas.par_iter()).filter_map(|(module, meta)| {
    module
      .as_normal()
      .filter(|m| is_module_included_vec.has_bit(m.idx))
      .map(|_| meta.depended_runtime_helper)
  });

  #[cfg(not(target_family = "wasm"))]
  let depended_runtime_helper = iter.reduce(RuntimeHelper::default, |a, b| a | b);
  #[cfg(target_family = "wasm")]
  let depended_runtime_helper = iter.reduce(|a, b| a | b).unwrap_or_default();

  depended_runtime_helper
}

pub fn include_runtime_symbol(
  ctx: &mut IncludeContext,
  runtime: &RuntimeModuleBrief,
  depended_runtime_helper: RuntimeHelper,
) {
  include_runtime_symbol_with_core(ctx, runtime, depended_runtime_helper);
}

/// if no export is used, and the module has no side effects, the module should not be included
/// Preserve re-exported interfaces for `preserveModules`.
///
/// Every module maps 1:1 to an output file whose `export { ... }` must mirror the source module's
/// interface. A re-export (`export { x } from './y'`) resolves to a *canonical* symbol owned by
/// `./y`, and consumers bind that canonical directly, bypassing this module's facade binding — so
/// the facade is tree-shaken out of this file's exports (issue #9122).
///
/// We re-mark a facade as used and include its re-export statement (so the cross-chunk import is
/// generated) only when the facade is actually consumed *through* this module — i.e. it appears as
/// an intermediate in the export chain of some used import, recorded in
/// `normal_symbol_exports_chain_map`. This is chain-granular: a re-export nobody imports through
/// this module stays tree-shaken, even when the same canonical is used via a different module path
/// (e.g. a side-effect-only wrapper re-exporting `foo` while a consumer reaches `foo` straight from
/// its source); a genuinely-unused export likewise stays tree-shaken because no used import reaches
/// it. `#9122`'s `wrapper` keeps `StateCode`/`getX` because the entry imports them *through*
/// `wrapper` (`export { … } from './wrapper.js'`). The synthetic runtime module is excluded.
///
/// Must run once, after the inclusion fixpoint has settled `used_symbol_refs`. It only includes
/// re-export statements that reference already-retained canonicals, so it introduces no new
/// reachable values and needs no further convergence.
pub(super) fn preserve_reexported_interfaces(ctx: &mut IncludeContext) {
  preserve_reexported_interfaces_with_core(ctx);
}
