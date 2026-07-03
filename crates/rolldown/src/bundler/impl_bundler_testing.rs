use std::collections::BTreeSet;

use rolldown_common::{EntryPointKind, Module, ModuleIdx, ModuleTable};

use super::Bundler;

impl Bundler {
  /// Asserts the incremental scan state matches what a fresh full build of
  /// the same inputs produced. Modules only present on the incremental side
  /// are allowed: removed modules currently stay in the state (issue #7416).
  /// Their outgoing importer records stay with them, so importer sets are
  /// compared ignoring importers that do not exist in the fresh build; a
  /// stale importer entry from a module the fresh build *does* have is still
  /// a divergence.
  ///
  /// # Panics
  /// - if the states diverge, listing every difference
  pub fn assert_scan_state_parity_with(&self, fresh: &Bundler) {
    fn module_id_at(table: &ModuleTable, idx: ModuleIdx) -> String {
      table.modules.get(idx).map_or_else(|| format!("<unknown {idx:?}>"), |m| m.id().to_string())
    }

    if self.options().experimental.is_lazy_barrel_enabled() {
      // Lazy barrel loads import records on demand, so the loaded subset
      // depends on request history and legitimately differs per session.
      return;
    }

    let incremental_cache = &self.cache;
    let fresh_cache = &fresh.cache;
    assert!(
      incremental_cache.has_snapshot() && fresh_cache.has_snapshot(),
      "state parity needs both bundlers to have completed a build with incremental build enabled"
    );
    let incremental = incremental_cache.get_snapshot();
    let fresh_snapshot = fresh_cache.get_snapshot();

    let mut diffs = Vec::new();

    // Orphan modules keep their outgoing importer records (issue #7416);
    // importer entries pointing at modules unknown to the fresh build are
    // exactly those and get filtered out of the comparisons below.
    let fresh_module_ids = fresh_snapshot
      .module_table
      .modules
      .iter()
      .map(|module| module.id().to_string())
      .collect::<BTreeSet<_>>();

    for fresh_module in &fresh_snapshot.module_table.modules {
      let id = fresh_module.id();
      let Some(state) = incremental_cache.module_id_to_idx.get(id) else {
        diffs.push(format!("`{id}` is missing in the incremental state"));
        continue;
      };
      let Some(incremental_module) = incremental.module_table.modules.get(state.idx()) else {
        diffs.push(format!("`{id}` has an idx but no module in the incremental snapshot"));
        continue;
      };
      match (fresh_module, incremental_module) {
        (Module::Normal(fresh_module), Module::Normal(incremental_module)) => {
          let fresh_importers =
            fresh_module.importers.iter().map(ToString::to_string).collect::<BTreeSet<_>>();
          let incremental_importers = incremental_module
            .importers
            .iter()
            .map(ToString::to_string)
            .filter(|importer| fresh_module_ids.contains(importer))
            .collect::<BTreeSet<_>>();
          if fresh_importers != incremental_importers {
            diffs.push(format!(
              "`{id}` importers differ: fresh {fresh_importers:?}, incremental {incremental_importers:?}"
            ));
          }

          let fresh_dynamic_importers =
            fresh_module.dynamic_importers.iter().map(ToString::to_string).collect::<BTreeSet<_>>();
          let incremental_dynamic_importers = incremental_module
            .dynamic_importers
            .iter()
            .map(ToString::to_string)
            .filter(|importer| fresh_module_ids.contains(importer))
            .collect::<BTreeSet<_>>();
          if fresh_dynamic_importers != incremental_dynamic_importers {
            diffs.push(format!(
              "`{id}` dynamic importers differ: fresh {fresh_dynamic_importers:?}, incremental {incremental_dynamic_importers:?}"
            ));
          }

          let fresh_importer_ids = fresh_module
            .importers_idx
            .iter()
            .map(|idx| module_id_at(&fresh_snapshot.module_table, *idx))
            .collect::<BTreeSet<_>>();
          let incremental_importer_ids = incremental_module
            .importers_idx
            .iter()
            .map(|idx| module_id_at(&incremental.module_table, *idx))
            .filter(|importer| fresh_module_ids.contains(importer))
            .collect::<BTreeSet<_>>();
          if fresh_importer_ids != incremental_importer_ids {
            diffs.push(format!(
              "`{id}` importers_idx differ: fresh {fresh_importer_ids:?}, incremental {incremental_importer_ids:?}"
            ));
          }

          if fresh_module.import_records.len() == incremental_module.import_records.len() {
            for (index, (fresh_record, incremental_record)) in
              fresh_module.import_records.iter().zip(&incremental_module.import_records).enumerate()
            {
              let fresh_target = fresh_record
                .resolved_module
                .map(|idx| module_id_at(&fresh_snapshot.module_table, idx));
              let incremental_target = incremental_record
                .resolved_module
                .map(|idx| module_id_at(&incremental.module_table, idx));
              if fresh_record.kind != incremental_record.kind || fresh_target != incremental_target
              {
                diffs.push(format!(
                  "`{id}` import record {index} differs: fresh ({:?}, {fresh_target:?}), incremental ({:?}, {incremental_target:?})",
                  fresh_record.kind, incremental_record.kind
                ));
              }
            }
          } else {
            diffs.push(format!(
              "`{id}` import record count differs: fresh {}, incremental {}",
              fresh_module.import_records.len(),
              incremental_module.import_records.len()
            ));
          }
        }
        (Module::External(_), Module::External(_)) => {}
        _ => {
          diffs.push(format!("`{id}` module kind differs between fresh and incremental state"));
        }
      }
    }

    let incremental_entries = incremental
      .entry_points
      .iter()
      .map(|entry| {
        (format!("{:?}", entry.kind), module_id_at(&incremental.module_table, entry.idx))
      })
      .collect::<BTreeSet<_>>();
    let fresh_entries = fresh_snapshot
      .entry_points
      .iter()
      .map(|entry| {
        (format!("{:?}", entry.kind), module_id_at(&fresh_snapshot.module_table, entry.idx))
      })
      .collect::<BTreeSet<_>>();
    for entry in &fresh_entries {
      if !incremental_entries.contains(entry) {
        diffs.push(format!("entry point {entry:?} is missing in the incremental state"));
      }
    }
    // Dynamic import entries must match exactly while a live module imports
    // them. An entry kept only by orphans' dynamic import records is
    // tolerated: the cache keeps the rows so re-adding an import can revive
    // the entry, and `create_output` filters it from the build output. Other
    // kinds allow extras on the incremental side, e.g. entries of modules
    // that are no longer reachable (issue #7416, orphan cleanup).
    for entry in &incremental.entry_points {
      if entry.kind == EntryPointKind::DynamicImport {
        let key = (format!("{:?}", entry.kind), module_id_at(&incremental.module_table, entry.idx));
        if !fresh_entries.contains(&key) {
          let has_live_dynamic_importer = incremental
            .module_table
            .modules
            .get(entry.idx)
            .and_then(Module::as_normal)
            .is_some_and(|module| {
              module
                .dynamic_importers
                .iter()
                .any(|importer| fresh_module_ids.contains(&importer.to_string()))
            });
          if has_live_dynamic_importer {
            diffs
              .push(format!("dynamic import entry {key:?} only exists in the incremental state"));
          }
        }
      }
    }

    assert!(
      diffs.is_empty(),
      "incremental scan state diverges from a fresh full build:\n{}",
      diffs.join("\n")
    );
  }
}
