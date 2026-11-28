use arcstr::ArcStr;
use json_escape_simd::escape;
use oxc_index::IndexVec;
use rolldown_common::{
  HmrSession, ImportKind, ImportRecordIdx, ImportRecordMeta, Module, ModuleIdx, ModuleTable,
  NormalModule, RUNTIME_MODULE_KEY, Specifier, lazy_proxy_target,
};
use rustc_hash::{FxHashMap, FxHashSet};

/// The session values a full build's rows read, taken before chunks render in parallel.
///
/// The rows do not carry `generation`: an edit and its undo must give the same output bytes, so
/// the browser can reuse the code it compiled before. The consumer tells the page the
/// generation from `BundleOutput::hmr_generation`.
pub struct DevRowsView {
  epoch: u32,
  generation: u32,
  nums: IndexVec<ModuleIdx, u32>,
}

impl DevRowsView {
  /// Gives a number to every module that can be named in a row.
  pub fn new(session: &mut HmrSession, module_table: &ModuleTable) -> Self {
    // The module table order depends on the parallel scan. New numbers follow the stable id
    // order, so the same build gives the same numbers.
    let mut new_ids = module_table
      .modules
      .iter()
      .filter_map(|module| match module {
        Module::Normal(module) if is_named_in_rows(module) => Some(module),
        _ => None,
      })
      .filter(|module| session.num_of(row_id(module)).is_none())
      .map(|module| match lazy_proxy_target(module.stable_id.as_str()) {
        Some(id) => ArcStr::from(id),
        None => module.stable_id.as_arc_str().clone(),
      })
      .collect::<Vec<_>>();
    new_ids.sort_unstable();
    new_ids.dedup();
    for id in &new_ids {
      session.assign_num(id);
    }
    let nums = module_table
      .modules
      .iter()
      .map(|module| match module {
        Module::Normal(module) if is_named_in_rows(module) => {
          session.num_of(row_id(module)).expect("every module named in rows got a number above")
        }
        _ => u32::MAX,
      })
      .collect();
    Self { epoch: session.epoch(), generation: session.generation(), nums }
  }

  /// The generation the output of this build is rendered at.
  pub fn generation(&self) -> u32 {
    self.generation
  }
}

/// Gives a number to every module that the rows of a payload carrying `carried_modules` name:
/// the carried modules and the targets of their edges.
#[cfg_attr(not(feature = "experimental"), allow(dead_code))]
pub fn assign_payload_nums(
  session: &mut HmrSession,
  module_table: &ModuleTable,
  carried_modules: impl IntoIterator<Item = ModuleIdx>,
) {
  for idx in carried_modules {
    let Module::Normal(module) = &module_table.modules[idx] else { continue };
    if !has_row(module) {
      continue;
    }
    session.assign_num(module.stable_id.as_arc_str());
    for record in &module.import_records {
      if !record.kind.is_static() && !record.kind.is_dynamic() {
        continue;
      }
      let Some(target_idx) = record.resolved_module else { continue };
      let Module::Normal(target) = &module_table.modules[target_idx] else { continue };
      if !is_named_in_rows(target) {
        continue;
      }
      match lazy_proxy_target(target.stable_id.as_str()) {
        Some(id) if session.num_of(id).is_none() => {
          session.assign_num(&ArcStr::from(id));
        }
        Some(_) => {}
        None => {
          session.assign_num(target.stable_id.as_arc_str());
        }
      }
    }
  }
}

/// Where a row set reads its module numbers and epoch.
#[derive(Clone, Copy)]
pub enum RowsSession<'a> {
  /// Full-build chunks. Their rows carry the epoch and call `__rolldown_runtime__.registerGraph`.
  Build(&'a DevRowsView),
  /// Patches and lazy chunks. `assign_payload_nums` numbered every module the rows name. Their
  /// rows call `__rolldown_payload__.registerGraph`, so the payload's generation applies to
  /// them, and they carry no epoch: the request already proved it.
  #[cfg_attr(not(feature = "experimental"), allow(dead_code))]
  Payload(&'a HmrSession),
}

impl RowsSession<'_> {
  /// The number of the row id of `module` (see `row_id`).
  fn num(self, module: &NormalModule) -> u32 {
    match self {
      Self::Build(view) => view.nums[module.idx],
      Self::Payload(session) => session
        .num_of(row_id(module))
        .expect("a payload gives its modules a number before it renders rows"),
    }
  }
}

/// A lazy proxy and the runtime module get no row.
fn has_row(module: &NormalModule) -> bool {
  is_named_in_rows(module) && lazy_proxy_target(module.stable_id.as_str()).is_none()
}

/// The runtime module appears in no row. A lazy proxy appears as an edge target, under the id
/// of the module it stands for (`row_id`).
fn is_named_in_rows(module: &NormalModule) -> bool {
  module.id.as_str() != RUNTIME_MODULE_KEY
}

fn row_id(module: &NormalModule) -> &str {
  let stable_id = module.stable_id.as_str();
  lazy_proxy_target(stable_id).unwrap_or(stable_id)
}

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

/// `bindings[i]` for one module: per static edge in `out_edges`, the names read through it
/// when the target calls `import.meta.hot.acceptExports`, else `None`.
fn edge_bindings<'a>(
  module_table: &ModuleTable,
  module: &'a NormalModule,
  out_edges: &[usize],
  id_to_index: &FxHashMap<&str, usize>,
) -> Vec<Option<Vec<&'a str>>> {
  let names_by_record = static_import_bindings(module);
  let mut out_bindings: Vec<Option<Vec<&str>>> = vec![None; out_edges.len()];
  for (record_idx, record) in module.import_records.iter_enumerated() {
    if !record.kind.is_static() {
      continue;
    }
    let Some(target_idx) = record.resolved_module else { continue };
    let Module::Normal(target) = &module_table.modules[target_idx] else { continue };
    if !target.is_hmr_partially_accepting_module() {
      continue;
    }
    let Some(target_pos) = id_to_index.get(target.stable_id.as_str()) else { continue };
    let Some(edge_pos) = out_edges.iter().position(|pos| pos == target_pos) else { continue };
    let names = out_bindings[edge_pos].get_or_insert_with(Vec::new);
    for name in names_by_record.get(&record_idx).into_iter().flatten() {
      if !names.contains(name) {
        names.push(name);
      }
    }
  }
  out_bindings
}

/// Renders the compiler-emitted `registerGraph(...)` statement for the rows of
/// `carried_modules` — module-graph topology (static + dynamic edges).
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
/// `import.meta.hot.acceptExports`; other entries are `null`. `bindings` is an object keyed
/// by row, holding only rows with such an edge, and the key is left out when no row has
/// one. The runtime reads a missing row or entry as `"*"`.
///
/// `dynamicEdges` has the same sparse shape: an object keyed by row, holding only rows with
/// at least one `import()` edge, and left out when no row has one. A missing row means no
/// dynamic edges. `edges` stays a dense array: most modules import something, and an empty
/// row (`[]`) is shorter than a row key.
///
/// `nums[i]` is the session number of `ids[i]`, for every id (dense): a client can name a
/// foreign edge target in a request before any row of it arrives. The client sends sets of
/// modules back as sets of these numbers (`HmrNumSet`). `epoch` is the engine's epoch, on
/// full-build rows only.
///
/// `entries` lists the rows whose module is in `entry_modules`, and is left out when no row
/// is one.
///
/// A lazy proxy gets no row, and an `import()` edge into a proxy points at the module the
/// proxy stands for. Nothing runs under a proxy id in the client (`requestLazy` runs the real
/// module), so a proxy row would leave the real module with no importer that ran, and the
/// client boundary walk would stop there. See internal-docs/lazy-compilation/implementation.md.
///
/// `ids[0, local_count)` are the carried modules in input order; `ids[local_count, ..)` are
/// foreign edge targets interned on first use. Ids are interned by stable id, not by
/// `ModuleIdx`: the module behind an unfetched proxy is not in `module_table` yet. Returns
/// `None` when the payload carries no rows.
pub fn render_register_graph_source(
  module_table: &ModuleTable,
  carried_modules: impl IntoIterator<Item = ModuleIdx>,
  entry_modules: &FxHashSet<ModuleIdx>,
  session: RowsSession<'_>,
) -> Option<String> {
  let mut ids: Vec<&str> = Vec::new();
  // `id_nums[i]` is the session number of `ids[i]`.
  let mut id_nums: Vec<u32> = Vec::new();
  let mut id_to_index: FxHashMap<&str, usize> = FxHashMap::default();
  // `local_modules[i]` is the module of row `i`.
  let mut local_modules: Vec<ModuleIdx> = Vec::new();

  for idx in carried_modules {
    let Module::Normal(module) = &module_table.modules[idx] else { continue };
    if !has_row(module) {
      continue;
    }
    let stable_id = module.stable_id.as_str();
    id_to_index.entry(stable_id).or_insert_with(|| {
      ids.push(stable_id);
      id_nums.push(session.num(module));
      local_modules.push(idx);
      ids.len() - 1
    });
  }
  let local_count = ids.len();
  if local_count == 0 {
    return None;
  }

  let mut edges: Vec<Vec<usize>> = Vec::with_capacity(local_count);
  // Sparse `(row, bindings)`: few modules import a module that calls `acceptExports`, and a
  // chunk's prelude lists all of its modules, so rows without such an edge are not emitted.
  let mut bindings: Vec<(usize, Vec<Option<Vec<&str>>>)> = Vec::new();
  // Sparse `(row, dynamic edges)` for the same reason: few modules call `import()`.
  let mut dynamic_edges: Vec<(usize, Vec<usize>)> = Vec::new();
  // Reused across modules: dedup import records targeting the same module without a
  // linear rescan of the edge list per record (quadratic for high-fan-out modules).
  let mut seen_static = FxHashSet::default();
  let mut seen_dynamic = FxHashSet::default();
  for (i, &module_idx) in local_modules.iter().enumerate() {
    let Module::Normal(module) = &module_table.modules[module_idx] else {
      unreachable!("carried rows are filtered to normal modules above");
    };
    seen_static.clear();
    seen_dynamic.clear();
    let mut needs_bindings = false;
    let mut out_edges = Vec::new();
    let mut dyn_out_edges = Vec::new();
    for record in &module.import_records {
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
      let target_id = target.stable_id.as_str();
      let target_id = lazy_proxy_target(target_id).unwrap_or(target_id);
      let target_pos = *id_to_index.entry(target_id).or_insert_with(|| {
        ids.push(target_id);
        id_nums.push(session.num(target));
        ids.len() - 1
      });
      if record.kind.is_static() {
        if seen_static.insert(target_pos) {
          out_edges.push(target_pos);
        }
        needs_bindings |= target.is_hmr_partially_accepting_module();
      } else if seen_dynamic.insert(target_pos) {
        dyn_out_edges.push(target_pos);
      }
    }
    if needs_bindings {
      bindings.push((i, edge_bindings(module_table, module, &out_edges, &id_to_index)));
    }
    edges.push(out_edges);
    if !dyn_out_edges.is_empty() {
      dynamic_edges.push((i, dyn_out_edges));
    }
  }

  let mut source = String::with_capacity(ids.len() * 32);
  source.push_str(match session {
    RowsSession::Build(_) => "__rolldown_runtime__.registerGraph({ids:[",
    RowsSession::Payload(_) => "__rolldown_payload__.registerGraph({ids:[",
  });
  for (i, id) in ids.iter().enumerate() {
    if i > 0 {
      source.push(',');
    }
    source.push_str(&escape(id));
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
  source.push(']');
  if !bindings.is_empty() {
    source.push_str(",bindings:{");
    for (n, (row, out_bindings)) in bindings.iter().enumerate() {
      if n > 0 {
        source.push(',');
      }
      source.push_str(itoa::Buffer::new().format(*row));
      source.push_str(":[");
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
    source.push('}');
  }
  if !dynamic_edges.is_empty() {
    source.push_str(",dynamicEdges:{");
    for (n, (row, out_edges)) in dynamic_edges.iter().enumerate() {
      if n > 0 {
        source.push(',');
      }
      source.push_str(itoa::Buffer::new().format(*row));
      source.push_str(":[");
      for (j, target_pos) in out_edges.iter().enumerate() {
        if j > 0 {
          source.push(',');
        }
        source.push_str(itoa::Buffer::new().format(*target_pos));
      }
      source.push(']');
    }
    source.push('}');
  }
  source.push_str(",nums:[");
  for (i, num) in id_nums.iter().enumerate() {
    if i > 0 {
      source.push(',');
    }
    source.push_str(itoa::Buffer::new().format(*num));
  }
  source.push(']');
  if let RowsSession::Build(view) = session {
    source.push_str(",epoch:");
    source.push_str(itoa::Buffer::new().format(view.epoch));
  }
  let mut entry_rows =
    local_modules.iter().enumerate().filter(|(_, idx)| entry_modules.contains(idx));
  if let Some((first_row, _)) = entry_rows.next() {
    source.push_str(",entries:[");
    source.push_str(itoa::Buffer::new().format(first_row));
    for (row, _) in entry_rows {
      source.push(',');
      source.push_str(itoa::Buffer::new().format(row));
    }
    source.push(']');
  }
  source.push_str("});");

  tracing::debug!(
    target: "hmr",
    "registerGraph manifest: {} bytes, {} carried modules, {} interned ids",
    source.len(),
    local_count,
    ids.len(),
  );

  Some(source)
}
