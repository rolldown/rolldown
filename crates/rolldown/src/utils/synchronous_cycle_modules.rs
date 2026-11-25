use rolldown_common::{ImportKind, IndexModules, ModuleIdx, ResolvedImportRecord};
use rustc_hash::FxHashSet;

pub fn synchronous_cycle_modules(
  modules: &IndexModules,
  include_record: impl Fn(ModuleIdx, &ResolvedImportRecord) -> bool,
) -> FxHashSet<ModuleIdx> {
  let mut graph = petgraph::prelude::DiGraphMap::<ModuleIdx, ()>::new();
  for module in modules.iter().filter_map(|module| module.as_normal()) {
    graph.add_node(module.idx);
    for rec in &module.import_records {
      if matches!(rec.kind, ImportKind::Import | ImportKind::Require)
        && include_record(module.idx, rec)
        && let Some(importee_idx) = rec.resolved_module
        && modules[importee_idx].is_normal()
      {
        graph.add_edge(module.idx, importee_idx, ());
      }
    }
  }

  let mut cyclic = FxHashSet::default();
  // Use iterative DFS even for deeply nested module graphs.
  for component in petgraph::algo::kosaraju_scc(&graph) {
    if component.len() > 1 || graph.contains_edge(component[0], component[0]) {
      cyclic.extend(component);
    }
  }
  cyclic
}
