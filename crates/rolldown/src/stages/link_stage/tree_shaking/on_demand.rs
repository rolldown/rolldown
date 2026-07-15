//! On-demand (body-demand) inclusion of side-effect statements for user-declared
//! side-effect-free modules. See [`side_effects_included_on_demand`] for the model.

use rolldown_common::{
  ExportOrigin, ExportsKind, IndexModules, Module, ModuleIdx, NormalModule, StmtInfo, SymbolRef,
  SymbolRefDb, side_effects::DeterminedSideEffects,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{stages::link_stage::LinkStage, type_alias::IndexStmtInfos};

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
pub(super) fn side_effects_included_on_demand(
  module: &NormalModule,
  entry_module_idxs: &FxHashSet<ModuleIdx>,
) -> bool {
  matches!(module.side_effects, DeterminedSideEffects::UserDefined(false))
    && matches!(module.exports_kind, ExportsKind::Esm)
    && !module.meta.has_eval()
    && !entry_module_idxs.contains(&module.idx)
}

/// Build the demand edges for modules whose side-effectful statements are
/// included on demand (see [`side_effects_included_on_demand`]): body-demand
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
  let mut map: FxHashMap<SymbolRef, ModuleIdx> = FxHashMap::default();
  if !treeshake_enabled {
    return map;
  }
  for module in modules.iter().filter_map(Module::as_normal) {
    if !side_effects_included_on_demand(module, entry_module_idxs) {
      continue;
    }
    let has_gated_stmts = stmt_infos[module.idx]
      .iter_enumerated_without_namespace_stmt()
      .any(|(_, stmt_info)| is_gated_side_effect_stmt(stmt_info));
    if !has_gated_stmts {
      continue;
    }
    let body_demand_keys = module
      .named_exports
      .values()
      .filter(|local_export| matches!(module.classify_export(local_export), ExportOrigin::Own))
      .map(|local_export| symbols.canonical_ref_for(local_export.referenced))
      .chain(std::iter::once(module.namespace_object_ref));
    for key in body_demand_keys {
      let previous = map.insert(key, module.idx);
      debug_assert!(
        previous.is_none_or(|prev| prev == module.idx),
        "a body-demand key must belong to exactly one module"
      );
    }
  }
  map
}

/// A statement that joins through body demand rather than the unconditional
/// sweep of [`super::include_statements::include_module`]: it evaluates side
/// effects *and* reads module-level bindings (so it can dangle a lazily-deferred
/// import), and is not an import/re-export statement (those drive wrapper init
/// calls and side-effect-import inclusion; `ReferenceNeededSymbolsPass` also
/// pushes wrapper refs onto them, which must not count as user references).
pub(super) fn is_gated_side_effect_stmt(stmt_info: &StmtInfo) -> bool {
  stmt_info.eval_flags.has_side_effect_for_tree_shaking()
    && !stmt_info.referenced_symbols.is_empty()
    && stmt_info.import_records.is_empty()
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
