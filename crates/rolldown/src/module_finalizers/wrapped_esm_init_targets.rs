use rolldown_common::{
  ConcatenateWrappedModuleKind, ImportRecordIdx, ImportRecordMeta, IndexModules, ModuleIdx,
  NormalModule, Specifier, SymbolRef, SymbolRefDb, WrapKind,
};
use rustc_hash::FxHashSet;

use crate::types::linking_metadata::{LinkingMetadata, LinkingMetadataVec};

pub struct WrappedEsmInitTargetContext<'a> {
  pub importer: &'a NormalModule,
  pub importer_meta: &'a LinkingMetadata,
  pub modules: &'a IndexModules,
  pub metas: &'a LinkingMetadataVec,
  pub symbol_db: &'a SymbolRefDb,
}

/// Resolve the wrapped ESM modules initialized by one static import/re-export record.
///
/// Besides direct wrapped importees, this follows canonical symbols through non-included,
/// unwrapped forwarding modules. Callers supply the wrapper-reachability check because the
/// finalizer observes deconflicted canonical names while earlier diagnostic passes observe final
/// cross-chunk links. They also supply the same-chunk forwarding ownership check: an included
/// unwrapped forwarding module executes in the importer's chunk and therefore owns its downstream
/// initialization instead of requiring a synthesized init at the importer.
///
/// See `internal-docs/linking/reference-needed-symbols/implementation.md`.
pub fn collect_wrapped_esm_init_targets_for_import_record(
  ctx: &WrappedEsmInitTargetContext<'_>,
  rec_idx: ImportRecordIdx,
  wrapper_is_reachable: impl Fn(SymbolRef) -> bool,
  forwarding_module_owns_initialization: impl Fn(ModuleIdx) -> bool,
) -> Vec<ModuleIdx> {
  let mut targets = Vec::new();
  let record = &ctx.importer.import_records[rec_idx];
  let Some(importee_idx) = record.resolved_module else { return targets };
  let importee_meta = &ctx.metas[importee_idx];

  if matches!(importee_meta.wrap_kind(), WrapKind::None)
    && importee_meta.is_included
    && forwarding_module_owns_initialization(importee_idx)
  {
    return targets;
  }

  if wrapped_esm_target_is_reachable(importee_meta, &wrapper_is_reachable) {
    targets.push(importee_idx);
    return targets;
  }

  let mut visited_symbols = FxHashSet::default();
  if record.meta.contains(ImportRecordMeta::IsExportStar) {
    for resolved_export in importee_meta.resolved_exports.values() {
      add_wrapped_esm_init_target_for_symbol(
        ctx,
        resolved_export.symbol_ref,
        &wrapper_is_reachable,
        &mut targets,
        &mut visited_symbols,
      );
    }
    return targets;
  }

  for named_import in ctx.importer.named_imports.values().filter(|item| item.record_idx == rec_idx)
  {
    match &named_import.imported {
      Specifier::Star => {
        for resolved_export in importee_meta.resolved_exports.values() {
          add_wrapped_esm_init_target_for_symbol(
            ctx,
            resolved_export.symbol_ref,
            &wrapper_is_reachable,
            &mut targets,
            &mut visited_symbols,
          );
        }
      }
      Specifier::Literal(name) => {
        let symbol_ref = importee_meta
          .resolved_exports
          .get(name)
          .map_or(named_import.imported_as, |resolved_export| resolved_export.symbol_ref);
        add_wrapped_esm_init_target_for_symbol(
          ctx,
          symbol_ref,
          &wrapper_is_reachable,
          &mut targets,
          &mut visited_symbols,
        );
      }
    }
  }

  targets
}

fn add_wrapped_esm_init_target_for_symbol(
  ctx: &WrappedEsmInitTargetContext<'_>,
  symbol_ref: SymbolRef,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
  targets: &mut Vec<ModuleIdx>,
  visited_symbols: &mut FxHashSet<SymbolRef>,
) {
  let canonical_ref = ctx.symbol_db.canonical_ref_resolving_namespace(symbol_ref);
  if !visited_symbols.insert(canonical_ref) {
    return;
  }
  let meta = &ctx.metas[canonical_ref.owner];
  if wrapped_esm_target_is_reachable(meta, wrapper_is_reachable) {
    targets.push(canonical_ref.owner);
    return;
  }

  let Some(module) = ctx.modules[canonical_ref.owner].as_normal() else {
    return;
  };
  if module.namespace_object_ref != canonical_ref
    || meta.is_included
    || !ctx.importer_meta.hoist_esm_wrapper
  {
    return;
  }

  for resolved_export in meta.resolved_exports.values() {
    add_wrapped_esm_init_target_for_symbol(
      ctx,
      resolved_export.symbol_ref,
      wrapper_is_reachable,
      targets,
      visited_symbols,
    );
  }
}

fn wrapped_esm_target_is_reachable(
  meta: &LinkingMetadata,
  wrapper_is_reachable: &impl Fn(SymbolRef) -> bool,
) -> bool {
  matches!(meta.wrap_kind(), WrapKind::Esm)
    && meta.is_included
    && meta.wrapper_ref.is_some_and(wrapper_is_reachable)
    && !matches!(meta.concatenated_wrapped_module_kind, ConcatenateWrappedModuleKind::Inner)
}
