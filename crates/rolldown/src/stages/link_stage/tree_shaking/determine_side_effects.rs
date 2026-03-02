use oxc_module_graph::{LinkConfig, SideEffectsHooks};
use rolldown_common::{Module, WrapKind, side_effects::DeterminedSideEffects};

use crate::{stages::link_stage::LinkStage, types::linking_metadata::LinkingMetadataVec};

/// Rolldown-specific side-effects checker that handles star-export wrapping logic.
///
/// For `export * from '...'` edges, this checks whether the importee needs
/// wrapping (CJS/ESM) or has dynamic exports, which introduces side effects
/// for the `__reExport` / init calls.
struct RolldownSideEffectsHooks<'a> {
  metas: &'a LinkingMetadataVec,
}

impl SideEffectsHooks for RolldownSideEffectsHooks<'_> {
  fn star_export_has_extra_side_effects(
    &self,
    _importer: oxc_module_graph::types::ModuleIdx,
    importee: oxc_module_graph::types::ModuleIdx,
  ) -> bool {
    let rd_importee = rolldown_common::ModuleIdx::from_usize(importee.index());
    let importee_linking_info = &self.metas[rd_importee];
    match importee_linking_info.wrap_kind() {
      // If importee has dynamic exports (e.g., re-exports from CJS), we need side effects
      // to ensure the __reExport call is preserved.
      WrapKind::None => importee_linking_info.has_dynamic_exports,
      // Wrapped modules always need the side effect(`init_xxx` for esm and `require_xxx` for cjs) for proper initialization
      WrapKind::Cjs | WrapKind::Esm => true,
    }
  }
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn determine_side_effects(&mut self) {
    let hooks = RolldownSideEffectsHooks { metas: &self.metas };

    let config = LinkConfig { side_effects_hooks: Some(&hooks), ..Default::default() };

    let result = oxc_module_graph::determine_side_effects(&self.link_kernel.graph, &config);

    // Apply propagated side effects to modules.
    // Only update Analyzed(false) → Analyzed(true). Preserves UserDefined/NoTreeshake.
    for (oxc_idx, has) in &result {
      let idx = rolldown_common::ModuleIdx::from_usize(oxc_idx.index());
      if *has {
        if let Module::Normal(m) = &mut self.module_table[idx] {
          if matches!(m.side_effects, DeterminedSideEffects::Analyzed(false)) {
            m.side_effects = DeterminedSideEffects::Analyzed(true);
          }
        }
      }
    }
  }
}
