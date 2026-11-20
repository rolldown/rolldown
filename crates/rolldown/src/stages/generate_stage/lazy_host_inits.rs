use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkModulesOrderBy, CrossChunkImportItem, ImportKind,
  ImportRecordIdx, ModuleIdx, NormalModule, NormalizedBundlerOptions, OutputExports, OutputFormat,
  PreliminaryFilename, RuntimeHelper, StaticImport, UsedSymbolRefs, WrapKind,
};
use rolldown_utils::{concat_string, dashmap::FxDashMap, make_unique_name::make_unique_name};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::{ChunkGraph, LazyHostInit, LazyHostInits},
  esm_init_obligations::{
    ObligationPurpose, WrappedEsmInitTargetContext,
    collect_wrapped_esm_init_targets_for_import_record, record_is_init_obligation,
  },
  stages::link_stage::LinkStageOutput,
};

use super::{
  FinalEsmInitMetadata, GenerateStage, PreGeneratedChunkName,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  order_wrap_state::OrderWrapState,
};

/// Where an import of the importer runs, when the chunk of the importer loads
/// (`LazyHostInitPlanner::import_load_step`).
enum ImportLoadStep {
  /// While the chunk loads: at this position of its `static_imports`.
  AtLoad(usize),
  /// The import runs no code.
  NoCode,
  /// In the body of the chunk: the importer runs code for the import there, for example
  /// `init_x()`, `require_x()`, a read of a `default` getter, or the preparation of a deferred
  /// import.
  InBody,
}

impl GenerateStage<'_> {
  /// The host of a deferred module loads without running the module
  /// (`ChunkGraph::deferred_init_host`): the lazy host of the module, or a common chunk. Thus an
  /// eager importer in another chunk calls `init_x()` in its body. The imports of a chunk run
  /// before its body, so the module then runs after the later static imports of the importer:
  /// `import './x.js'; import './y.js'` runs `y` first. Here, a chunk with such an importer
  /// initializes the module right after its import of the host. In `esm` output, an init chunk
  /// with no modules imports `init_x` from the host and calls it, and the chunk imports the init
  /// chunk right after the host. In `cjs` output, the chunk calls `require_x.init_x()` after the
  /// `require()` of the host. The importer then does not call `init_x()` in its body. A common
  /// chunk can hold more than one deferred module, so each init chunk calls the init function of
  /// one module.
  ///
  /// The init then runs before the body of the chunk, so each step that runs before the module in
  /// native ESM must also run before the init:
  /// - Only the first module of the body that runs code takes part. A wrapped importer calls
  ///   `init_x()` in its own init function, and a deferred-only importer has no call.
  /// - Each earlier import of the importer must run while the chunk loads (`import_load_step`).
  ///   When the import of an earlier module comes after the import of the host, the init runs
  ///   after that import: in `import './a1.js'; import './w.js'; import './a2.js'`, with `a1` and
  ///   `a2` in one common host and `w` in another chunk, the init of `a2` runs after the import of
  ///   `w`. An import that runs code in the body stops the plan for the later hosts of the
  ///   importer: a module of the same chunk, a CommonJS module that a getter runs, an init function
  ///   that the importer calls for a module behind an unwrapped re-export, and the preparation of a
  ///   deferred import with top-level await (`__esmGather`).
  /// - A chunk initializes a module only when a static import of the chunk after the init can run
  ///   code at load (`chunk_runs_code_at_load`). Otherwise the call in the body runs in the same
  ///   order.
  /// - The host must not import the chunk, directly or through other chunks. In such a cycle, the
  ///   module can read bindings of the chunk before its body runs.
  /// - With `strictExecutionOrder`, only with `preserveModules`. Without it, a module of a common
  ///   chunk runs when an init function calls it, so the order wrappers already call `init_x()` in
  ///   the order of the imports. With it, each file calls the init function of its own module at
  ///   load, so an importer with an order wrapper also runs when its chunk loads. Such an importer
  ///   keeps its call: in a cycle, another module can call its init function before its chunk
  ///   imports the init chunk. A common chunk that holds an entry module is not a host there
  ///   (`LazyHostInitPlanner::init_host`).
  /// - Not with `experimentalInlineCommonChunks`: a file can run the modules of the records that
  ///   it carries.
  ///
  /// When the first two rules decline an importer, the call in its body can run the module after
  /// code of a later static import of the chunk. The chunk then initializes the module at the
  /// place of the plan anyway, as the build without `import defer` does
  /// (`LazyHostInitPlanner::add_declined_inits`).
  ///
  /// Call it after the cross-chunk links and the export modes are final: it reads
  /// `static_imports`, `imports_from_other_chunks` and `output_exports`. The init chunks need
  /// names, so call it before the chunks get their names.
  /// See internal-docs/import-defer/implementation.md ("Init after the import of a host").
  pub fn plan_lazy_host_inits(
    &self,
    chunk_graph: &mut ChunkGraph,
    order_state: &OrderWrapState,
    used_symbol_refs: &UsedSymbolRefs,
    final_esm_init_metadata: &FinalEsmInitMetadata,
  ) {
    if !self.options.experimental.is_import_defer_enabled()
      || (self.options.is_strict_execution_order_enabled() && !self.options.preserve_modules)
      || self.options.is_dev_mode_enabled()
      || self.options.inline_common_chunks.is_some()
      || !matches!(self.options.format, OutputFormat::Esm | OutputFormat::Cjs)
    {
      return;
    }
    let mut planner = LazyHostInitPlanner {
      link_output: self.link_output,
      options: self.options,
      chunk_graph,
      order_state,
      used_symbol_refs,
      final_esm_init_metadata,
      runs_code_at_load: FxHashMap::default(),
    };
    chunk_graph.lazy_host_inits = planner.plan();
    if matches!(self.options.format, OutputFormat::Esm) {
      self.add_lazy_host_init_chunks(chunk_graph);
    }
  }

  /// Only the first module of the body of a chunk can take part in `plan_lazy_host_inits`: an init
  /// before the body also runs before each earlier module of the body. In `import './x.js'; import
  /// './a.js'; import './y.js'`, native ESM runs `x`, `a`, `y`. When `x` is in the chunk of the
  /// importer, and `a` is a deferred module in another chunk, the chunk runs `y`, `x`, `a`. Here,
  /// the modules of the body before the importer move to a new common chunk. The chunk imports the
  /// new chunk at its first import of them, so the plan can then initialize `a` after the import
  /// of its host: `x`, `a`, `y`. The same holds for `import './a.js'; import './x.js'; import
  /// './y.js'`: `a`, `x`, `y`.
  ///
  /// The importer is the first module of the body with an import of a deferred module in another
  /// chunk. A chunk splits only when the moved modules cannot read the chunk at load, and keep
  /// their order among the other imports of the chunk:
  /// - The moved modules do not import the other modules of the chunk, have no top-level await and
  ///   use no runtime helper.
  /// - The walk of the import ranks (`chunk_import_ranks`) visits the moved modules as one block
  ///   (`split_keeps_import_order`).
  /// - No import of the importer before the import of `a` calls an init function in the body. The
  ///   plan stops at such an import.
  /// - A later import of the importer can run code at load: an import of another chunk or an
  ///   external. Before a later import of a module of the chunk,
  ///   `ensure_lazy_module_initialization_order` already calls `init_a()`.
  ///
  /// Before that, a common host gives up its later modules (`modules_after_deferred_modules`).
  ///
  /// Call it after the modules have their chunks, and before the chunk modules are sorted.
  /// See internal-docs/import-defer/implementation.md ("Split before a deferred import").
  pub(super) fn split_modules_before_deferred_imports(&self, chunk_graph: &mut ChunkGraph) {
    let options = self.options;
    if !options.experimental.is_import_defer_enabled()
      || options.is_strict_execution_order_enabled()
      || options.preserve_modules
      || options.is_dev_mode_enabled()
      || options.inline_common_chunks.is_some()
      || options.code_splitting.is_disabled()
      || !matches!(options.format, OutputFormat::Esm | OutputFormat::Cjs)
      // `split_keeps_import_order` and `modules_after_deferred_modules` read the chunk modules in
      // execution order.
      || matches!(options.experimental.chunk_modules_order, Some(ChunkModulesOrderBy::ModuleId))
      || !self.link_output.metas.iter().any(|meta| meta.wrapped_by_import_defer)
    {
      return;
    }
    let eagerly_imported = self.deferred_modules_with_eager_importers(chunk_graph);
    if eagerly_imported.is_empty() {
      return;
    }
    // The common hosts first, so that the importers see the final chunk of each module.
    let host_splits = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter_map(|(chunk_idx, _)| {
        self
          .modules_after_deferred_modules(chunk_graph, chunk_idx, &eagerly_imported)
          .map(|moved| (chunk_idx, moved))
      })
      .collect::<Vec<_>>();
    for (chunk_idx, moved) in host_splits {
      self.move_modules_to_new_common_chunk(chunk_graph, chunk_idx, &moved);
    }
    let splits = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter_map(|(chunk_idx, _)| {
        self.modules_before_deferred_import(chunk_graph, chunk_idx).map(|moved| (chunk_idx, moved))
      })
      .collect::<Vec<_>>();
    for (chunk_idx, moved) in splits {
      self.move_modules_to_new_common_chunk(chunk_graph, chunk_idx, &moved);
    }
  }

  /// Moves the modules to a new common chunk with the bits of their chunk. The old chunk keeps the
  /// runtime helpers of its other modules. `ExportAll` stays: the chunk optimizer adds it for the
  /// namespace of an eliminated facade.
  fn move_modules_to_new_common_chunk(
    &self,
    chunk_graph: &mut ChunkGraph,
    chunk_idx: ChunkIdx,
    moved: &[ModuleIdx],
  ) {
    let metas = &self.link_output.metas;
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    let bits = chunk.bits.clone();
    let mut split = Chunk::new(
      None,
      None,
      bits.clone(),
      vec![],
      ChunkKind::Common,
      chunk.input_base.clone(),
      None,
    );
    split.add_creation_reason(
      ChunkCreationReason::CommonChunk { bits: &bits, link_output: self.link_output },
      self.options,
    );
    let split_idx = chunk_graph.add_chunk(split);
    let moved_set = moved.iter().copied().collect::<FxHashSet<_>>();
    let old = &mut chunk_graph.chunk_table[chunk_idx];
    old.modules.retain(|module_idx| !moved_set.contains(module_idx));
    let helpers_of = |modules: &[ModuleIdx]| {
      modules.iter().fold(RuntimeHelper::empty(), |helpers, module_idx| {
        helpers | metas[*module_idx].depended_runtime_helper
      })
    };
    let moved_only = helpers_of(moved).difference(helpers_of(&old.modules));
    old.depended_runtime_helper.remove(moved_only.difference(RuntimeHelper::ExportAll));
    for &module_idx in moved {
      chunk_graph.add_module_to_chunk(
        module_idx,
        split_idx,
        metas[module_idx].depended_runtime_helper,
      );
    }
  }

  /// The deferred modules with a kept eager import from a module with no wrapper in another chunk
  /// than their host (`ChunkGraph::deferred_init_host`). `plan_lazy_host_inits` can initialize
  /// such a module after the import of its host.
  fn deferred_modules_with_eager_importers(
    &self,
    chunk_graph: &ChunkGraph,
  ) -> FxHashSet<ModuleIdx> {
    let metas = &self.link_output.metas;
    let mut result = FxHashSet::default();
    for module in self.link_output.module_table.modules.iter().filter_map(|m| m.as_normal()) {
      let meta = &metas[module.idx];
      let Some(chunk_idx) = chunk_graph.module_to_chunk[module.idx] else { continue };
      if !matches!(meta.wrap_kind(), WrapKind::None) {
        continue;
      }
      for rec in &module.import_records {
        let Some(target_idx) = rec.resolved_module else { continue };
        if rec.kind == ImportKind::Import
          && !rec.is_defer_phase()
          && meta.load_dependencies.contains(&target_idx)
          && chunk_graph
            .deferred_init_host(target_idx, &metas[target_idx])
            .is_some_and(|host_idx| host_idx != chunk_idx)
        {
          result.insert(target_idx);
        }
      }
    }
    result
  }

  /// A common host can hold a deferred module `a` and a module `w` that runs at load. The host
  /// then runs `w` when it loads, before the init after the import of the host. In `import
  /// './a.js'; import './w.js'`, native ESM runs `a`, then `w`. Here, the modules of the host that
  /// run at load and come after its first deferred module in execution order move to a new common
  /// chunk. The importers load the new chunk at their first import of its modules, so `b` runs
  /// `a`, then `w`. A deferred-only importer loads both chunks and runs only `w`. The earlier
  /// modules stay: the host runs them at load, before `a` in the same order as the build without
  /// `import defer`. The first deferred module counts only with an eager importer in another chunk
  /// (`deferred_modules_with_eager_importers`).
  ///
  /// The host splits only when the move keeps the load of each part:
  /// - The modules that stay do not import the moved modules: else the host loads the new chunk.
  /// - No dynamic import goes from one part to the other.
  /// - The moved modules have no top-level await, and the chunk does not keep their exports for a
  ///   facade.
  fn modules_after_deferred_modules(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    eagerly_imported: &FxHashSet<ModuleIdx>,
  ) -> Option<Vec<ModuleIdx>> {
    let link_output = &*self.link_output;
    let module_table = &link_output.module_table;
    let metas = &link_output.metas;
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    if !matches!(chunk.kind, ChunkKind::Common)
      || !chunk_graph.chunk_is_live(chunk_idx)
      || chunk.chunk_reason_type.group_index().is_some()
    {
      return None;
    }
    let exec_order = |module_idx: ModuleIdx| module_table[module_idx].exec_order();
    let first_deferred = chunk
      .modules
      .iter()
      .copied()
      .filter(|&module_idx| {
        eagerly_imported.contains(&module_idx)
          && chunk_graph.deferred_init_host(module_idx, &metas[module_idx]) == Some(chunk_idx)
      })
      .map(exec_order)
      .min()?;
    let runtime_idx = link_output.runtime.id();
    let mut moved = chunk
      .modules
      .iter()
      .copied()
      .filter(|&module_idx| {
        module_idx != runtime_idx
          && matches!(metas[module_idx].wrap_kind(), WrapKind::None)
          && exec_order(module_idx) > first_deferred
      })
      .collect::<Vec<_>>();
    if moved.is_empty() {
      return None;
    }
    moved.sort_unstable_by_key(|&module_idx| exec_order(module_idx));
    let moved_set = moved.iter().copied().collect::<FxHashSet<_>>();
    if moved.iter().any(|&module_idx| metas[module_idx].is_tla_or_contains_tla_dependency)
      || chunk_graph
        .common_chunk_preserve_export_names_modules
        .get(&chunk_idx)
        .into_iter()
        .chain(chunk_graph.common_chunk_exported_facade_chunk_namespace.get(&chunk_idx))
        .flatten()
        .any(|module_idx| moved_set.contains(module_idx))
    {
      return None;
    }
    let mut importees = vec![];
    for &module_idx in &chunk.modules {
      let Some(module) = module_table[module_idx].as_normal() else { continue };
      let is_moved = moved_set.contains(&module_idx);
      if module.import_records.iter().any(|rec| {
        rec.kind.is_dynamic()
          && rec.resolved_module.is_some_and(|target_idx| {
            chunk_graph.module_to_chunk[target_idx] == Some(chunk_idx)
              && moved_set.contains(&target_idx) != is_moved
          })
      }) {
        return None;
      }
      if !is_moved {
        self.push_kept_static_importees(module_idx, &mut importees);
        if importees.drain(..).any(|importee| moved_set.contains(&importee)) {
          return None;
        }
      }
    }
    Some(moved)
  }

  /// The modules that `split_modules_before_deferred_imports` moves out of the chunk, in execution
  /// order: the modules of the body before the importer.
  fn modules_before_deferred_import(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
  ) -> Option<Vec<ModuleIdx>> {
    let link_output = &*self.link_output;
    let module_table = &link_output.module_table;
    let metas = &link_output.metas;
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    if !chunk_graph.chunk_is_live(chunk_idx)
      || chunk.chunk_reason_type.group_index().is_some()
      || chunk.modules.contains(&link_output.runtime.id())
    {
      return None;
    }
    let exec_order = |module_idx: ModuleIdx| module_table[module_idx].exec_order();
    // The first module of the walk of `chunk_import_ranks`.
    let lead = match chunk.kind {
      ChunkKind::EntryPoint { module, .. } => module,
      ChunkKind::Common => {
        chunk.modules.iter().copied().max_by_key(|&module_idx| exec_order(module_idx))?
      }
    };
    // The entry module of a facade is in another chunk.
    if chunk_graph.module_to_chunk[lead] != Some(chunk_idx) {
      return None;
    }
    // The body in the order of `sort_chunk_modules`, and the importer: the first module of the
    // body with an import of a deferred module in another chunk.
    let mut body = chunk
      .modules
      .iter()
      .copied()
      .filter(|&module_idx| matches!(metas[module_idx].wrap_kind(), WrapKind::None))
      .collect::<Vec<_>>();
    body.sort_unstable_by_key(|&module_idx| exec_order(module_idx));
    let (position, host_idx) = body.iter().enumerate().find_map(|(position, &module_idx)| {
      self
        .first_deferred_import_host(chunk_graph, chunk_idx, module_idx)
        .map(|host| (position, host))
    })?;
    let importer_idx = body[position];
    let moved = body[..position].to_vec();
    let moved_set = moved.iter().copied().collect::<FxHashSet<_>>();
    if moved.is_empty()
      || metas[importer_idx].is_tla_or_contains_tla_dependency
      || chunk_graph
        .common_chunk_preserve_export_names_modules
        .get(&chunk_idx)
        .into_iter()
        .chain(chunk_graph.common_chunk_exported_facade_chunk_namespace.get(&chunk_idx))
        .flatten()
        .any(|module_idx| moved_set.contains(module_idx))
    {
      return None;
    }
    // Each import of the importer before the import of the host must run at load after the
    // split, and a later import must run code at load.
    let importer = module_table[importer_idx].as_normal()?;
    let load_dependencies = &metas[importer_idx].load_dependencies;
    let mut host_seen = false;
    let mut earlier_import_moves = false;
    let mut later_import_runs_code = false;
    for rec in &importer.import_records {
      if rec.kind != ImportKind::Import {
        continue;
      }
      let Some(target_idx) = rec.resolved_module else { continue };
      if rec.is_defer_phase() {
        // The body prepares a deferred import of a module with top-level await, so the plan stops
        // there (`deferred_import_has_preparation`).
        if !host_seen && metas[target_idx].is_tla_or_contains_tla_dependency {
          return None;
        }
        continue;
      }
      // Tree shaking keeps an import when its module is in `load_dependencies`, also when it keeps
      // no statement of the import, as for `import './x.js'` (`push_kept_static_importees`).
      if !load_dependencies.contains(&target_idx) {
        continue;
      }
      let target_chunk_idx = chunk_graph.module_to_chunk[target_idx];
      if host_seen {
        // A later import of a module of the chunk needs no split:
        // `ensure_lazy_module_initialization_order` moves the init call before that module.
        later_import_runs_code |= match target_chunk_idx {
          Some(target_chunk_idx) => target_chunk_idx != chunk_idx && target_chunk_idx != host_idx,
          None => module_table[target_idx].is_external(),
        };
        continue;
      }
      match target_chunk_idx {
        Some(target_chunk_idx) if target_chunk_idx == chunk_idx => {
          if !moved_set.contains(&target_idx) {
            return None;
          }
          earlier_import_moves = true;
        }
        Some(target_chunk_idx) => {
          let target_meta = &metas[target_idx];
          if chunk_graph.deferred_init_host(target_idx, target_meta) == Some(target_chunk_idx) {
            host_seen = true;
          } else if !matches!(target_meta.wrap_kind(), WrapKind::None) {
            // The body calls the init function of the module, so the plan stops there.
            return None;
          }
        }
        None if module_table[target_idx].is_external() => {}
        None => return None,
      }
    }
    if !later_import_runs_code {
      return None;
    }
    // The moved modules must not read the chunk at load.
    let mut importees = vec![];
    for &module_idx in &moved {
      let meta = &metas[module_idx];
      if !meta.depended_runtime_helper.is_empty() || meta.is_tla_or_contains_tla_dependency {
        return None;
      }
      // A dynamic import of a module of the chunk can read its namespace in the chunk.
      let module = module_table[module_idx].as_normal()?;
      if module.import_records.iter().any(|rec| {
        rec.kind.is_dynamic()
          && rec
            .resolved_module
            .is_some_and(|target_idx| chunk_graph.module_to_chunk[target_idx] == Some(chunk_idx))
      }) {
        return None;
      }
      self.push_kept_static_importees(module_idx, &mut importees);
      if importees.drain(..).any(|importee| {
        chunk_graph.module_to_chunk[importee] == Some(chunk_idx) && !moved_set.contains(&importee)
      }) {
        return None;
      }
    }
    self
      .split_keeps_import_order(
        chunk_graph,
        chunk_idx,
        lead,
        &moved_set,
        host_idx,
        earlier_import_moves,
      )
      .then_some(moved)
  }

  /// The chunk of the first kept eager import of a deferred module in another chunk
  /// (`ChunkGraph::deferred_init_host`).
  fn first_deferred_import_host(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    module_idx: ModuleIdx,
  ) -> Option<ChunkIdx> {
    let module = self.link_output.module_table[module_idx].as_normal()?;
    let metas = &self.link_output.metas;
    let load_dependencies = &metas[module_idx].load_dependencies;
    module.import_records.iter().find_map(|rec| {
      let target_idx = rec.resolved_module?;
      if rec.kind != ImportKind::Import
        || rec.is_defer_phase()
        || !load_dependencies.contains(&target_idx)
      {
        return None;
      }
      chunk_graph
        .deferred_init_host(target_idx, &metas[target_idx])
        .filter(|&host_idx| host_idx != chunk_idx)
    })
  }

  /// The walk of `chunk_import_ranks` for the chunk after the split, where the moved modules are
  /// in a new chunk. The new chunk gets its rank at the first moved module that the walk visits.
  /// The order is kept when:
  /// - the moved modules that the walk visits are one block: the walk visits no other chunk or
  ///   external for the first time between them;
  /// - the new chunk comes before the host when an earlier import of the importer moves, so that
  ///   the plan takes the importer;
  /// - the moved modules that the walk visits lead to all moved modules;
  /// - in a common chunk, the walk visits each module that stays. Else the chunk sorts its imports
  ///   by execution order only.
  fn split_keeps_import_order(
    &self,
    chunk_graph: &ChunkGraph,
    chunk_idx: ChunkIdx,
    lead: ModuleIdx,
    moved: &FxHashSet<ModuleIdx>,
    host_idx: ChunkIdx,
    earlier_import_moves: bool,
  ) -> bool {
    let module_table = &self.link_output.module_table;
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    let mut remaining = chunk
      .modules
      .iter()
      .copied()
      .filter(|module_idx| !moved.contains(module_idx))
      .collect::<Vec<_>>();
    remaining.sort_unstable_by_key(|&module_idx| module_table[module_idx].exec_order());
    let mut stack = match chunk.kind {
      ChunkKind::EntryPoint { .. } => {
        remaining.iter().rev().copied().chain(std::iter::once(lead)).collect::<Vec<_>>()
      }
      ChunkKind::Common => vec![lead],
    };
    let mut visited = FxHashSet::default();
    let mut ranked_chunks = FxHashSet::default();
    let mut roots = vec![];
    let mut block_ended = false;
    let mut host_ranked = false;
    let mut walked = 0;
    let mut importees = vec![];
    while let Some(module_idx) = stack.pop() {
      if !visited.insert(module_idx) {
        continue;
      }
      if moved.contains(&module_idx) {
        if block_ended || (roots.is_empty() && earlier_import_moves && host_ranked) {
          return false;
        }
        roots.push(module_idx);
        continue;
      }
      match chunk_graph.module_to_chunk[module_idx] {
        Some(importee) if importee != chunk_idx => {
          if ranked_chunks.insert(importee) {
            block_ended |= !roots.is_empty();
            host_ranked |= importee == host_idx;
          }
        }
        Some(_) => {
          walked += 1;
          self.push_kept_static_importees(module_idx, &mut importees);
          stack.extend(importees.drain(..).rev());
        }
        None => {
          // `visited` makes each external a first visit.
          if module_table[module_idx].is_external() {
            block_ended |= !roots.is_empty();
          }
        }
      }
    }
    if matches!(chunk.kind, ChunkKind::Common) && walked < remaining.len() {
      return false;
    }
    // The moved modules that the roots lead to.
    let mut reached = FxHashSet::default();
    while let Some(module_idx) = roots.pop() {
      if !reached.insert(module_idx) {
        continue;
      }
      self.push_kept_static_importees(module_idx, &mut importees);
      roots.extend(importees.drain(..).filter(|importee| moved.contains(importee)));
    }
    reached.len() == moved.len()
  }

  /// `esm` output: adds the init chunk of each module of `inits_by_chunk`. Each chunk that
  /// initializes the module imports the init chunk right after the import of
  /// `LazyHostInit::after`, and after the init chunks of the earlier modules there. An init chunk
  /// is a common chunk with no modules, so the naming, the plugin hooks, the hashes and the
  /// minifier handle it as any other chunk. It is last in `sorted_chunk_idx_vec`, so the other
  /// chunks keep their names.
  fn add_lazy_host_init_chunks(&self, chunk_graph: &mut ChunkGraph) {
    let symbol_db = &self.link_output.symbol_db;
    let mut importers = chunk_graph
      .lazy_host_inits
      .inits_by_chunk
      .iter()
      .map(|(chunk_idx, inits)| (*chunk_idx, inits.clone()))
      .collect::<Vec<_>>();
    importers.sort_unstable_by_key(|(chunk_idx, _)| *chunk_idx);
    let mut init_chunk_by_module: FxHashMap<(ChunkIdx, ModuleIdx), ChunkIdx> = FxHashMap::default();
    for (chunk_idx, inits) in importers {
      // The import before the previous init, and the number of init chunks after that import so
      // far. The inits after one import are next to each other in `inits`.
      let mut previous: Option<(ChunkIdx, usize)> = None;
      for LazyHostInit { after, host: host_idx, module } in inits {
        let offset = match previous {
          Some((previous_after, count)) if previous_after == after => count,
          _ => 0,
        };
        previous = Some((after, offset + 1));
        let init_idx = if let Some(&init_idx) = init_chunk_by_module.get(&(host_idx, module)) {
          init_idx
        } else {
          let host = &chunk_graph.chunk_table[host_idx];
          let wrapper_ref = self.link_output.metas[module]
            .wrapper_ref
            .map(|wrapper_ref| symbol_db.canonical_ref_for(wrapper_ref));
          // The import of `init_x` by the chunk, which the host exports under this reference.
          let import_ref = chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks[&host_idx]
            .iter()
            .find(|item| Some(symbol_db.canonical_ref_for(item.import_ref)) == wrapper_ref)
            .expect("the plan checks that the chunk imports `init_x`")
            .import_ref;
          let mut init = Chunk::new(
            None,
            None,
            host.bits.clone(),
            vec![],
            ChunkKind::Common,
            host.input_base.clone(),
            None,
          );
          init.exec_order = host.exec_order;
          init.output_exports = OutputExports::Named;
          init.static_imports.push(StaticImport::Chunk(host_idx));
          init.cross_chunk_imports.push(host_idx);
          init
            .imports_from_other_chunks
            .insert(host_idx, vec![CrossChunkImportItem { import_ref }]);
          let init_idx = chunk_graph.add_chunk(init);
          chunk_graph.sorted_chunk_idx_vec.push(init_idx);
          init_chunk_by_module.insert((host_idx, module), init_idx);
          init_idx
        };
        let chunk = &mut chunk_graph.chunk_table[chunk_idx];
        let position = chunk
          .static_imports
          .iter()
          .position(|import| *import == StaticImport::Chunk(after))
          .expect("the plan reads the position of the import");
        chunk.static_imports.insert(position + 1 + offset, StaticImport::Chunk(init_idx));
        let position = chunk
          .cross_chunk_imports
          .iter()
          .position(|importee| *importee == after)
          .map_or(chunk.cross_chunk_imports.len(), |position| position + 1 + offset);
        chunk.cross_chunk_imports.insert(position, init_idx);
        let index = chunk
          .imports_from_other_chunks
          .get_index_of(&after)
          .expect("every chunk of `static_imports` is in `imports_from_other_chunks`");
        chunk.imports_from_other_chunks.shift_insert(index + 1 + offset, init_idx, vec![]);
      }
    }
    chunk_graph.lazy_host_inits.host_by_init_chunk =
      init_chunk_by_module.into_iter().map(|(target, init_idx)| (init_idx, target)).collect();
  }

  /// The init chunk of a lazy host gets the name of the host with `.init`: `x.init`, or
  /// `dir/x.init` with `preserveModules`. Without `preserveModules`, the init chunk uses the file
  /// name template of the host, so its file is next to the host. With `preserveModules`, a chunk
  /// with no module is in the folder of the generated chunks: `_virtual/dir/x.init.js`. A common
  /// chunk can hold more than one deferred module, so the init chunk of a module of a common
  /// chunk gets the name of the module with `.init`: its own pre-generated name is the name of the
  /// module. Call it after the hosts have their pre-generated names.
  pub(super) fn name_lazy_host_init_chunks(
    chunk_graph: &ChunkGraph,
    names: &mut IndexVec<ChunkIdx, PreGeneratedChunkName>,
  ) {
    for (&init_idx, &(host_idx, _)) in &chunk_graph.lazy_host_inits.host_by_init_chunk {
      let base_idx = if matches!(chunk_graph.chunk_table[host_idx].kind, ChunkKind::Common) {
        init_idx
      } else {
        host_idx
      };
      let host_name = &names[base_idx];
      let name = PreGeneratedChunkName {
        representative_chunk_name: concat_string!(host_name.representative_chunk_name, ".init")
          .into(),
        chunk_name: concat_string!(host_name.chunk_name, ".init").into(),
      };
      names[init_idx] = name;
    }
  }

  /// The names, in lower case, that the file of an init chunk must not have: the emitted assets,
  /// the emitted prebuilt chunks and the chunks with a `fileName`. The other chunks reserve their
  /// names in `used_name_counts`. An asset that a plugin emits after the chunks have their names
  /// is not in the list.
  pub(super) fn lazy_host_init_reserved_names(
    &self,
    chunk_graph: &ChunkGraph,
  ) -> FxHashSet<String> {
    if chunk_graph.lazy_host_inits.host_by_init_chunk.is_empty() {
      return FxHashSet::default();
    }
    self
      .plugin_driver
      .file_emitter
      .asset_and_prebuilt_chunk_filenames()
      .chain(chunk_graph.chunk_table.iter().filter_map(|chunk| chunk.file_name.clone()))
      .map(|name| name.as_str().to_ascii_lowercase())
      .collect()
  }
}

/// The file name of an init chunk, when another file has the name: the name with a number, as for
/// two chunks with the same name (`make_unique_name`). With `x.init.js` from a plugin, the init
/// chunk of `x.js` is `x2.init.js`. A name with a hash is not changed.
pub(super) fn avoid_reserved_names(
  filename: PreliminaryFilename,
  reserved_names: &FxHashSet<String>,
  used_name_counts: &FxDashMap<ArcStr, u32>,
) -> PreliminaryFilename {
  if filename.hash_placeholder().is_some() {
    return filename;
  }
  let original: ArcStr = (*filename).clone();
  let mut name = original.clone();
  while reserved_names.contains(&name.as_str().to_ascii_lowercase()) {
    name = make_unique_name(&original, used_name_counts);
  }
  if name == original { filename } else { PreliminaryFilename::new(name, None) }
}

/// The read-only inputs of `plan_lazy_host_inits`.
struct LazyHostInitPlanner<'a> {
  link_output: &'a LinkStageOutput,
  options: &'a NormalizedBundlerOptions,
  chunk_graph: &'a ChunkGraph,
  order_state: &'a OrderWrapState,
  used_symbol_refs: &'a UsedSymbolRefs,
  final_esm_init_metadata: &'a FinalEsmInitMetadata,
  /// The memo of `chunk_runs_code_at_load`.
  runs_code_at_load: FxHashMap<ChunkIdx, bool>,
}

impl LazyHostInitPlanner<'_> {
  /// For each chunk, the modules that it initializes, the imports that their inits follow, and the
  /// moved calls, in the order of its `static_imports`. The init chunks come later
  /// (`GenerateStage::add_lazy_host_init_chunks`).
  fn plan(&mut self) -> LazyHostInits {
    let mut plan = LazyHostInits::default();
    let chunk_graph = self.chunk_graph;
    let link_output = self.link_output;
    let hosts_of_deferred_modules = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(chunk_idx, chunk)| {
        chunk_graph.chunk_is_live(*chunk_idx)
          && chunk.modules.iter().any(|&module_idx| self.init_host(module_idx) == Some(*chunk_idx))
      })
      .map(|(chunk_idx, _)| chunk_idx)
      .collect::<FxHashSet<_>>();
    if hosts_of_deferred_modules.is_empty() {
      return plan;
    }
    let runtime_idx = link_output.runtime.id();
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if !chunk_graph.chunk_is_live(chunk_idx)
        || !chunk.static_imports.iter().any(|import| {
          matches!(import, StaticImport::Chunk(importee_idx)
            if hosts_of_deferred_modules.contains(importee_idx))
        })
      {
        continue;
      }
      // The modules that the chunk initializes, with the positions in `static_imports` of the
      // imports that their inits follow.
      let mut inits: Vec<(usize, LazyHostInit)> = vec![];
      for &importer_idx in &chunk.modules {
        let Some(importer) = link_output.module_table[importer_idx].as_normal() else {
          continue;
        };
        let importer_meta = &link_output.metas[importer_idx];
        if importer_idx == runtime_idx || !matches!(importer_meta.wrap_kind(), WrapKind::None) {
          continue;
        }
        // A consumer-local re-export route renders none of its imports.
        if self.order_state.is_consumer_local_reexport_route(importer_idx) {
          break;
        }
        // Order wrappers exist only with `strictExecutionOrder`, so here with `preserveModules`.
        let keeps_call = self.order_state.has_order_wrapper(importer_idx);
        // An excluded re-export in the init function of an order wrapper still calls the init
        // functions that it forwards to.
        let excluded_init_targets =
          self.final_esm_init_metadata.transitive_init_targets(importer_idx);
        // The last position in `static_imports` of an earlier import of the importer, and whether
        // each earlier import runs while the chunk loads.
        let mut last_position: Option<usize> = None;
        let mut earlier_imports_at_load = true;
        for (stmt_idx, stmt_info) in link_output.stmt_infos[importer_idx].iter_enumerated() {
          if !importer_meta.stmt_info_included.has_bit(stmt_idx) {
            if excluded_init_targets
              .and_then(|targets_by_stmt| targets_by_stmt.get(&stmt_idx))
              .is_some_and(|targets| !targets.is_empty())
            {
              earlier_imports_at_load = false;
            }
            continue;
          }
          for &rec_idx in &stmt_info.import_records {
            let rec = &importer.import_records[rec_idx];
            if rec.kind != ImportKind::Import {
              continue;
            }
            let Some(target_idx) = rec.resolved_module else { continue };
            if rec.is_defer_phase() {
              if self.deferred_import_has_preparation(target_idx) {
                earlier_imports_at_load = false;
              }
              continue;
            }
            // An earlier import at the position of the host runs while the host loads, so before
            // the init. For example, an earlier deferred module of the same common chunk.
            match self.lazy_host_to_init(chunk_idx, target_idx, last_position) {
              Some((init, position)) if earlier_imports_at_load => {
                if !keeps_call {
                  plan.moved_calls.insert((importer_idx, target_idx));
                }
                if !inits
                  .iter()
                  .any(|(_, other)| other.host == init.host && other.module == init.module)
                {
                  inits.push((position, init));
                }
                last_position = Some(position);
              }
              _ => match self.import_load_step(chunk_idx, importer, rec_idx, target_idx) {
                ImportLoadStep::AtLoad(position) => {
                  last_position = Some(last_position.map_or(position, |last| last.max(position)));
                }
                ImportLoadStep::NoCode => {}
                ImportLoadStep::InBody => earlier_imports_at_load = false,
              },
            }
          }
        }
        // The body runs after all imports of the chunk, so an init after the import of a host
        // would also run before each later module of the body.
        if !keeps_call {
          break;
        }
      }
      self.add_declined_inits(chunk_idx, &mut inits, &mut plan.moved_calls);
      if !inits.is_empty() {
        // A stable sort: the inits after one import keep the order of the imports.
        inits.sort_by_key(|(position, _)| *position);
        plan.inits_by_chunk.insert(chunk_idx, inits.into_iter().map(|(_, init)| init).collect());
      }
    }
    plan
  }

  /// The inits that the plan declines, when the call in the body runs the module too late. The
  /// plan declines an importer when a step before the module would not run before the init: an
  /// earlier import that runs code in the body, or an earlier module of the body. The call then
  /// stays in the body of the importer, after each import of the chunk. When a static import of
  /// the chunk after the place of the init runs code at load (`lazy_host_to_init`), that code then
  /// runs before the module, and native ESM runs it after the module.
  ///
  /// Here, the chunk initializes such a module at the place of the plan anyway, after the import of
  /// the host or of the last earlier import of the importer that runs at load. The build without
  /// `import defer` runs the module when its chunk loads, so also before the code of the body.
  /// Thus the init keeps the order of that build where the plan cannot give the native order. Each
  /// module of the body takes part, also after the first one.
  /// See internal-docs/import-defer/implementation.md ("Init after the import of a host").
  fn add_declined_inits(
    &mut self,
    chunk_idx: ChunkIdx,
    inits: &mut Vec<(usize, LazyHostInit)>,
    moved_calls: &mut FxHashSet<(ModuleIdx, ModuleIdx)>,
  ) {
    let chunk_graph = self.chunk_graph;
    let link_output = self.link_output;
    let runtime_idx = link_output.runtime.id();
    for &importer_idx in &chunk_graph.chunk_table[chunk_idx].modules {
      let Some(importer) = link_output.module_table[importer_idx].as_normal() else {
        continue;
      };
      let importer_meta = &link_output.metas[importer_idx];
      if importer_idx == runtime_idx
        || !matches!(importer_meta.wrap_kind(), WrapKind::None)
        || self.order_state.is_consumer_local_reexport_route(importer_idx)
      {
        continue;
      }
      let keeps_call = self.order_state.has_order_wrapper(importer_idx);
      let mut last_position: Option<usize> = None;
      for (stmt_idx, stmt_info) in link_output.stmt_infos[importer_idx].iter_enumerated() {
        if !importer_meta.stmt_info_included.has_bit(stmt_idx) {
          continue;
        }
        for &rec_idx in &stmt_info.import_records {
          let rec = &importer.import_records[rec_idx];
          let Some(target_idx) = rec.resolved_module else { continue };
          if rec.kind != ImportKind::Import || rec.is_defer_phase() {
            continue;
          }
          match self.lazy_host_to_init(chunk_idx, target_idx, last_position) {
            Some((init, position)) => {
              // An init of the plan, or of an earlier importer, already runs the module at load.
              if !inits
                .iter()
                .any(|(_, other)| other.host == init.host && other.module == init.module)
              {
                if !keeps_call {
                  moved_calls.insert((importer_idx, target_idx));
                }
                inits.push((position, init));
              }
              last_position = Some(position);
            }
            None => {
              if let ImportLoadStep::AtLoad(position) =
                self.import_load_step(chunk_idx, importer, rec_idx, target_idx)
              {
                last_position = Some(last_position.map_or(position, |last| last.max(position)));
              }
            }
          }
        }
      }
    }
  }

  /// The host of a deferred module (`ChunkGraph::deferred_init_host`). With
  /// `strictExecutionOrder`, a common chunk that holds an entry module is not a host.
  /// `preserveModules` moves such an entry, for example a dynamic entry that another file also
  /// imports statically (`collapse_carries_trigger`), into a common chunk that does not run it at
  /// load, also without `import defer`. Its importers then keep the order of that build.
  fn init_host(&self, module_idx: ModuleIdx) -> Option<ChunkIdx> {
    let host_idx =
      self.chunk_graph.deferred_init_host(module_idx, &self.link_output.metas[module_idx])?;
    let collapsed_entry = self.options.is_strict_execution_order_enabled()
      && matches!(self.chunk_graph.chunk_table[host_idx].kind, ChunkKind::Common)
      && self.link_output.entries.contains_key(&module_idx);
    (!collapsed_entry).then_some(host_idx)
  }

  /// The init of `target_idx` and the position in the `static_imports` of the chunk of the import
  /// that the init follows, when the chunk can initialize the module there: the chunk imports
  /// `init_x` from the host (`init_host`), and the host does not import the chunk. The init
  /// follows the import of the host, or the last earlier import of the importer (`last_position`)
  /// when that import is a chunk after the host. A static import after the init must run code at
  /// load.
  fn lazy_host_to_init(
    &mut self,
    chunk_idx: ChunkIdx,
    target_idx: ModuleIdx,
    last_position: Option<usize>,
  ) -> Option<(LazyHostInit, usize)> {
    let chunk_graph = self.chunk_graph;
    let link_output = self.link_output;
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    let target_meta = &link_output.metas[target_idx];
    let host_idx = self.init_host(target_idx)?;
    if host_idx == chunk_idx {
      return None;
    }
    // The chunk imports `init_x` from the host: the call in the body needs it, and the init after
    // the import calls it.
    let symbol_db = &link_output.symbol_db;
    let wrapper_ref = symbol_db.canonical_ref_for(target_meta.wrapper_ref?);
    let imports_wrapper = chunk.imports_from_other_chunks.get(&host_idx).is_some_and(|items| {
      items.iter().any(|item| symbol_db.canonical_ref_for(item.import_ref) == wrapper_ref)
    });
    let host_position =
      chunk.static_imports.iter().position(|import| *import == StaticImport::Chunk(host_idx))?;
    if !imports_wrapper || self.chunk_imports_chunk(host_idx, chunk_idx) {
      return None;
    }
    let position = last_position.map_or(host_position, |last| last.max(host_position));
    let StaticImport::Chunk(after) = chunk.static_imports[position] else {
      return None;
    };
    let later_import_runs_code =
      chunk.static_imports[position + 1..].iter().any(|import| match *import {
        StaticImport::External(_) => true,
        StaticImport::Chunk(importee_idx) => self.chunk_runs_code_at_load(importee_idx),
      });
    later_import_runs_code
      .then_some((LazyHostInit { after, host: host_idx, module: target_idx }, position))
  }

  /// Whether the load of `from` loads `to`, through the static imports of the chunks.
  fn chunk_imports_chunk(&self, from: ChunkIdx, to: ChunkIdx) -> bool {
    let chunk_table = &self.chunk_graph.chunk_table;
    let mut visited = FxHashSet::from_iter([from]);
    let mut stack = vec![from];
    while let Some(chunk_idx) = stack.pop() {
      for import in &chunk_table[chunk_idx].static_imports {
        if let StaticImport::Chunk(importee_idx) = *import {
          if importee_idx == to {
            return true;
          }
          if visited.insert(importee_idx) {
            stack.push(importee_idx);
          }
        }
      }
    }
    false
  }

  /// Where the import `rec_idx` of the importer runs, when the chunk loads. An external module and
  /// a module with no wrapper in another chunk run when their chunk loads. A wrapped module runs at
  /// load only through the tail of its own entry chunk (`runs_by_tail`). A module of the same
  /// chunk runs in the body. An import through which the importer calls init functions runs in the
  /// body (`has_init_obligation_targets`).
  fn import_load_step(
    &self,
    chunk_idx: ChunkIdx,
    importer: &NormalModule,
    rec_idx: ImportRecordIdx,
    target_idx: ModuleIdx,
  ) -> ImportLoadStep {
    let chunk_graph = self.chunk_graph;
    let link_output = self.link_output;
    let static_imports = &chunk_graph.chunk_table[chunk_idx].static_imports;
    let step_of = |import: StaticImport| {
      static_imports
        .iter()
        .position(|item| *item == import)
        .map_or(ImportLoadStep::NoCode, ImportLoadStep::AtLoad)
    };
    if link_output.module_table[target_idx].as_normal().is_none() {
      return step_of(StaticImport::External(target_idx));
    }
    if self.has_init_obligation_targets(chunk_idx, importer, rec_idx, target_idx) {
      return ImportLoadStep::InBody;
    }
    let Some(target_chunk_idx) = chunk_graph.module_to_chunk[target_idx] else {
      return ImportLoadStep::NoCode;
    };
    if target_chunk_idx == chunk_idx {
      return ImportLoadStep::InBody;
    }
    let meta = &link_output.metas[target_idx];
    let has_no_wrapper =
      matches!(meta.wrap_kind(), WrapKind::None) && !self.order_state.has_order_wrapper(target_idx);
    if has_no_wrapper || self.runs_by_tail(target_chunk_idx, target_idx) {
      step_of(StaticImport::Chunk(target_chunk_idx))
    } else {
      ImportLoadStep::InBody
    }
  }

  /// Whether the tail of the entry chunk of a wrapped module runs the module when the chunk loads
  /// (`render_wrapped_entry_chunk`). A lazy host has no tail. In `cjs` output, the tail of a
  /// CommonJS module in `Named` mode is a getter of `default`: the importer runs the module when
  /// it reads `default` in its body. A tail that calls the init functions of consumer-local
  /// re-exports does not call the init function of the module.
  fn runs_by_tail(&self, target_chunk_idx: ChunkIdx, target_idx: ModuleIdx) -> bool {
    let chunk_graph = self.chunk_graph;
    let target_chunk = &chunk_graph.chunk_table[target_chunk_idx];
    let meta = &self.link_output.metas[target_idx];
    if !matches!(target_chunk.kind, ChunkKind::EntryPoint { module, .. } if module == target_idx)
      || chunk_graph.lazy_dynamic_defer_host(target_idx, meta).is_some()
    {
      return false;
    }
    if matches!(meta.wrap_kind(), WrapKind::Cjs) {
      return !(matches!(self.options.format, OutputFormat::Cjs)
        && matches!(target_chunk.output_exports, OutputExports::Named));
    }
    self.order_state.consumer_local_namespace_targets(target_idx).is_none()
      && self.order_state.esm_init_target(target_idx, meta).is_some()
  }

  /// Whether the body calls init functions for the import `rec_idx`: the importer reaches wrapped
  /// modules through an unwrapped module (`wrapped_esm_init_stmt_for_import_record`). The names of
  /// the chunk do not exist yet, so a wrapper of a module of the chunk and a wrapper that the chunk
  /// imports count as reachable in the chunk.
  fn has_init_obligation_targets(
    &self,
    chunk_idx: ChunkIdx,
    importer: &NormalModule,
    rec_idx: ImportRecordIdx,
    target_idx: ModuleIdx,
  ) -> bool {
    let link_output = self.link_output;
    let options = self.options;
    let order_state = self.order_state;
    if !matches!(link_output.metas[target_idx].wrap_kind(), WrapKind::None)
      && !order_state.is_consumer_local_reexport_route(target_idx)
    {
      return false;
    }
    let rec = &importer.import_records[rec_idx];
    if !record_is_init_obligation(
      ObligationPurpose::Emit,
      order_state,
      importer,
      rec,
      rec_idx,
      true,
    ) {
      return false;
    }
    let symbol_db = &link_output.symbol_db;
    let module_to_chunk = &self.chunk_graph.module_to_chunk;
    let chunk = &self.chunk_graph.chunk_table[chunk_idx];
    let ctx = WrappedEsmInitTargetContext {
      importer,
      importer_meta: &link_output.metas[importer.idx],
      modules: &link_output.module_table.modules,
      metas: &link_output.metas,
      stmt_infos: &link_output.stmt_infos,
      symbol_db,
      constant_value_map: &link_output.global_constant_symbol_map,
      inline_const_mode: options.optimization.inline_const.map(|config| config.mode),
      order_wrap_state: order_state,
      strict_execution_order: options.is_strict_execution_order_enabled(),
    };
    !collect_wrapped_esm_init_targets_for_import_record(
      &ctx,
      rec_idx,
      |symbol_ref| self.used_symbol_refs.contains(&symbol_ref),
      |wrapper_ref| {
        let wrapper_ref = symbol_db.canonical_ref_for(wrapper_ref);
        module_to_chunk[wrapper_ref.owner] == Some(chunk_idx)
          || chunk
            .imports_from_other_chunks
            .values()
            .flatten()
            .any(|item| symbol_db.canonical_ref_for(item.import_ref) == wrapper_ref)
      },
      |forwarding_module_idx| module_to_chunk[forwarding_module_idx] == Some(chunk_idx),
    )
    .is_empty()
  }

  /// Whether the importer prepares a deferred import of the module in its body:
  /// `__esmGather(init_x, ...) || await __esmGather()` (`deferred_import_preparation_stmt`). The
  /// preparation can evaluate modules with top-level await.
  fn deferred_import_has_preparation(&self, target_idx: ModuleIdx) -> bool {
    let meta = &self.link_output.metas[target_idx];
    matches!(meta.wrap_kind(), WrapKind::Esm)
      && meta.is_tla_or_contains_tla_dependency
      && meta.import_defer_tracked_deps.is_some()
  }

  /// Whether the load of a chunk can run code. A lazy host and a chunk with only the runtime run
  /// only declarations, so they run code only through their own static imports. An external
  /// import runs code. Any other chunk counts as one that runs code. A cycle of chunk imports adds
  /// nothing.
  fn chunk_runs_code_at_load(&mut self, chunk_idx: ChunkIdx) -> bool {
    if let Some(&result) = self.runs_code_at_load.get(&chunk_idx) {
      return result;
    }
    self.runs_code_at_load.insert(chunk_idx, false);
    let chunk_graph = self.chunk_graph;
    let link_output = self.link_output;
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    let runtime_idx = link_output.runtime.id();
    let is_lazy_host = matches!(chunk.kind, ChunkKind::EntryPoint { module, .. }
      if chunk_graph.lazy_dynamic_defer_host(module, &link_output.metas[module])
        == Some(chunk_idx));
    let runs_only_declarations = chunk.modules.iter().all(|&module_idx| {
      module_idx == runtime_idx
        || (is_lazy_host && !matches!(link_output.metas[module_idx].wrap_kind(), WrapKind::None))
    });
    let result = !runs_only_declarations
      || chunk.static_imports.iter().any(|import| match *import {
        StaticImport::External(_) => true,
        StaticImport::Chunk(importee_idx) => self.chunk_runs_code_at_load(importee_idx),
      });
    self.runs_code_at_load.insert(chunk_idx, result);
    result
  }
}
