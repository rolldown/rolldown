//! On-demand (body-demand) inclusion of side-effect statements for user-declared
//! side-effect-free modules. See [`compute_body_demand_keys`] for the model.

use rolldown_common::{
  ExportsKind, IndexModules, ModuleIdx, SymbolRef, SymbolRefDb, side_effects::DeterminedSideEffects,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{stages::link_stage::LinkStage, type_alias::IndexStmtInfos};

use super::inclusion_core::{InclusionModuleFacts, compute_body_demand_keys_core};

struct LegacyModuleFacts<'a> {
  modules: &'a IndexModules,
}

impl InclusionModuleFacts for LegacyModuleFacts<'_> {
  fn exports_kind(&self, module_idx: ModuleIdx) -> ExportsKind {
    self.modules[module_idx]
      .as_normal()
      .expect("body-demand facts require a normal module")
      .exports_kind
  }

  fn side_effects(&self, module_idx: ModuleIdx) -> DeterminedSideEffects {
    *self.modules[module_idx].side_effects()
  }
}

/// Whether side-effectful statements of `module` that reference module-level
/// symbols are included on demand instead of being swept in unconditionally by
/// [`super::include_statements::include_module`].
///
/// A module the user declared side-effect free (`"sideEffects": false` in
/// package.json, `treeshake.moduleSideEffects: false`) must not have such
/// statements retained merely because they *evaluate* effects. Without this,
/// any force-inclusion of the module — the wrapper-ref edge under
/// `strictExecutionOrder`, `require()` of ESM, or an inlined dynamic import
/// under `codeSplitting: false` — resurrects side-effect statements that plain
/// (unwrapped) tree-shaking provably drops, making inclusion inconsistent
/// between `strictExecutionOrder: true` and `false`. It also breaks the lazy
/// barrel loader's contract: the loader defers import records by requested
/// exports, so a retained statement referencing a never-requested import ends up
/// referencing a record that was correctly never loaded and crashes at runtime
/// (#9691, #9806, #9961, #9964, #10013, #10048).
///
/// Instead such statements join when the module's *body* is demanded: one of
/// its own (non-re-export) exports or its namespace object becomes used (via
/// `body_demand_keys`) — e.g. `foo.bar = 1` stays once `foo` is
/// demanded, `#7597`'s top-level asserts stay because the entry imports the
/// module's own `Modal`. This mirrors the lazy-barrel loader exactly: body
/// demand is its `has_local_export`/`All` case, in which it loads every plain
/// import record of the module — so a kept statement can never reference an
/// unloaded record, and a deferred record is only ever referenced by dropped
/// statements.
///
/// Statements referencing no module-level symbol (a bare `console.log()`) and
/// import/re-export statements (which drive wrapper init calls and side-effect
/// imports) cannot dangle an import and keep the unconditional sweep.
/// Exemptions: user-defined entry modules (the requested program keeps its
/// body — dynamic entries instead join through namespace/own-export body
/// demand, so a dead pure dynamic import can't resurrect a body), modules
/// using `eval` (it can observe anything), and CommonJS modules (their exports
/// are side-effect assignments that the demand edges don't model).
/// Build the demand edges for modules whose side-effectful statements are
/// included on demand: body-demand
/// key -> the module whose gated side-effect statements it demands.
///
/// A module's body counts as demanded when one of its *own* exports (an export
/// that doesn't re-export an import) or its namespace object becomes used —
/// mirroring the lazy-barrel loader's `local` classification, which loads every
/// plain import record of the module exactly in those cases. Demand through
/// pure re-exports leaves the body dropped.
///
/// The gated statements themselves are enumerated at demand time (they are a pure
/// function of the immutable `stmt_infos`); this map only answers "whose body does
/// using this symbol demand?". Every key's canonical is owned by the module itself
/// (own exports and namespace refs never link across modules), so the map is
/// one-to-one per module.
pub fn compute_body_demand_keys(
  modules: &IndexModules,
  stmt_infos: &IndexStmtInfos,
  symbols: &SymbolRefDb,
  treeshake_enabled: bool,
  entry_module_idxs: &FxHashSet<ModuleIdx>,
) -> FxHashMap<SymbolRef, ModuleIdx> {
  compute_body_demand_keys_core(
    &LegacyModuleFacts { modules },
    modules,
    stmt_infos,
    symbols,
    treeshake_enabled,
    entry_module_idxs,
  )
}

impl LinkStage<'_> {
  /// User-defined (and emitted) entries — exempt from on-demand side-effect
  /// gating: they are the requested program. A dynamic entry participates like
  /// any module: an observed namespace or used export is body demand, while a
  /// dead pure dynamic import must not resurrect the body its removal already
  /// models as an empty namespace.
  pub(super) fn user_defined_entry_module_idxs(&self) -> FxHashSet<ModuleIdx> {
    self
      .entries
      .values()
      .flatten()
      .filter(|entry| entry.kind.is_user_defined())
      .map(|entry| entry.idx)
      .collect()
  }
}
