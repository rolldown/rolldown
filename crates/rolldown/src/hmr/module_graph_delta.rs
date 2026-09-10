use json_escape_simd::escape;
use oxc_str::CompactStr;
use rolldown_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, Module, ModuleIdx, ModuleTable, NormalModule,
  RUNTIME_MODULE_KEY, Specifier,
};
use rustc_hash::{FxHashMap, FxHashSet};

/// Stands for every export: `import * as ns`, `export * from`, `require()`, non-JS records.
const ANY_BINDING: &str = "*";

/// The export names `module` reads through each static import record, keyed by the record.
/// `"*"` for a whole-namespace read, an empty list for a side-effect-only import.
pub fn static_import_bindings(module: &NormalModule) -> FxHashMap<ImportRecordIdx, Vec<&str>> {
  let mut names_by_record: FxHashMap<ImportRecordIdx, Vec<&str>> = FxHashMap::default();
  for (record_idx, record) in module.import_records.iter_enumerated() {
    if !record.kind.is_static() {
      continue;
    }
    let names = names_by_record.entry(record_idx).or_default();
    if record.kind != ImportKind::Import || record.meta.contains(ImportRecordMeta::IsExportStar) {
      names.push(ANY_BINDING);
    }
  }
  for named_import in module.named_imports.values() {
    let Some(names) = names_by_record.get_mut(&named_import.record_idx) else { continue };
    let name = match &named_import.imported {
      Specifier::Star => ANY_BINDING,
      Specifier::Literal(name) => name.as_str(),
    };
    if !names.contains(&name) {
      names.push(name);
    }
  }
  names_by_record
}

/// Whether every export name `importer` reads from `module` is in `accepted`.
pub fn imports_only_accepted_exports(
  importer: &NormalModule,
  module_idx: ModuleIdx,
  accepted: &FxHashSet<CompactStr>,
) -> bool {
  let bindings = static_import_bindings(importer);
  importer.import_records.iter_enumerated().all(|(record_idx, record)| {
    // a dynamic `import()` reads the whole namespace
    record.resolved_module != Some(module_idx)
      || bindings
        .get(&record_idx)
        .is_some_and(|names| names.iter().all(|name| accepted.contains(*name)))
  })
}

/// Renders the compiler-emitted `__rolldown_runtime__.registerGraph(...)` prelude for a
/// payload carrying `carried_modules` — module-graph topology (static + dynamic edges).
///
/// The edge sets mirror the split `EcmaView::rebuild_importer_sets` maintains server-side:
/// `ImportKind::is_static()` records go to `edges`, `ImportKind::is_dynamic()` (`import()`)
/// records go to `dynamicEdges` — both resolving to normal modules. `HotAccept` and other
/// non-static/non-dynamic records contribute no edge, and the runtime module appears
/// nowhere (parity with its skipped HMR registration header).
///
/// `bindings[i][j]` lists the export names `ids[i]` imports through static edge `edges[i][j]`
/// (re-exports included), `"*"` for anything that reads the whole namespace, and an empty
/// list for a side-effect-only import. It is only filled for edges into a module that calls
/// `import.meta.hot.acceptExports`; other entries are `null`, and the key is left out when
/// no edge qualifies. The runtime reads a missing entry as `"*"`.
///
/// `ids[0, local_count)` are the carried modules in input order; `ids[local_count, ..)` are
/// foreign edge targets interned on first use. Returns `None` when the payload carries no rows.
pub fn render_register_graph_source(
  module_table: &ModuleTable,
  carried_modules: impl IntoIterator<Item = ModuleIdx>,
) -> Option<String> {
  let mut ids: Vec<ModuleIdx> = Vec::new();
  let mut id_to_index: FxHashMap<ModuleIdx, usize> = FxHashMap::default();

  for idx in carried_modules {
    let Module::Normal(module) = &module_table.modules[idx] else { continue };
    if module.id.as_str() == RUNTIME_MODULE_KEY {
      continue;
    }
    id_to_index.entry(idx).or_insert_with(|| {
      ids.push(idx);
      ids.len() - 1
    });
  }
  let local_count = ids.len();
  if local_count == 0 {
    return None;
  }

  let mut edges: Vec<Vec<usize>> = Vec::with_capacity(local_count);
  let mut bindings: Vec<Vec<Option<Vec<&str>>>> = Vec::with_capacity(local_count);
  let mut any_bindings = false;
  let mut dynamic_edges: Vec<Vec<usize>> = Vec::with_capacity(local_count);
  // Reused across modules: dedup import records targeting the same module without a
  // linear rescan of the edge list per record (quadratic for high-fan-out modules).
  let mut static_edge_pos: FxHashMap<usize, usize> = FxHashMap::default();
  let mut seen_dynamic = FxHashSet::default();
  for i in 0..local_count {
    let Module::Normal(module) = &module_table.modules[ids[i]] else {
      unreachable!("carried rows are filtered to normal modules above");
    };
    static_edge_pos.clear();
    seen_dynamic.clear();
    let names_by_record = static_import_bindings(module);
    let mut out_edges = Vec::new();
    let mut out_bindings: Vec<Option<Vec<&str>>> = Vec::new();
    let mut dyn_out_edges = Vec::new();
    for (record_idx, record) in module.import_records.iter_enumerated() {
      // Static edges and dynamic `import()` edges both ship; the runtime keeps them in
      // separate reverse indexes but unions them in `getImporters`. `HotAccept` records
      // are not import edges and are skipped.
      if !record.kind.is_static() && !record.kind.is_dynamic() {
        continue;
      }
      let Some(target_idx) = record.resolved_module else { continue };
      let Module::Normal(target) = &module_table.modules[target_idx] else { continue };
      if target.id.as_str() == RUNTIME_MODULE_KEY {
        continue;
      }
      let target_pos = *id_to_index.entry(target_idx).or_insert_with(|| {
        ids.push(target_idx);
        ids.len() - 1
      });
      if record.kind.is_static() {
        let edge_pos = *static_edge_pos.entry(target_pos).or_insert_with(|| {
          out_edges.push(target_pos);
          out_bindings.push(None);
          out_edges.len() - 1
        });
        if target.is_hmr_partially_accepting_module() {
          let names = out_bindings[edge_pos].get_or_insert_with(Vec::new);
          for name in names_by_record.get(&record_idx).into_iter().flatten() {
            if !names.contains(name) {
              names.push(name);
            }
          }
          any_bindings = true;
        }
      } else if seen_dynamic.insert(target_pos) {
        dyn_out_edges.push(target_pos);
      }
    }
    edges.push(out_edges);
    bindings.push(out_bindings);
    dynamic_edges.push(dyn_out_edges);
  }

  let mut source = String::with_capacity(ids.len() * 32);
  source.push_str("__rolldown_runtime__.registerGraph({ids:[");
  for (i, idx) in ids.iter().enumerate() {
    if i > 0 {
      source.push(',');
    }
    source.push_str(&escape(module_table.modules[*idx].stable_id().as_str()));
  }
  source.push_str("],localCount:");
  source.push_str(itoa::Buffer::new().format(local_count));
  source.push_str(",edges:[");
  for (i, out_edges) in edges.iter().enumerate() {
    if i > 0 {
      source.push(',');
    }
    source.push('[');
    for (j, target_pos) in out_edges.iter().enumerate() {
      if j > 0 {
        source.push(',');
      }
      source.push_str(itoa::Buffer::new().format(*target_pos));
    }
    source.push(']');
  }
  if any_bindings {
    source.push_str("],bindings:[");
    for (i, out_bindings) in bindings.iter().enumerate() {
      if i > 0 {
        source.push(',');
      }
      source.push('[');
      // trailing `null`s are dropped: a missing entry reads the same as `null`
      let len = out_bindings.iter().rposition(Option::is_some).map_or(0, |pos| pos + 1);
      for (j, names) in out_bindings[..len].iter().enumerate() {
        if j > 0 {
          source.push(',');
        }
        match names {
          None => source.push_str("null"),
          Some(names) => {
            source.push('[');
            for (k, name) in names.iter().enumerate() {
              if k > 0 {
                source.push(',');
              }
              source.push_str(&escape(name));
            }
            source.push(']');
          }
        }
      }
      source.push(']');
    }
  }
  source.push_str("],dynamicEdges:[");
  for (i, out_edges) in dynamic_edges.iter().enumerate() {
    if i > 0 {
      source.push(',');
    }
    source.push('[');
    for (j, target_pos) in out_edges.iter().enumerate() {
      if j > 0 {
        source.push(',');
      }
      source.push_str(itoa::Buffer::new().format(*target_pos));
    }
    source.push(']');
  }
  source.push_str("]});");

  tracing::debug!(
    target: "hmr",
    "registerGraph manifest: {} bytes, {} carried modules, {} interned ids",
    source.len(),
    local_count,
    ids.len(),
  );

  Some(source)
}
