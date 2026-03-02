/// Rolldown-specific per-module payload that lives outside the generic graph.
///
/// Contains heavy data not needed for generic linking: source text, AST,
/// sourcemap chain, render mutations, HMR state, etc.
///
/// During Phase 3, fields will be migrated here from Rolldown's `NormalModule`
/// and `EcmaView`.
#[derive(Debug, Default, Clone)]
pub struct ModulePayload {
  // Phase 3 will populate this with fields from EcmaView/NormalModule:
  // - source text
  // - AST (EcmaView minus link-relevant fields)
  // - ModuleId, StableModuleId, debug_id, repr_name
  // - sourcemap_chain
  // - mutations (render state)
  // - HMR info
  // - stmt_infos
  // - importers/dynamic_importers/imported_ids/dynamically_imported_ids
  // - module_type
  // - etc.
}
