use rolldown_common::{ChunkIdx, ImportKind, Module, ModuleIdx, ModuleType, WrapKind};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::{GenerateStage, order_wrap_state::OrderWrapState};

/// The new order of the modules of each chunk that `GenerateStage::plan_dynamic_defer_declarations`
/// changes.
pub type DynamicDeferPlacements = Vec<(ChunkIdx, Vec<ModuleIdx>)>;

impl GenerateStage<'_> {
  /// Plans to put the code of each `import.defer()` target of the same chunk, and of the modules
  /// of its deferred closure in that chunk, before the code of the other modules of the chunk
  /// (after the runtime module). `place_dynamic_defer_declarations` applies the plan.
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
  /// A module that its chunk runs at load (`ChunkGraph::deferred_modules_run_at_load`) also has its
  /// init call. It moves, but its init call stays at its place: the last module before it that
  /// keeps its place runs the call at the end of its code (`ChunkGraph::run_at_load_anchors`).
  ///
  /// Call it before `finalize_modules`, which renders the init calls of the anchors. Call
  /// `place_dynamic_defer_declarations` after `finalize_modules`: the order analyses and the
  /// finalizers read the order of a chunk as the order of execution.
  /// See internal-docs/import-defer/implementation.md ("Declarations before the first call").
  pub fn plan_dynamic_defer_declarations(
    &self,
    chunk_graph: &mut ChunkGraph,
    order_state: &OrderWrapState,
  ) -> DynamicDeferPlacements {
    let mut placements = vec![];
    if !self.options.experimental.is_import_defer_enabled() {
      return placements;
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
    let ChunkGraph {
      chunk_table,
      module_to_chunk,
      deferred_modules_run_at_load,
      run_at_load_anchors,
      ..
    } = chunk_graph;
    for (chunk_idx, chunk) in chunk_table.iter_enumerated() {
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
      // The first importer of a target keeps its place, so each moved module has an anchor.
      for (position, &module_idx) in chunk.modules.iter().enumerate() {
        if !moved.contains(&module_idx) || !deferred_modules_run_at_load.contains(&module_idx) {
          continue;
        }
        let anchor = chunk.modules[..position]
          .iter()
          .rev()
          .find(|&&idx| idx != runtime_idx && !moved.contains(&idx));
        match anchor {
          Some(&anchor) => {
            run_at_load_anchors.insert(module_idx, anchor);
          }
          None => {
            moved.remove(&module_idx);
          }
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
      placements.push((chunk_idx, modules));
    }
    placements
  }

  /// Applies the plan of `plan_dynamic_defer_declarations`. Call it after `finalize_modules`.
  pub fn place_dynamic_defer_declarations(
    chunk_graph: &mut ChunkGraph,
    placements: DynamicDeferPlacements,
  ) {
    for (chunk_idx, modules) in placements {
      chunk_graph.chunk_table[chunk_idx].modules = modules;
    }
  }
}
