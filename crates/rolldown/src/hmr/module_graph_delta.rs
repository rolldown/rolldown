use json_escape_simd::escape;
use rolldown_common::{Module, ModuleIdx, ModuleTable, RUNTIME_MODULE_KEY};
use rustc_hash::{FxHashMap, FxHashSet};

/// Renders the compiler-emitted `__rolldown_runtime__.registerGraph(...)` prelude for a
/// payload carrying `carried_modules` — pure module-graph topology (static + dynamic edges).
///
/// The edge sets mirror the split `EcmaView::rebuild_importer_sets` maintains server-side:
/// `ImportKind::is_static()` records go to `edges`, `ImportKind::is_dynamic()` (`import()`)
/// records go to `dynamicEdges` — both resolving to normal modules. `HotAccept` and other
/// non-static/non-dynamic records contribute no edge, and the runtime module appears
/// nowhere (parity with its skipped HMR registration header).
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
  let mut dynamic_edges: Vec<Vec<usize>> = Vec::with_capacity(local_count);
  // Reused across modules: dedup import records targeting the same module without a
  // linear rescan of the edge list per record (quadratic for high-fan-out modules).
  let mut seen_static = FxHashSet::default();
  let mut seen_dynamic = FxHashSet::default();
  for i in 0..local_count {
    let Module::Normal(module) = &module_table.modules[ids[i]] else {
      unreachable!("carried rows are filtered to normal modules above");
    };
    seen_static.clear();
    seen_dynamic.clear();
    let mut out_edges = Vec::new();
    let mut dyn_out_edges = Vec::new();
    for record in &module.import_records {
      // Static edges and dynamic `import()` edges both ship; the runtime keeps them in
      // separate reverse indexes but unions them in `getImporters`. Other non-static
      // records (HotAccept, new URL, css url) are not import edges and are skipped.
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
        if seen_static.insert(target_pos) {
          out_edges.push(target_pos);
        }
      } else if seen_dynamic.insert(target_pos) {
        dyn_out_edges.push(target_pos);
      }
    }
    edges.push(out_edges);
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
