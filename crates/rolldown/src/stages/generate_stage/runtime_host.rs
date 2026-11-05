//! Runtime placement: the runtime module stays in its own chunk until the final links exist. Then
//! one step can move it into a host chunk, when the move changes no load and no evaluation order.
//! See internal-docs/code-splitting/implementation.md#runtime-module-placement and
//! internal-docs/code-splitting/design.md#runtime-placement.

use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ChunkReasonType, EntryPointKind, ModuleIdx, OutputExports,
  OutputFormat, PathsOutputOption, PostChunkOptimizationOperation, PreserveEntrySignatures,
  RuntimeHelper, UsedSymbolRefsView, WrapKind,
};
use rolldown_utils::{IndexBitSet, indexmap::FxIndexSet};
use rustc_hash::FxHashSet;

use crate::chunk_graph::ChunkGraph;

use super::{
  FinalEsmInitMetadata, GenerateStage, Sealed,
  compute_cross_chunk_links::{ChunkImportOrder, CrossChunkLinkState},
  order_wrap_state::OrderWrapState,
};

/// A host for the runtime module, with the facts of the baseline that the check after the move
/// reads. The baseline is the final links with the runtime in its own chunk.
struct RuntimeFold {
  runtime_chunk: ChunkIdx,
  host: ChunkIdx,
  /// The chunks that import the runtime chunk in the baseline.
  consumers: FxHashSet<ChunkIdx>,
}

/// The state that `move_runtime_module` changes, so that the fall back can restore it.
struct RuntimeMove {
  runtime_chunk: ChunkIdx,
  host: ChunkIdx,
  host_helpers: RuntimeHelper,
  sorted_chunk_idx_vec: Vec<ChunkIdx>,
}

impl GenerateStage<'_> {
  /// Moves the runtime module into a host chunk when the host rule proves that the move changes
  /// no load and no evaluation order. `metadata` and `links` are the final links with the runtime
  /// in its own chunk. Returns the links of the final layout and the import order of each chunk.
  ///
  /// The import order after a move comes from the baseline, not from a sort: the runtime file
  /// leaves each list, and the host goes last in each list that did not import it. A check
  /// compares the links after the move with the baseline. If they differ, the runtime goes back
  /// into its own chunk.
  pub(super) fn place_runtime_module(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    used_symbol_refs: UsedSymbolRefsView<'_>,
    order_state: &OrderWrapState,
    metadata: Sealed<FinalEsmInitMetadata>,
    links: CrossChunkLinkState,
  ) -> (Sealed<FinalEsmInitMetadata>, CrossChunkLinkState, ChunkImportOrder) {
    let baseline = self.sort_chunk_imports(chunk_graph, &links);
    let Some(fold) = self.select_runtime_fold(chunk_graph, &links, &baseline) else {
      return (metadata, links, baseline);
    };
    let expected = expected_import_order(chunk_graph, &baseline, &fold);
    // Release the old tables before allocating their replacements.
    drop((metadata, links));
    let runtime_move = self.move_runtime_module(chunk_graph, fold.runtime_chunk, fold.host);
    let (metadata, links) =
      self.compute_final_chunk_links(chunk_graph, used_symbol_refs, order_state);
    if self.fold_keeps_baseline(chunk_graph, &links, &fold, &expected) {
      return (metadata, links, expected);
    }
    debug_assert!(
      false,
      "the runtime fold into chunk {:?} changes the links of the baseline",
      chunk_graph.chunk_table[fold.host].name
    );
    drop((metadata, links));
    self.restore_runtime_chunk(chunk_graph, runtime_move);
    let (metadata, links) =
      self.compute_final_chunk_links(chunk_graph, used_symbol_refs, order_state);
    let order = self.sort_chunk_imports(chunk_graph, &links);
    (metadata, links, order)
  }

  /// Selects the host of the runtime module, or returns `None` to keep the runtime file.
  fn select_runtime_fold(
    &self,
    chunk_graph: &ChunkGraph,
    links: &CrossChunkLinkState,
    baseline: &ChunkImportOrder,
  ) -> Option<RuntimeFold> {
    // Policy for the whole build. `preserveModules` keeps one file for each module, also for the
    // runtime. `experimentalInlineCommonChunks` registers registry demand on the files that read
    // records after this step, so the runtime stays in its own chunk while the feature is on.
    // Without code splitting, the runtime is already in the only chunk.
    if self.options.preserve_modules
      || self.options.inline_common_chunks.is_some()
      || self.options.code_splitting.is_disabled()
    {
      return None;
    }
    let runtime_chunk = self.movable_runtime_chunk(chunk_graph, links, baseline)?;
    let chunks = chunk_graph
      .chunk_table
      .indices()
      .filter(|&chunk_idx| chunk_idx != runtime_chunk && chunk_graph.chunk_is_live(chunk_idx))
      .collect::<Vec<_>>();
    let consumers = chunks
      .iter()
      .copied()
      .filter(|&chunk_idx| baseline.importees(chunk_idx).contains(&runtime_chunk))
      .collect::<FxHashSet<_>>();
    // With one other chunk, every root loads that chunk, and it imports no other chunk. The move
    // gives the layout of the same build without code splitting, which keeps the runtime in the
    // only chunk. So this case does not need the host rule or precondition 6.
    if let [host] = chunks[..] {
      return (!self.chunk_has_declaration_module(&chunk_graph.chunk_table[host]))
        .then_some(RuntimeFold { runtime_chunk, host, consumers });
    }
    // An unused runtime has no host: each root loads no consumer, so the root check rejects each
    // chunk.
    if consumers.is_empty() || !self.runtime_start_is_invisible() {
      return None;
    }
    let host = HostSearch::new(chunk_graph, baseline, runtime_chunk).select(
      self,
      chunk_graph,
      links,
      &consumers,
    )?;
    Some(RuntimeFold { runtime_chunk, host, consumers })
  }

  /// Preconditions 1, 2, 3 and 5: the runtime chunk is a chunk that only the static imports of its
  /// consumers load, and it holds nothing but the runtime module.
  fn movable_runtime_chunk(
    &self,
    chunk_graph: &ChunkGraph,
    links: &CrossChunkLinkState,
    baseline: &ChunkImportOrder,
  ) -> Option<ChunkIdx> {
    let runtime_idx = self.link_output.runtime.id();
    let runtime_chunk = chunk_graph.module_to_chunk[runtime_idx]?;
    let chunk = &chunk_graph.chunk_table[runtime_chunk];
    // 1. A fold must not remove an entry file.
    let not_an_entry = !self.link_output.entries.contains_key(&runtime_idx)
      && matches!(chunk.kind, ChunkKind::Common)
      && !chunk_graph.post_chunk_optimization_operations.contains_key(&runtime_chunk)
      && !chunk_graph.chunk_idx_to_reference_ids.contains_key(&runtime_chunk);
    // 2. The move takes one module, with no user code.
    let only_runtime = chunk.modules.as_slice() == [runtime_idx];
    // 3. A fold removes the file that a dynamic `import()` would load.
    let not_dynamically_imported = links
      .index_cross_chunk_dynamic_imports
      .iter()
      .all(|importees| !importees.contains(&runtime_chunk));
    // 5. The runtime file is a leaf, so no user chunk runs before it or inside it.
    let leaf = baseline.importees(runtime_chunk).is_empty();
    (not_an_entry && only_runtime && not_dynamically_imported && leaf).then_some(runtime_chunk)
  }

  /// Preconditions 4 and 6. After a fold, the runtime starts when the host starts, not before the
  /// first effect of each consumer. User code cannot see this change only for the runtime that
  /// rolldown generates: it declares helpers, and its only import is the built-in `node:module`.
  fn runtime_start_is_invisible(&self) -> bool {
    let runtime_idx = self.link_output.runtime.id();
    let module_table = &self.link_output.module_table;
    let Some(runtime) = module_table[runtime_idx].as_normal() else {
      return false;
    };
    // 4. The proof uses synchronous evaluation.
    if self.link_output.metas[runtime_idx].is_tla_or_contains_tla_dependency {
      return false;
    }
    // 6.1. A transform can make the start visible with no side effects: the runtime can read an
    // imported binding at its start, or wait for an import with top-level await.
    if !self.link_output.runtime.modified_by_plugins().is_empty() {
      return false;
    }
    // 6.2. The generated runtime has at most one import: `node:module` for ESM output on Node.
    // `resolveId` also runs on this import, so check the resolved module, not the specifier.
    let mut records = runtime.import_records.iter();
    let generated_imports = match (records.next(), records.next()) {
      (None, _) => true,
      (Some(record), None) => {
        record
          .resolved_module
          .and_then(|module_idx| module_table[module_idx].as_external())
          .is_some_and(|module| module.id.as_str() == "node:module")
          && !self.external_path_can_change("node:module")
      }
      (Some(_), Some(_)) => false,
    };
    // 6.3. A second guard: a `transform` hook can set `moduleSideEffects` with no code change.
    // `determine_side_effects` passes the effects of an imported module to its importers, but not
    // into a verdict that a plugin sets, so check each imported module too.
    generated_imports
      && !runtime.side_effects.has_side_effects()
      && runtime
        .import_records
        .iter()
        .filter_map(|record| record.resolved_module)
        .all(|module_idx| !module_table[module_idx].side_effects().has_side_effects())
  }

  /// `output.paths` can send an import of the external module `id` to another module after this
  /// step. A `paths` function runs only after this step, so treat it as a change.
  fn external_path_can_change(&self, id: &str) -> bool {
    match &self.options.paths {
      None => false,
      Some(PathsOutputOption::FxHashMap(paths)) => paths.contains_key(id),
      Some(PathsOutputOption::Fn(_)) => true,
    }
  }

  /// A chunk that can wait during its evaluation: it has a module with top-level await, or, in ESM
  /// output, it statically imports an external module that can wait. Rolldown cannot see the code
  /// of an external module. That code can use top-level await, and it can import a user entry, so
  /// it can also put the chunk in a cycle that the chunk graph does not show. Only a Node built-in
  /// module evaluates synchronously and imports no user module, and only when the output imports
  /// it by its own id. In CJS output, `require()` loads each external module synchronously.
  fn chunk_can_wait(
    &self,
    chunk_graph: &ChunkGraph,
    links: &CrossChunkLinkState,
    chunk_idx: ChunkIdx,
  ) -> bool {
    let chunk = &chunk_graph.chunk_table[chunk_idx];
    if chunk
      .modules
      .iter()
      .any(|&module_idx| self.link_output.metas[module_idx].is_tla_or_contains_tla_dependency)
    {
      return true;
    }
    if matches!(self.options.format, OutputFormat::Cjs) {
      return false;
    }
    let module_table = &self.link_output.module_table;
    let is_synchronous = |module_idx: ModuleIdx| {
      module_table[module_idx].as_external().is_some_and(|module| {
        module.id.as_str().starts_with("node:")
          && !self.external_path_can_change(module.id.as_str())
      })
    };
    // The links hold each static import of an external module, except one: for `export * from
    // 'ext'` below the entry level, the namespace object of the module imports `ext` inline. An
    // `import()` of an external module adds no wait.
    let star_exports = chunk.modules.iter().flat_map(|&module_idx| {
      let module = module_table[module_idx].as_normal();
      self.link_output.metas[module_idx]
        .star_exports_from_external_modules
        .iter()
        .filter_map(move |&rec_idx| module?.import_records[rec_idx].resolved_module)
    });
    links.index_chunk_direct_imports_from_external_modules[chunk_idx]
      .keys()
      .copied()
      .chain(links.index_chunk_indirect_imports_from_external_modules[chunk_idx].iter().copied())
      .chain(chunk.entry_level_external_module_idx.iter().copied())
      .chain(star_exports)
      .any(|module_idx| !is_synchronous(module_idx))
  }

  /// Declaration plugins discard generated runtime statements in `renderChunk`, so these chunks
  /// cannot host executable helpers.
  fn chunk_has_declaration_module(&self, chunk: &Chunk) -> bool {
    chunk.modules.iter().any(|&module_idx| {
      self.link_output.module_table[module_idx].as_normal().is_some_and(|module| {
        [".d.ts", ".d.mts", ".d.cts"].iter().any(|suffix| module.id.as_str().ends_with(suffix))
      })
    })
  }

  /// The policy for one host. The host rule proves that a fold keeps the evaluation order. These
  /// rules reject a fold that changes a contract of the output, or whose helper exports the
  /// consumers cannot read.
  fn host_policy_allows(
    &self,
    chunk_graph: &ChunkGraph,
    host: ChunkIdx,
    consumers: &FxHashSet<ChunkIdx>,
  ) -> bool {
    let chunk = &chunk_graph.chunk_table[host];
    if self.chunk_has_declaration_module(chunk) {
      return false;
    }
    // A host that is the only consumer keeps the helpers internal. Any other host exports them.
    if consumers.iter().all(|&consumer| consumer == host) {
      return true;
    }
    // The hash of a manual group must not change when the helpers of the app change.
    if matches!(chunk.chunk_reason_type.as_ref(), ChunkReasonType::ManualCodeSplitting { .. }) {
      return false;
    }
    // Keep async evaluation out of the rule.
    if chunk
      .modules
      .iter()
      .any(|&module_idx| self.link_output.metas[module_idx].is_tla_or_contains_tla_dependency)
    {
      return false;
    }
    let ChunkKind::EntryPoint { meta, module: entry_module_idx, .. } = chunk.kind else {
      return true;
    };
    if !meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk) {
      return true;
    }
    let entry_meta = &self.link_output.metas[entry_module_idx];
    let has_exports = !entry_meta.resolved_exports.is_empty() || entry_meta.has_dynamic_exports;
    // A fixed signature does not get the helper exports. `exports-only` fixes the signature of an
    // entry with exports.
    let fixed_signature = match chunk.preserve_entry_signature {
      Some(PreserveEntrySignatures::AllowExtension | PreserveEntrySignatures::False) => false,
      Some(PreserveEntrySignatures::Strict) => true,
      Some(PreserveEntrySignatures::ExportsOnly) | None => has_exports,
    };
    let cjs = matches!(self.options.format, OutputFormat::Cjs);
    // `output.exports` fixes the export names of each entry chunk, in each format: `none` allows no
    // export, and `default` allows only `default`. In CJS output, `auto` gives the `default` mode to
    // an entry that exports only a default. Then the helper exports change its `module.exports`
    // from that value into an object.
    let mode_rejects_helpers = match self.options.exports {
      OutputExports::None | OutputExports::Default => true,
      OutputExports::Auto => {
        cjs
          && !entry_meta.has_dynamic_exports
          && entry_meta.resolved_exports.len() == 1
          && entry_meta.resolved_exports.contains_key("default")
      }
      OutputExports::Named => false,
    };
    // In CJS output, `require()` of the host returns `module.exports`, and the helper exports go on
    // `exports`. A CommonJS entry controls both objects. Without a wrapper, its code runs at the top
    // level of the chunk and can replace either object, for example with `module.exports = {}`,
    // `exports = {}` or `require.main.exports = {}`. A wrapped entry ends with
    // `module.exports = require_x()`, except in the `named` mode.
    let detaches_exports = cjs
      && self.link_output.module_table[entry_module_idx]
        .as_normal()
        .is_some_and(|module| module.exports_kind.is_commonjs())
      && !(matches!(entry_meta.wrap_kind(), WrapKind::Cjs)
        && matches!(self.options.exports, OutputExports::Named));
    !fixed_signature && !mode_rejects_helpers && !detaches_exports
  }

  /// Moves the runtime module to the front of the host. The chunk order does not change: the host
  /// keeps its `exec_order`, and no sort runs again.
  fn move_runtime_module(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    runtime_chunk: ChunkIdx,
    host: ChunkIdx,
  ) -> RuntimeMove {
    let runtime_idx = self.link_output.runtime.id();
    let runtime_move = RuntimeMove {
      runtime_chunk,
      host,
      host_helpers: chunk_graph.chunk_table[host].depended_runtime_helper,
      sorted_chunk_idx_vec: chunk_graph.sorted_chunk_idx_vec.clone(),
    };
    let runtime_helpers = chunk_graph.chunk_table[runtime_chunk].depended_runtime_helper;
    chunk_graph.chunk_table[runtime_chunk].modules.clear();
    // `sort_chunk_modules` also puts the runtime module first.
    chunk_graph.chunk_table[host].modules.insert(0, runtime_idx);
    chunk_graph.chunk_table[host].depended_runtime_helper.insert(runtime_helpers);
    chunk_graph.module_to_chunk[runtime_idx] = Some(host);
    chunk_graph
      .post_chunk_optimization_operations
      .insert(runtime_chunk, PostChunkOptimizationOperation::Removed);
    chunk_graph.sorted_chunk_idx_vec.retain(|&chunk_idx| chunk_idx != runtime_chunk);
    self.clear_module_symbol_chunk_indices(runtime_idx);
    runtime_move
  }

  fn restore_runtime_chunk(&mut self, chunk_graph: &mut ChunkGraph, runtime_move: RuntimeMove) {
    let runtime_idx = self.link_output.runtime.id();
    let RuntimeMove { runtime_chunk, host, host_helpers, sorted_chunk_idx_vec } = runtime_move;
    let host_chunk = &mut chunk_graph.chunk_table[host];
    debug_assert_eq!(host_chunk.modules.first(), Some(&runtime_idx));
    host_chunk.modules.remove(0);
    host_chunk.depended_runtime_helper = host_helpers;
    chunk_graph.chunk_table[runtime_chunk].modules.push(runtime_idx);
    chunk_graph.module_to_chunk[runtime_idx] = Some(runtime_chunk);
    chunk_graph.post_chunk_optimization_operations.remove(&runtime_chunk);
    chunk_graph.sorted_chunk_idx_vec = sorted_chunk_idx_vec;
    self.clear_module_symbol_chunk_indices(runtime_idx);
  }

  /// The check after the move. It runs in release builds too, and it reads only the baseline.
  /// - Same edges: each chunk imports the chunks of its expected list, and no other chunk. This
  ///   finds a new load, a lost edge, and a difference between the derivation and the list.
  /// - No new consumer: each chunk that imports a helper from the host was a consumer in the
  ///   baseline. The host is not checked. The demand of its modules does not depend on the
  ///   layout, and the host reads the helpers locally, after the runtime at its front.
  ///
  /// The commit writes the expected lists without a sort, so the order needs no check here.
  fn fold_keeps_baseline(
    &self,
    chunk_graph: &ChunkGraph,
    links: &CrossChunkLinkState,
    fold: &RuntimeFold,
    expected: &ChunkImportOrder,
  ) -> bool {
    let runtime_idx = self.link_output.runtime.id();
    let mut importees = vec![];
    let mut expected_importees = vec![];
    chunk_graph.chunk_table.indices().filter(|&chunk_idx| chunk_graph.chunk_is_live(chunk_idx)).all(
      |chunk_idx| {
        importees.clear();
        importees.extend(links.static_importees(chunk_graph, chunk_idx));
        importees.sort_unstable();
        expected_importees.clear();
        expected_importees.extend_from_slice(expected.importees(chunk_idx));
        expected_importees.sort_unstable();
        importees == expected_importees
          && (chunk_idx == fold.host
            || fold.consumers.contains(&chunk_idx)
            || !links
              .imports_from_other_chunks(chunk_idx)
              .get(&fold.host)
              .is_some_and(|items| items.iter().any(|item| item.import_ref.owner == runtime_idx)))
      },
    )
  }
}

/// The import lists after the move, from the baseline (ordering rules 3 to 5): a consumer loses
/// the runtime file and gets the host last when it did not import the host. The host loses the
/// runtime file. Each other list stays the same.
fn expected_import_order(
  chunk_graph: &ChunkGraph,
  baseline: &ChunkImportOrder,
  fold: &RuntimeFold,
) -> ChunkImportOrder {
  ChunkImportOrder::new(
    chunk_graph
      .chunk_table
      .indices()
      .map(|chunk_idx| {
        let importees = baseline.importees(chunk_idx);
        if chunk_idx == fold.runtime_chunk {
          return vec![];
        }
        if !importees.contains(&fold.runtime_chunk) {
          return importees.to_vec();
        }
        let mut importees = importees
          .iter()
          .copied()
          .filter(|&importee| importee != fold.runtime_chunk)
          .collect::<Vec<_>>();
        if chunk_idx != fold.host && !importees.contains(&fold.host) {
          importees.push(fold.host);
        }
        importees
      })
      .collect(),
  )
}

/// The static chunk graph of the baseline, without the runtime chunk, and the walks of the host
/// rule over it.
struct HostSearch<'a> {
  baseline: &'a ChunkImportOrder,
  /// The live chunks other than the runtime chunk.
  nodes: IndexBitSet<ChunkIdx>,
  importers: IndexVec<ChunkIdx, Vec<ChunkIdx>>,
}

impl<'a> HostSearch<'a> {
  fn new(
    chunk_graph: &ChunkGraph,
    baseline: &'a ChunkImportOrder,
    runtime_chunk: ChunkIdx,
  ) -> Self {
    let chunk_count = chunk_graph.chunk_table.len();
    let mut nodes = IndexBitSet::new(chunk_count);
    for chunk_idx in chunk_graph.chunk_table.indices() {
      if chunk_idx != runtime_chunk && chunk_graph.chunk_is_live(chunk_idx) {
        nodes.set_bit(chunk_idx);
      }
    }
    let mut search = Self { baseline, nodes, importers: index_vec![vec![]; chunk_count] };
    for chunk_idx in chunk_graph.chunk_table.indices() {
      if search.nodes.has_bit(chunk_idx) {
        for importee in search.importees(chunk_idx).collect::<Vec<_>>() {
          search.importers[importee].push(chunk_idx);
        }
      }
    }
    search
  }

  fn importees(&self, chunk_idx: ChunkIdx) -> impl Iterator<Item = ChunkIdx> + '_ {
    self
      .baseline
      .importees(chunk_idx)
      .iter()
      .copied()
      .filter(|&importee| self.nodes.has_bit(importee))
  }

  /// The chunks that a walk from `start` reaches, `start` included. `forward` follows the imports,
  /// and otherwise the walk follows the importers.
  fn walk(
    &self,
    start: impl IntoIterator<Item = ChunkIdx>,
    forward: bool,
  ) -> IndexBitSet<ChunkIdx> {
    let mut seen = IndexBitSet::new(self.importers.len());
    let mut stack = start.into_iter().collect::<Vec<_>>();
    while let Some(chunk_idx) = stack.pop() {
      if seen.has_bit(chunk_idx) {
        continue;
      }
      seen.set_bit(chunk_idx);
      if forward {
        stack.extend(self.importees(chunk_idx));
      } else {
        stack.extend(self.importers[chunk_idx].iter().copied());
      }
    }
    seen
  }

  /// The chunks that are in a chunk cycle: in a strongly connected component with two or more
  /// chunks. An iterative Tarjan walk, O(V + E).
  fn chunks_in_cycles(&self) -> IndexBitSet<ChunkIdx> {
    const UNVISITED: u32 = u32::MAX;
    let chunk_count = self.importers.len();
    let mut index = vec![UNVISITED; chunk_count];
    let mut low = vec![0; chunk_count];
    let mut on_stack = IndexBitSet::new(chunk_count);
    let mut stack = vec![];
    let mut in_cycle = IndexBitSet::new(chunk_count);
    let mut next_index = 0;
    for root in (0..chunk_count).map(ChunkIdx::from_usize) {
      if !self.nodes.has_bit(root) || index[root.index()] != UNVISITED {
        continue;
      }
      // Each frame has a chunk and the position of its next importee.
      let mut frames = vec![(root, 0)];
      index[root.index()] = next_index;
      low[root.index()] = next_index;
      next_index += 1;
      stack.push(root);
      on_stack.set_bit(root);
      while let Some(frame) = frames.last_mut() {
        let chunk_idx = frame.0;
        let importees = self.baseline.importees(chunk_idx);
        if let Some(&importee) = importees.get(frame.1) {
          frame.1 += 1;
          if !self.nodes.has_bit(importee) {
            continue;
          }
          if index[importee.index()] == UNVISITED {
            index[importee.index()] = next_index;
            low[importee.index()] = next_index;
            next_index += 1;
            stack.push(importee);
            on_stack.set_bit(importee);
            frames.push((importee, 0));
          } else if on_stack.has_bit(importee) {
            low[chunk_idx.index()] = low[chunk_idx.index()].min(index[importee.index()]);
          }
          continue;
        }
        frames.pop();
        if let Some(&(importer, _)) = frames.last() {
          low[importer.index()] = low[importer.index()].min(low[chunk_idx.index()]);
        }
        if low[chunk_idx.index()] == index[chunk_idx.index()] {
          let start = stack.iter().rposition(|&member| member == chunk_idx).expect("on the stack");
          let component = stack.split_off(start);
          for &member in &component {
            on_stack.clear_bit(member);
          }
          if component.len() > 1 {
            for member in component {
              in_cycle.set_bit(member);
            }
          }
        }
      }
    }
    in_cycle
  }

  /// The host rule, the root check and the policy filter on the baseline. A chunk `H` can host
  /// the runtime only if:
  /// 1. no consumer is in a chunk cycle,
  /// 2. `H` is a static dependency of each other consumer, and
  /// 3. no consumer is a static dependency of `H`.
  ///
  /// When no chunk passes, the lazy consumer rule replaces condition 2 (see
  /// `select_with_lazy_consumers`). The choice: the fewest added edges, then the chunk order, then
  /// the chunk index.
  fn select(
    &self,
    stage: &GenerateStage<'_>,
    chunk_graph: &ChunkGraph,
    links: &CrossChunkLinkState,
    consumers: &FxHashSet<ChunkIdx>,
  ) -> Option<ChunkIdx> {
    // Condition 1.
    let in_cycle = self.chunks_in_cycles();
    if consumers.iter().any(|&consumer| in_cycle.has_bit(consumer)) {
      return None;
    }
    // Condition 3: `forbidden` holds each chunk that reaches a consumer through one import or more.
    let forbidden = self
      .walk(consumers.iter().flat_map(|&consumer| self.importers[consumer].iter().copied()), false);

    // The root check: a root that loads no consumer must not load the host. A root is an entry or
    // the target of a dynamic import.
    let roots = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(chunk_idx, chunk)| {
        self.nodes.has_bit(*chunk_idx) && matches!(chunk.kind, ChunkKind::EntryPoint { .. })
      })
      .map(|(chunk_idx, _)| chunk_idx)
      .chain(
        links
          .index_cross_chunk_dynamic_imports
          .iter()
          .flatten()
          .copied()
          .filter(|&chunk_idx| self.nodes.has_bit(chunk_idx)),
      );
    let cold_reach =
      self.walk(roots.filter(|&root| !consumers.contains(&root) && !forbidden.has_bit(root)), true);

    // The number of consumers that import each chunk directly.
    let mut direct = index_vec![0usize; self.importers.len()];
    for &consumer in consumers {
      for importee in self.importees(consumer) {
        direct[importee] += 1;
      }
    }
    // The candidates that pass the root check and the policy, in the order of the choice.
    let hosts = |candidates: &IndexBitSet<ChunkIdx>| {
      let mut hosts = candidates
        .index_of_one()
        .filter(|&host| !cold_reach.has_bit(host))
        .filter(|&host| stage.host_policy_allows(chunk_graph, host, consumers))
        .collect::<Vec<_>>();
      hosts.sort_unstable_by_key(|&host| {
        let others = consumers.len() - usize::from(consumers.contains(&host));
        (others - direct[host], chunk_graph.chunk_table[host].exec_order, host)
      });
      hosts
    };

    // Condition 2. A consumer that reaches no other consumer is a minimal consumer. Each other
    // consumer reaches a minimal one, because no consumer is in a cycle. So a chunk that each
    // minimal consumer reaches is also a static dependency of each other consumer. None of these
    // chunks reaches a consumer: then the minimal consumer that reaches it would too.
    let minimal = consumers.iter().copied().filter(|&consumer| !forbidden.has_bit(consumer));
    if let Some(host) =
      self.common_dependencies(minimal).and_then(|candidates| hosts(&candidates).first().copied())
    {
      return Some(host);
    }
    self.select_with_lazy_consumers(stage, chunk_graph, links, consumers, &forbidden, hosts)
  }

  /// The candidates of condition 2 for `minimal`, the consumers that reach no other consumer: the
  /// chunks that each of them reaches through one import or more. A sole minimal consumer can also
  /// host the runtime itself. Returns `None` when no chunk passes.
  fn common_dependencies(
    &self,
    minimal: impl Iterator<Item = ChunkIdx>,
  ) -> Option<IndexBitSet<ChunkIdx>> {
    let mut minimal = minimal.collect::<Vec<_>>();
    minimal.sort_unstable();
    let (&first, rest) = minimal.split_first()?;
    let mut candidates = if rest.is_empty() {
      self.walk([first], true)
    } else {
      self.walk(self.importees(first), true)
    };
    for &consumer in rest {
      candidates = self.reached_candidates(consumer, &candidates)?;
    }
    (!candidates.is_empty()).then_some(candidates)
  }

  /// The lazy consumer rule. It replaces condition 2 when no chunk passes condition 2. A consumer
  /// that does not reach `H` can import `H` when each load of the consumer starts after `H`
  /// completes. Then the added edge loads nothing and runs nothing: `H` already ran.
  ///
  /// The chunks that start after `H` completes are the largest set of chunks `X` with these
  /// properties:
  /// - `X` is not `H`, not a user entry and not an emitted chunk.
  /// - Each static importer of `X` is in the set.
  /// - Each chunk with an `import()` of `X` is `H` or is in the set.
  ///
  /// Proof, by induction over the start of each evaluation whose root is in the set: the root is
  /// not an entry, so an `import()` in a chunk `D` starts the evaluation. If `D` is `H`, the target
  /// of the `import()` starts in a later job than the code of `H`: a promise job in ESM output,
  /// `Promise.resolve().then()` in CJS output. If `D` is in the set, `D` started after `H`
  /// completed. A chunk of the set is part of only these evaluations, because its static importers
  /// are in the set too.
  ///
  /// The proof also holds when `D` only reaches `H`, but the rule does not accept that case. Then
  /// `H` is not the chunk that loads the consumer, and `cjs_reexport_barrel_namespace_routing`
  /// asserts that such a lazy route does not import `H`.
  ///
  /// The proof needs three more facts:
  /// - No chunk that `H` reaches can wait (see `chunk_can_wait`). Then each cycle that contains `H`
  ///   is in the static closure of `H`, and no module in that closure waits. So `H` completes
  ///   before the end of each job that runs code of `H`, also when `H` is in a chunk cycle. A
  ///   module that waits can keep a cycle that contains `H` unfinished after the code of `H` ran.
  ///   ESM then makes each new importer of `H` wait for that cycle, and the cycle can pass through
  ///   an external module that imports a user entry.
  /// - No module throws during its evaluation. If `H` throws after an `import()`, the lazy
  ///   consumer now fails with the error of `H`.
  /// - The entries and the `import()` edges show each load of a chunk.
  fn select_with_lazy_consumers(
    &self,
    stage: &GenerateStage<'_>,
    chunk_graph: &ChunkGraph,
    links: &CrossChunkLinkState,
    consumers: &FxHashSet<ChunkIdx>,
    forbidden: &IndexBitSet<ChunkIdx>,
    hosts: impl Fn(&IndexBitSet<ChunkIdx>) -> Vec<ChunkIdx>,
  ) -> Option<ChunkIdx> {
    // Dev mode also loads chunks through the dev runtime. Precondition 6 rejects dev builds too,
    // because the HMR plugin changes the runtime, but this rule must not depend on that fact.
    if stage.options.is_dev_mode_enabled() {
      return None;
    }
    // An `import()` of a chunk that is not live hides a load.
    let dynamic_importees = &links.index_cross_chunk_dynamic_imports;
    if self.nodes.index_of_one().any(|importer| {
      dynamic_importees[importer].iter().any(|&importee| !self.nodes.has_bit(importee))
    }) {
      return None;
    }
    let entries = self.entry_chunks(stage, chunk_graph);
    // An entry loads each chunk that it reaches, so these chunks are never in the set. `H` must
    // then be a static dependency of each consumer among them: condition 2 for these consumers.
    let eager = self.walk(entries.iter().copied(), true);
    let minimal = consumers
      .iter()
      .copied()
      .filter(|&consumer| eager.has_bit(consumer) && !forbidden.has_bit(consumer));
    let candidates = self.common_dependencies(minimal)?;
    let reaches_wait = self.walk(
      self
        .nodes
        .index_of_one()
        .filter(|&chunk_idx| stage.chunk_can_wait(chunk_graph, links, chunk_idx)),
      false,
    );
    hosts(&candidates).into_iter().filter(|&host| !reaches_wait.has_bit(host)).find(|&host| {
      let reaches_host = self.walk([host], false);
      let before_host = self.start_before_host(&entries, host, dynamic_importees);
      consumers.iter().all(|&consumer| {
        consumer == host || reaches_host.has_bit(consumer) || !before_host.has_bit(consumer)
      })
    })
  }

  /// The chunks that code outside the bundle can load: the user entries, the emitted chunks, and
  /// the chunks that hold the module of one of them.
  fn entry_chunks(&self, stage: &GenerateStage<'_>, chunk_graph: &ChunkGraph) -> Vec<ChunkIdx> {
    let mut entries = self
      .nodes
      .index_of_one()
      .filter(|chunk_idx| {
        matches!(
          chunk_graph.chunk_table[*chunk_idx].kind,
          ChunkKind::EntryPoint { meta, .. }
            if meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk)
        ) || chunk_graph.chunk_idx_to_reference_ids.contains_key(chunk_idx)
      })
      .collect::<Vec<_>>();
    for (&module_idx, entry_points) in &stage.link_output.entries {
      if entry_points.iter().any(|entry| !matches!(entry.kind, EntryPointKind::DynamicImport)) {
        entries.extend(chunk_graph.entry_module_to_entry_chunk.get(&module_idx).copied());
        entries.extend(chunk_graph.module_to_chunk[module_idx]);
      }
    }
    entries.retain(|&chunk_idx| self.nodes.has_bit(chunk_idx));
    entries
  }

  /// The chunks that can start before `host` completes: the complement of the set of the lazy
  /// consumer rule. A walk from the entries and `host` follows the static imports of each chunk,
  /// and the `import()` targets of each chunk other than `host`.
  fn start_before_host(
    &self,
    entries: &[ChunkIdx],
    host: ChunkIdx,
    dynamic_importees: &IndexVec<ChunkIdx, FxIndexSet<ChunkIdx>>,
  ) -> IndexBitSet<ChunkIdx> {
    let mut seen = IndexBitSet::new(self.importers.len());
    let mut stack = entries.iter().copied().chain([host]).collect::<Vec<_>>();
    while let Some(chunk_idx) = stack.pop() {
      if seen.has_bit(chunk_idx) {
        continue;
      }
      seen.set_bit(chunk_idx);
      stack.extend(self.importees(chunk_idx));
      if chunk_idx != host {
        stack.extend(dynamic_importees[chunk_idx].iter().copied());
      }
    }
    seen
  }

  /// The candidates that a walk from the importees of `consumer` reaches. Returns `None` when it
  /// reaches none. The walk stops when it reaches all candidates.
  fn reached_candidates(
    &self,
    consumer: ChunkIdx,
    candidates: &IndexBitSet<ChunkIdx>,
  ) -> Option<IndexBitSet<ChunkIdx>> {
    let mut remaining = candidates.bit_count();
    let mut reached = IndexBitSet::new(self.importers.len());
    let mut seen = IndexBitSet::new(self.importers.len());
    let mut stack = self.importees(consumer).collect::<Vec<_>>();
    while remaining > 0
      && let Some(chunk_idx) = stack.pop()
    {
      if seen.has_bit(chunk_idx) {
        continue;
      }
      seen.set_bit(chunk_idx);
      if candidates.has_bit(chunk_idx) {
        reached.set_bit(chunk_idx);
        remaining -= 1;
      }
      stack.extend(self.importees(chunk_idx));
    }
    (!reached.is_empty()).then_some(reached)
  }
}
