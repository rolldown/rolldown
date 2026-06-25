//! SHADOW-MODE init-obligation computation (migration step 2 of the strictExecutionOrder
//! rewrite — see `internal-docs/linking/strict-execution-order-rewrite-proposal.md`).
//!
//! This pass computes the candidate `InitBefore` obligations under the proposed two-obligation
//! model and contrasts them with what the *current* model emits. It is **purely diagnostic**:
//! nothing consumes its output, so the bundle is byte-for-byte unchanged. The whole pass is
//! gated behind the `ROLLDOWN_DUMP_INIT_OBLIGATIONS` env var so it has zero cost on normal
//! builds; it exists to validate the model on real fixtures (notably #9961) before the model
//! is switched on (step 4).

use rolldown_common::{ImportKind, ImportRecordMeta, Module, ModuleIdx, Specifier, WrapKind};
use rustc_hash::FxHashMap;

use super::LinkStage;

/// Why an `init_*()` must run before a given site. See proposal §2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitReason {
  /// A *used* imported binding whose canonical value lives in the target's wrapped closure.
  /// Target is the canonical owner — never the syntactic import source. (The #9961 fix.)
  BindingInit,
  /// The target's namespace value is observed (`import * as ns`, dynamic `export *`).
  /// Never waived by side-effect metadata.
  NamespaceInit,
  /// The target has a non-waived top-level side effect that must run in order.
  Ordering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InitObligation {
  target: ModuleIdx,
  reason: InitReason,
}

fn push_obligation(list: &mut Vec<InitObligation>, target: ModuleIdx, reason: InitReason) {
  if !list.iter().any(|o| o.target == target && o.reason == reason) {
    list.push(InitObligation { target, reason });
  }
}

impl LinkStage<'_> {
  /// Compute candidate `InitBefore` obligations (shadow mode). No-op unless
  /// `ROLLDOWN_DUMP_INIT_OBLIGATIONS` is set; never feeds any consumer.
  pub(super) fn compute_init_obligations(&self) {
    if std::env::var_os("ROLLDOWN_DUMP_INIT_OBLIGATIONS").is_none() {
      return;
    }
    if !self.options.is_strict_execution_order_enabled() {
      return;
    }

    let mut new_model: FxHashMap<ModuleIdx, Vec<InitObligation>> = FxHashMap::default();
    let mut current_model: FxHashMap<ModuleIdx, Vec<ModuleIdx>> = FxHashMap::default();

    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      if !self.metas[module.idx].is_included {
        continue;
      }

      let mut obligations: Vec<InitObligation> = Vec::new();

      // --- new model -------------------------------------------------------------------
      // BindingInit: per *used* named import, target = canonical owner (resolved through
      // re-export barrels), NOT the syntactic import source. Unused named imports
      // (`import { used, unused }` where only `used` is read) are skipped — per-binding
      // activation via `used_symbol_refs`.
      for named_import in module.named_imports.values() {
        match &named_import.imported {
          Specifier::Star => {
            // `import * as ns from X` observes X's namespace value.
            if let Some(importee_idx) =
              module.import_records[named_import.record_idx].resolved_module
              && self.is_wrapped_esm_included(importee_idx)
            {
              push_obligation(&mut obligations, importee_idx, InitReason::NamespaceInit);
            }
          }
          Specifier::Literal(_) => {
            if !self.used_symbol_refs.contains(&named_import.imported_as) {
              continue;
            }
            let canonical =
              self.symbols.canonical_ref_resolving_namespace(named_import.imported_as);
            if self.is_wrapped_esm_included(canonical.owner) {
              push_obligation(&mut obligations, canonical.owner, InitReason::BindingInit);
            }
          }
        }
      }

      // NamespaceInit (dynamic `export *`) + BindingInit (static star members) + Ordering.
      for rec in &module.import_records {
        if rec.kind != ImportKind::Import {
          continue;
        }
        let Some(importee_idx) = rec.resolved_module else { continue };
        let Module::Normal(importee) = &self.module_table[importee_idx] else { continue };

        if rec.meta.contains(ImportRecordMeta::IsExportStar) {
          if self.metas[importee_idx].has_dynamic_exports
            && self.is_wrapped_esm_included(importee_idx)
          {
            push_obligation(&mut obligations, importee_idx, InitReason::NamespaceInit);
          }
          for resolved_export in self.metas[importee_idx].resolved_exports.values() {
            let canonical =
              self.symbols.canonical_ref_resolving_namespace(resolved_export.symbol_ref);
            if self.is_wrapped_esm_included(canonical.owner) {
              push_obligation(&mut obligations, canonical.owner, InitReason::BindingInit);
            }
          }
        }

        // Ordering: only a *non-waived* top-level side effect earns an ordering obligation.
        // `sideEffects:false` waives it — this is exactly why #9961's `core` gets none.
        if importee.side_effects.has_side_effects() && self.is_wrapped_esm_included(importee_idx) {
          push_obligation(&mut obligations, importee_idx, InitReason::Ordering);
        }
      }

      // --- current model (for contrast) ------------------------------------------------
      // Approximates today's emission: `reference_needed_symbols` attaches `init_<importee>`
      // for every static import of a wrapped-ESM importee — keyed on the *syntactic* import
      // source, which is what over-includes side-effect-free barrels.
      let mut current: Vec<ModuleIdx> = Vec::new();
      for rec in &module.import_records {
        if rec.kind != ImportKind::Import {
          continue;
        }
        let Some(importee_idx) = rec.resolved_module else { continue };
        if self.is_wrapped_esm_included(importee_idx) && !current.contains(&importee_idx) {
          current.push(importee_idx);
        }
      }

      if !obligations.is_empty() {
        new_model.insert(module.idx, obligations);
      }
      if !current.is_empty() {
        current_model.insert(module.idx, current);
      }
    }

    self.dump_init_obligations(&new_model, &current_model);
  }

  fn is_wrapped_esm_included(&self, idx: ModuleIdx) -> bool {
    let meta = &self.metas[idx];
    matches!(meta.wrap_kind(), WrapKind::Esm) && meta.is_included && meta.wrapper_ref.is_some()
  }

  fn debug_id(&self, idx: ModuleIdx) -> String {
    self.module_table.modules[idx]
      .as_normal()
      .map_or_else(|| "<external>".to_string(), |m| m.stable_id.to_string())
  }

  // Dev-only shadow diagnostic gated behind `ROLLDOWN_DUMP_INIT_OBLIGATIONS`; writes a
  // contrast table to stderr. `eprintln!` is intentional here (this is a debugging dump, not
  // production output), so the `print_stderr` lint is waived for this function only.
  #[expect(clippy::print_stderr)]
  fn dump_init_obligations(
    &self,
    new_model: &FxHashMap<ModuleIdx, Vec<InitObligation>>,
    current_model: &FxHashMap<ModuleIdx, Vec<ModuleIdx>>,
  ) {
    use std::collections::BTreeSet;

    // Graph-level view: which modules does *some* importer init under each model.
    let new_global: BTreeSet<ModuleIdx> =
      new_model.values().flat_map(|v| v.iter().map(|o| o.target)).collect();
    let current_global: BTreeSet<ModuleIdx> = current_model.values().flatten().copied().collect();

    // `dropped` = init'd by the current model but given NO obligation anywhere under the new
    // model. Sound only if the module is safely prunable: no non-waived side effect (Ordering
    // would have targeted it) and it owns no used binding (BindingInit would have, so it can't
    // be in `dropped` if it did). So a dropped module that is **side-effectful** is a RED FLAG —
    // the new model would skip an init that must run. Everything else dropped is a pure
    // barrel/proxy correctly bypassed in favour of canonical owners (see `added`).
    let dropped: Vec<ModuleIdx> = current_global.difference(&new_global).copied().collect();
    let risky: Vec<ModuleIdx> = dropped
      .iter()
      .copied()
      .filter(|idx| {
        self.module_table.modules[*idx]
          .as_normal()
          .is_some_and(|m| m.side_effects.has_side_effects())
      })
      .collect();
    let added: Vec<ModuleIdx> = new_global.difference(&current_global).copied().collect();

    let fmt =
      |ids: &[ModuleIdx]| ids.iter().map(|idx| self.debug_id(*idx)).collect::<Vec<_>>().join(", ");
    eprintln!(
      "[init-obligations] dropped={} added={} RISKY={} | dropped=[{}] added=[{}] risky=[{}]",
      dropped.len(),
      added.len(),
      risky.len(),
      fmt(&dropped),
      fmt(&added),
      fmt(&risky),
    );

    // Only spell out per-importer detail when there's a red flag to investigate.
    if risky.is_empty() {
      return;
    }
    let mut importer_ids: Vec<ModuleIdx> =
      new_model.keys().chain(current_model.keys()).copied().collect();
    importer_ids.sort_unstable();
    importer_ids.dedup();
    importer_ids.sort_by_key(|idx| self.debug_id(*idx));
    for importer in importer_ids {
      let current: BTreeSet<ModuleIdx> =
        current_model.get(&importer).map(|v| v.iter().copied().collect()).unwrap_or_default();
      let new = new_model.get(&importer).map(Vec::as_slice).unwrap_or(&[]);
      let new_targets: BTreeSet<ModuleIdx> = new.iter().map(|o| o.target).collect();
      if current == new_targets {
        continue;
      }
      let current_only: Vec<String> =
        current.difference(&new_targets).map(|idx| self.debug_id(*idx)).collect();
      let new_only: Vec<String> = new
        .iter()
        .filter(|o| !current.contains(&o.target))
        .map(|o| format!("{}({:?})", self.debug_id(o.target), o.reason))
        .collect();
      eprintln!(
        "    {} | dropped=[{}] added=[{}]",
        self.debug_id(importer),
        current_only.join(", "),
        new_only.join(", ")
      );
    }
  }
}
