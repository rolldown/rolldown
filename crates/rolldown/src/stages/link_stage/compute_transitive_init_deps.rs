use oxc_index::IndexVec;
use rolldown_common::{ImportKind, Module, ModuleIdx, ModuleTable, WrapKind};
use rolldown_utils::IndexBitSet;
use rustc_hash::FxHashSet;

use crate::types::linking_metadata::LinkingMetadataVec;

/// Compute transitive reachability of wrapped ESM dependencies for each module.
///
/// For each module, `result[m]` contains the set of wrapped ESM modules transitively reachable
/// through static imports. This is used to perform transitive reduction on init calls —
/// if `a` imports `b` and `c`, and `b` already imports `c`, then `init_c()` in `a` is redundant.
pub fn compute_transitive_init_deps(
  module_table: &ModuleTable,
  metas: &LinkingMetadataVec,
) -> IndexVec<ModuleIdx, IndexBitSet<ModuleIdx>> {
  let module_count = module_table.modules.len();

  // Build adjacency list: for each module, collect direct deps where wrap_kind == Esm
  let adj: IndexVec<ModuleIdx, Vec<ModuleIdx>> = module_table
    .modules
    .iter()
    .map(|module| {
      let mut deps = FxHashSet::default();
      for rec in module.import_records() {
        if rec.kind != ImportKind::Import {
          continue;
        }
        let Some(resolved) = rec.resolved_module else {
          continue;
        };
        if metas[resolved].wrap_kind() == WrapKind::Esm {
          deps.insert(resolved);
        }
      }
      deps.into_iter().collect()
    })
    .collect();

  let mut cache: IndexVec<ModuleIdx, Option<IndexBitSet<ModuleIdx>>> =
    module_table.modules.iter().map(|_| None).collect();
  let mut visiting = IndexBitSet::new(module_count);

  for idx in module_table.modules.iter().map(Module::idx) {
    if cache[idx].is_none() {
      dfs(idx, &adj, &mut cache, &mut visiting, module_count);
    }
  }

  cache.into_iter().map(Option::unwrap_or_default).collect()
}

fn dfs(
  module_idx: ModuleIdx,
  adj: &IndexVec<ModuleIdx, Vec<ModuleIdx>>,
  cache: &mut IndexVec<ModuleIdx, Option<IndexBitSet<ModuleIdx>>>,
  visiting: &mut IndexBitSet<ModuleIdx>,
  module_count: usize,
) -> IndexBitSet<ModuleIdx> {
  if let Some(cached) = &cache[module_idx] {
    return cached.clone();
  }
  if visiting.has_bit(module_idx) {
    // Cycle guard: return empty set conservatively
    return IndexBitSet::default();
  }
  visiting.set_bit(module_idx);

  let mut reach = IndexBitSet::new(module_count);
  // Clone adj list to avoid borrow conflict with cache
  let deps = adj[module_idx].clone();
  for dep in deps {
    reach.set_bit(dep);
    let dep_reach = dfs(dep, adj, cache, visiting, module_count);
    if !dep_reach.is_empty() {
      reach.union(&dep_reach);
    }
  }

  visiting.clear_bit(module_idx);
  cache[module_idx] = Some(reach.clone());
  reach
}
