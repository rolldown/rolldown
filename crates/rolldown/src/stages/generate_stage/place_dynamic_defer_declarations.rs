use rolldown_common::{ImportKind, Module, ModuleIdx, ModuleType, WrapKind};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::{GenerateStage, order_wrap_state::OrderWrapState};

impl GenerateStage<'_> {
  /// Puts the code of each `import.defer()` target of the same chunk, and of the modules of its
  /// deferred closure in that chunk, before the code of the other modules of the chunk (after
  /// the runtime module).
  ///
  /// The same-chunk form of `import.defer('./x')` reads `x_deferred` when it is called
  /// (`rewrite_dynamic_import_of_defer_target`). A callback can also read the namespace before
  /// the code of the chunk ends, for example during a top-level await pause. Without the move, a
  /// call that comes before the declarations of `x` reads `undefined`, and a read during a pause
  /// can reach a namespace object of the closure that does not exist yet.
  ///
  /// `wrap_import_defer_closures` wraps each module of the static closure. The code of a wrapped
  /// module has only declarations, so the move does not change when a module evaluates. A module
  /// with no wrapper does not move, except a JSON module, whose code has only data
  /// (`unwrap_json_module`). A module that comes before the first importer of the target in the
  /// chunk does not move either: its declarations already run before each call.
  ///
  /// Call it after `finalize_modules`. The order analyses and the finalizers read the order of a
  /// chunk as the order of execution.
  pub fn place_dynamic_defer_declarations(
    &self,
    chunk_graph: &mut ChunkGraph,
    order_state: &OrderWrapState,
  ) {
    if !self.options.experimental.is_import_defer_enabled() {
      return;
    }
    let module_table = &self.link_output.module_table;
    let metas = &self.link_output.metas;
    let runtime_idx = self.link_output.runtime.id();
    let has_only_declarations = |module_idx: ModuleIdx| {
      let Module::Normal(module) = &module_table[module_idx] else { return false };
      let meta = &metas[module_idx];
      matches!(meta.wrap_kind(), WrapKind::Cjs)
        || order_state.esm_init_target(module_idx, meta).is_some()
        || matches!(module.module_type, ModuleType::Json)
    };
    let ChunkGraph { chunk_table, module_to_chunk, .. } = chunk_graph;
    for (chunk_idx, chunk) in chunk_table.iter_mut_enumerated() {
      if self.inline_state.is_record(chunk_idx) {
        continue;
      }
      // The position of the first importer of each target, in the order of the chunk.
      let mut first_importer: FxHashMap<ModuleIdx, usize> = FxHashMap::default();
      for (position, &importer_idx) in chunk.modules.iter().enumerate() {
        let Module::Normal(importer) = &module_table[importer_idx] else { continue };
        for rec in &importer.import_records {
          if rec.kind != ImportKind::DynamicImport || !rec.is_defer_phase() {
            continue;
          }
          let Some(target) = rec.resolved_module else { continue };
          if metas[target].dynamic_defer_target && module_to_chunk[target] == Some(chunk_idx) {
            first_importer.entry(target).or_insert(position);
          }
        }
      }
      if first_importer.is_empty() {
        continue;
      }
      let positions: FxHashMap<ModuleIdx, usize> =
        chunk.modules.iter().enumerate().map(|(position, &idx)| (idx, position)).collect();
      let mut moved = FxHashSet::default();
      for (&target, &importer_position) in &first_importer {
        // The static closure of the target, in this chunk.
        let mut visited = FxHashSet::default();
        let mut stack = vec![target];
        while let Some(module_idx) = stack.pop() {
          if module_idx == runtime_idx || !visited.insert(module_idx) {
            continue;
          }
          let Some(&position) = positions.get(&module_idx) else { continue };
          let Module::Normal(module) = &module_table[module_idx] else { continue };
          if position > importer_position && has_only_declarations(module_idx) {
            moved.insert(module_idx);
          }
          stack.extend(
            module
              .import_records
              .iter()
              .filter(|rec| rec.kind.is_static())
              .filter_map(|rec| rec.resolved_module),
          );
        }
      }
      if moved.is_empty() {
        continue;
      }
      let mut modules = Vec::with_capacity(chunk.modules.len());
      modules.extend(chunk.modules.iter().copied().filter(|&idx| idx == runtime_idx));
      modules.extend(chunk.modules.iter().copied().filter(|idx| moved.contains(idx)));
      modules.extend(
        chunk.modules.iter().copied().filter(|&idx| idx != runtime_idx && !moved.contains(&idx)),
      );
      chunk.modules = modules;
    }
  }
}
