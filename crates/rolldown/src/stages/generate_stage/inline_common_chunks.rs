//! `codeSplitting.inlineCommonChunks`: replace small automatic common chunks with factory
//! definitions placed in their consumers, linked at runtime through one shared registry.
//!
//! See `internal-docs/inline-common-chunks/design.md` for the model and its limits.
//!
//! The pass runs after `compute_cross_chunk_links`, because it needs the final static chunk import
//! graph and the symbol-to-chunk table, and before deconfliction, because a host chunk must reserve
//! the names an inlined chunk's body already uses.

use rolldown_common::{Chunk, ChunkIdx, ChunkKind, ChunkReasonType};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

/// The registry binding names the generated code refers to. They are reserved in every chunk's
/// renamer while the feature is on, so a user symbol of the same name is renamed instead.
pub const SHARE_DEFINE_NAME: &str = "__rd_share";
pub const SHARE_REQUIRE_NAME: &str = "__rd_share_require";

#[derive(Debug, Default)]
pub struct InlineCommonChunksPlan {
  /// Why the feature selected nothing, when it was asked to.
  pub disabled_reason: Option<&'static str>,
  /// Chunks replaced by factory placements, in evaluation order.
  pub inlined: Vec<ChunkIdx>,
  /// The live chunk that carries the runtime module, which also carries the registry.
  pub registry_chunk: Option<ChunkIdx>,
  pub stats: InlineCommonChunksStats,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct InlineCommonChunksStats {
  pub candidate_chunks: u32,
  pub selected_chunks: u32,
  pub rejected_too_large: u32,
  pub rejected_top_level_await: u32,
  pub rejected_dynamically_imported: u32,
  pub rejected_chunk_relative_path: u32,
  pub rejected_emitted: u32,
  pub rejected_no_consumer: u32,
  pub rejected_no_carrier: u32,
  pub rejected_reexported: u32,
  pub chunks_in_static_cycles: u32,
  pub factory_placements: u32,
  pub placements_removed_by_elimination: u32,
}

impl InlineCommonChunksPlan {
  pub fn is_active(&self) -> bool {
    !self.inlined.is_empty()
  }
}

impl GenerateStage<'_> {
  /// Selects the common chunks the feature may replace, decides where their factories go, and
  /// rewires the chunk graph so no chunk imports an inlined chunk as a file any more.
  pub(super) fn plan_inline_common_chunks(
    &mut self,
    chunk_graph: &mut ChunkGraph,
  ) -> InlineCommonChunksPlan {
    let plan = self.select_and_place_inline_common_chunks(chunk_graph);
    // Written on every exit, including the ones that select nothing, so a cell that asked for the
    // feature and got none of it is distinguishable from a stale ledger left by an earlier run.
    if self.options.inline_common_chunks_max_size() > 0 {
      self.write_inline_common_chunks_ledger(chunk_graph, &plan);
    }
    plan
  }

  fn select_and_place_inline_common_chunks(
    &mut self,
    chunk_graph: &mut ChunkGraph,
  ) -> InlineCommonChunksPlan {
    let max_size = f64::from(self.options.inline_common_chunks_max_size());
    if max_size <= 0.0 {
      return InlineCommonChunksPlan::default();
    }
    if !matches!(self.options.format, rolldown_common::OutputFormat::Esm) {
      // Every other format resolves cross-chunk references through its own binding shape; the
      // factory interface is only defined for ESM output here.
      return InlineCommonChunksPlan { disabled_reason: Some("format is not esm"), ..Default::default() };
    }
    if !self.options.is_strict_execution_order_enabled() {
      // A carrier registers and executes a factory in its own body, which ESM runs after every
      // chunk it statically imports. Without order wrapping an inlined chunk's body therefore moves
      // after chunks that used to evaluate later, which is an observable reordering. Under strict
      // execution order every body is already deferred behind an `init_*` wrapper, so the factory
      // only defines and the order plan still decides when anything runs.
      return InlineCommonChunksPlan {
        disabled_reason: Some("strictExecutionOrder is off"),
        ..Default::default()
      };
    }

    let runtime_module = self.link_output.runtime.id();
    let Some(registry_chunk) = chunk_graph.module_to_chunk[runtime_module] else {
      return InlineCommonChunksPlan { disabled_reason: Some("no runtime chunk"), ..Default::default() };
    };
    if !chunk_graph.chunk_is_live(registry_chunk) {
      return InlineCommonChunksPlan { disabled_reason: Some("no runtime chunk"), ..Default::default() };
    }

    let live: Vec<ChunkIdx> = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter(|(idx, _)| chunk_graph.chunk_is_live(*idx))
      .map(|(idx, _)| idx)
      .collect();

    let mut dynamically_imported: FxHashSet<ChunkIdx> = FxHashSet::default();
    for &idx in &live {
      for target in &chunk_graph.chunk_table[idx].cross_chunk_dynamic_imports {
        dynamically_imported.insert(*target);
      }
    }
    let emitted: FxHashSet<ChunkIdx> =
      chunk_graph.chunk_idx_to_reference_ids.keys().copied().collect();

    // A chunk that re-exports a symbol owned by an inlined chunk can only bind it once, because ESM
    // cannot re-export a property read. That would silently turn a live binding into a snapshot, so
    // the owner is not a candidate.
    let mut reexported_owners: FxHashSet<ChunkIdx> = FxHashSet::default();
    for &idx in &live {
      for symbol_ref in chunk_graph.chunk_table[idx].exports_to_other_chunks.keys() {
        let canonical = self.link_output.symbol_db.canonical_ref_for(*symbol_ref);
        if let Some(owner) = self.link_output.symbol_db.get(canonical).chunk_idx
          && owner != idx
        {
          reexported_owners.insert(owner);
        }
      }
    }

    let mut stats = InlineCommonChunksStats::default();
    let mut selected: FxHashSet<ChunkIdx> = FxHashSet::default();
    for &idx in &live {
      let chunk = &chunk_graph.chunk_table[idx];
      if idx == registry_chunk {
        // The registry must stay one instance per realm. A copy of it would fork the factory and
        // module tables, which is the one property the whole mechanism depends on.
        continue;
      }
      if !matches!(chunk.kind, ChunkKind::Common) {
        continue;
      }
      if !matches!(*chunk.chunk_reason_type, ChunkReasonType::Common) {
        // Manual code-splitting group chunks and their `maxSize` splits are user-directed output,
        // outside this feature's scope.
        continue;
      }
      stats.candidate_chunks += 1;
      if emitted.contains(&idx) {
        stats.rejected_emitted += 1;
        continue;
      }
      if dynamically_imported.contains(&idx) {
        stats.rejected_dynamically_imported += 1;
        continue;
      }
      if !chunk.cross_chunk_dynamic_imports.is_empty() || self.chunk_emits_chunk_relative_url(chunk)
      {
        // A factory body is rendered once and printed into hosts that can sit in different output
        // directories, so it must not contain a chunk-relative path. Dynamic import specifiers and
        // resolved file URLs are exactly the two constructs that do.
        stats.rejected_chunk_relative_path += 1;
        continue;
      }
      if reexported_owners.contains(&idx) {
        stats.rejected_reexported += 1;
        continue;
      }
      if self.chunk_has_top_level_await(chunk) {
        // The RFC's first policy does not select a chunk containing top-level await: the factory
        // interface is synchronous.
        stats.rejected_top_level_await += 1;
        continue;
      }
      if self.chunk_pre_render_size(chunk) > max_size {
        stats.rejected_too_large += 1;
        continue;
      }
      selected.insert(idx);
    }

    if selected.is_empty() {
      return InlineCommonChunksPlan {
        stats,
        disabled_reason: Some("no chunk was selectable"),
        ..InlineCommonChunksPlan::default()
      };
    }

    // Static import edges over live chunks, importer -> importees.
    let mut imports: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
    for &idx in &live {
      let targets: Vec<ChunkIdx> = chunk_graph.chunk_table[idx]
        .cross_chunk_imports
        .iter()
        .copied()
        .filter(|target| chunk_graph.chunk_is_live(*target))
        .collect();
      imports.insert(idx, targets);
    }

    // A selected chunk with no importer would simply disappear; keep it as a file instead.
    let mut has_importer: FxHashSet<ChunkIdx> = FxHashSet::default();
    for targets in imports.values() {
      for target in targets {
        has_importer.insert(*target);
      }
    }
    selected.retain(|idx| {
      if has_importer.contains(idx) {
        true
      } else {
        stats.rejected_no_consumer += 1;
        false
      }
    });
    if selected.is_empty() {
      return InlineCommonChunksPlan {
        stats,
        disabled_reason: Some("no chunk was selectable"),
        ..InlineCommonChunksPlan::default()
      };
    }

    // `reach(X)` is every selected chunk X pulls in through chains of selected chunks. A host that
    // carries a factory must also carry everything that factory executes.
    let mut reach: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> = FxHashMap::default();
    for &idx in &live {
      let mut result: FxHashSet<ChunkIdx> = FxHashSet::default();
      let mut pending: Vec<ChunkIdx> =
        imports[&idx].iter().copied().filter(|target| selected.contains(target)).collect();
      while let Some(current) = pending.pop() {
        if !result.insert(current) {
          continue;
        }
        for target in &imports[&current] {
          if selected.contains(target) {
            pending.push(*target);
          }
        }
      }
      reach.insert(idx, result);
    }

    // Evaluation order: an ESM importee runs before its importer, so a chunk's static import
    // closure is exactly what is guaranteed to have run when its own body starts. Walking chunks in
    // that order lets a chunk drop a factory a dependency already registered.
    // ESM evaluation order over a cyclic chunk graph: strongly connected components form a DAG, and
    // every module of a strictly lower component finishes before any module of a higher one starts.
    // Inheriting only across component boundaries is therefore sound, while inheriting inside a
    // component is not — which member runs first there depends on which root the loader entered from.
    let components = strongly_connected_components(&live, &imports);
    let mut component_of: FxHashMap<ChunkIdx, usize> = FxHashMap::default();
    for (index, component) in components.iter().enumerate() {
      for idx in component {
        component_of.insert(*idx, index);
      }
    }
    stats.chunks_in_static_cycles = u32::try_from(
      components.iter().filter(|component| component.len() > 1).map(Vec::len).sum::<usize>(),
    )
    .unwrap_or(u32::MAX);

    let mut carried: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
    let mut available: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> = FxHashMap::default();
    for (component_index, component) in components.iter().enumerate() {
      let mut inherited_by_member: Vec<FxHashSet<ChunkIdx>> = Vec::with_capacity(component.len());
      for idx in component {
        let mut inherited: FxHashSet<ChunkIdx> = FxHashSet::default();
        for dependency in &imports[idx] {
          if component_of[dependency] == component_index || selected.contains(dependency) {
            continue;
          }
          if let Some(from_dependency) = available.get(dependency) {
            inherited.extend(from_dependency.iter().copied());
          }
          if let Some(carried_by_dependency) = carried.get(dependency) {
            inherited.extend(carried_by_dependency.iter().copied());
          }
        }
        inherited_by_member.push(inherited);
      }
      for (idx, inherited) in component.iter().zip(inherited_by_member) {
        if selected.contains(idx) {
          available.insert(*idx, FxHashSet::default());
          continue;
        }
        let mut needed: Vec<ChunkIdx> = reach[idx]
          .iter()
          .copied()
          .filter(|target| {
            if inherited.contains(target) {
              stats.placements_removed_by_elimination += 1;
              false
            } else {
              true
            }
          })
          .collect();
        needed.sort_unstable_by_key(|target| chunk_graph.chunk_table[*target].exec_order);
        if !needed.is_empty() {
          carried.insert(*idx, needed);
        }
        available.insert(*idx, inherited);
      }
    }

    // Safety net: a selected chunk nothing carries would disappear from the build. Keep it as a
    // file instead of trusting the placement sweep to have covered every consumer.
    let mut with_carrier: FxHashSet<ChunkIdx> = FxHashSet::default();
    for targets in carried.values() {
      for target in targets {
        with_carrier.insert(*target);
      }
    }
    let uncarried: Vec<ChunkIdx> =
      selected.iter().copied().filter(|idx| !with_carrier.contains(idx)).collect();
    stats.rejected_no_carrier = u32::try_from(uncarried.len()).unwrap_or(u32::MAX);
    for idx in &uncarried {
      selected.remove(idx);
    }
    if !uncarried.is_empty() {
      for targets in carried.values_mut() {
        targets.retain(|target| selected.contains(target));
      }
      carried.retain(|_, targets| !targets.is_empty());
    }
    if selected.is_empty() {
      return InlineCommonChunksPlan {
        stats,
        disabled_reason: Some("no chunk was selectable"),
        ..InlineCommonChunksPlan::default()
      };
    }

    let mut inlined: Vec<ChunkIdx> = selected.iter().copied().collect();
    inlined.sort_unstable_by_key(|idx| chunk_graph.chunk_table[*idx].exec_order);
    stats.selected_chunks = u32::try_from(inlined.len()).unwrap_or(u32::MAX);
    for (share_id, idx) in inlined.iter().enumerate() {
      chunk_graph.chunk_table[*idx].inline_share_id =
        Some(u32::try_from(share_id).unwrap_or(u32::MAX));
    }

    for (host, mut targets) in carried {
      targets.sort_unstable_by_key(|idx| chunk_graph.chunk_table[*idx].exec_order);
      stats.factory_placements += u32::try_from(targets.len()).unwrap_or(0);
      chunk_graph.chunk_table[host].carried_inline_chunks = targets;
    }

    // Every chunk that statically imported an inlined chunk must still execute it. Chains between
    // inlined chunks are executed from inside the consuming factory.
    for &idx in &live {
      let mut required: Vec<ChunkIdx> =
        imports[&idx].iter().copied().filter(|target| selected.contains(target)).collect();
      required.sort_unstable_by_key(|target| chunk_graph.chunk_table[*target].exec_order);
      if !required.is_empty() {
        chunk_graph.chunk_table[idx].required_inline_chunks = required;
      }
    }

    self.rewire_chunk_imports(chunk_graph, &live, &selected, registry_chunk);

    InlineCommonChunksPlan {
      disabled_reason: None,
      inlined,
      registry_chunk: Some(registry_chunk),
      stats,
    }
  }

  /// After the plan, no chunk may import an inlined chunk as a file. A host takes over the imports
  /// the inlined chunk needed for its own body, and every chunk that touches the registry imports
  /// the chunk that defines it.
  fn rewire_chunk_imports(
    &self,
    chunk_graph: &mut ChunkGraph,
    live: &[ChunkIdx],
    selected: &FxHashSet<ChunkIdx>,
    registry_chunk: ChunkIdx,
  ) {
    // Collected first because a host reads the inlined chunks' import lists while its own is
    // rewritten.
    let inlined_imports: FxHashMap<ChunkIdx, (Vec<ChunkIdx>, Vec<ChunkIdx>)> = selected
      .iter()
      .map(|idx| {
        let chunk = &chunk_graph.chunk_table[*idx];
        (
          *idx,
          (chunk.cross_chunk_imports.clone(), chunk.cross_chunk_dynamic_imports.clone()),
        )
      })
      .collect();

    for &idx in live {
      if selected.contains(&idx) {
        // An inlined chunk reaches another one through the registry, never through a file import.
        let chunk = &mut chunk_graph.chunk_table[idx];
        chunk.imports_from_other_chunks.retain(|target, _| !selected.contains(target));
        chunk.cross_chunk_imports.retain(|target| !selected.contains(target));
        continue;
      }
      let carried = chunk_graph.chunk_table[idx].carried_inline_chunks.clone();
      let touches_registry =
        !carried.is_empty() || !chunk_graph.chunk_table[idx].required_inline_chunks.is_empty();

      let mut static_imports: Vec<ChunkIdx> = Vec::new();
      let mut dynamic_imports = chunk_graph.chunk_table[idx].cross_chunk_dynamic_imports.clone();
      let mut seen: FxHashSet<ChunkIdx> = FxHashSet::default();
      let push_static = |target: ChunkIdx,
                             seen: &mut FxHashSet<ChunkIdx>,
                             out: &mut Vec<ChunkIdx>| {
        if target != idx && seen.insert(target) {
          out.push(target);
        }
      };

      for target in chunk_graph.chunk_table[idx].cross_chunk_imports.clone() {
        if selected.contains(&target) {
          continue;
        }
        push_static(target, &mut seen, &mut static_imports);
      }
      // A carried factory renders inside this chunk, so its own dependencies become this chunk's.
      for carried_idx in &carried {
        let (statics, dynamics) = &inlined_imports[carried_idx];
        for target in statics {
          if selected.contains(target) {
            continue;
          }
          push_static(*target, &mut seen, &mut static_imports);
        }
        for target in dynamics {
          if !dynamic_imports.contains(target) {
            dynamic_imports.push(*target);
          }
        }
      }
      if touches_registry {
        push_static(registry_chunk, &mut seen, &mut static_imports);
      }

      static_imports
        .sort_unstable_by_key(|target| chunk_graph.chunk_table[*target].exec_order);
      let chunk = &mut chunk_graph.chunk_table[idx];
      chunk.cross_chunk_imports = static_imports;
      chunk.cross_chunk_dynamic_imports = dynamic_imports;
      // The host no longer imports the inlined chunk's symbols as bindings; the module finalizer
      // rewrites those references to `<binding>.<export>` instead.
      chunk.imports_from_other_chunks.retain(|target, _| !selected.contains(target));
    }
  }

  fn chunk_pre_render_size(&self, chunk: &Chunk) -> f64 {
    // The same unit `codeSplitting.maxSize` uses for manual groups: the module's source size, the
    // only size that exists while chunks are still being placed.
    #[expect(clippy::cast_precision_loss)]
    chunk
      .modules
      .iter()
      .map(|idx| self.link_output.module_table[*idx].size() as f64)
      .sum::<f64>()
  }

  /// Research hook: `ROLLDOWN_INLINE_COMMON_CHUNKS_LEDGER=<path>` writes what the pass selected and
  /// where it placed each factory, so selection, placement, and elimination can be measured apart
  /// from the emitted artifact.
  fn write_inline_common_chunks_ledger(
    &self,
    chunk_graph: &ChunkGraph,
    plan: &InlineCommonChunksPlan,
  ) {
    let Ok(path) = std::env::var("ROLLDOWN_INLINE_COMMON_CHUNKS_LEDGER") else { return };
    let disabled_reason = plan
      .disabled_reason
      .map(|reason| format!("{reason:?}"))
      .unwrap_or_else(|| "null".to_string());
    let mut chunks = String::new();
    for idx in &plan.inlined {
      let chunk = &chunk_graph.chunk_table[*idx];
      let carriers = chunk_graph
        .chunk_table
        .iter()
        .filter(|host| host.carried_inline_chunks.contains(idx))
        .count();
      if !chunks.is_empty() {
        chunks.push_str(",\n");
      }
      let module_ids = chunk
        .modules
        .iter()
        .map(|module_idx| format!("{:?}", self.link_output.module_table[*module_idx].id().as_str()))
        .collect::<Vec<_>>()
        .join(", ");
      chunks.push_str(&format!(
        "    {{\"shareId\": {}, \"name\": {:?}, \"moduleCount\": {}, \"preRenderSize\": {}, \"carriers\": {}, \"modules\": [{}]}}",
        chunk.inline_share_id.unwrap_or_default(),
        chunk.name.as_deref().unwrap_or(""),
        chunk.modules.len(),
        self.chunk_pre_render_size(chunk),
        carriers,
        module_ids,
      ));
    }
    let stats = &plan.stats;
    let body = format!(
      "{{\n  \"marker\": \"rolldown-inline-common-chunks/1\",\n  \"disabledReason\": {},\n  \"maxSize\": {},\n  \"stats\": {{\"candidateChunks\": {}, \"selectedChunks\": {}, \"rejectedTooLarge\": {}, \"rejectedTopLevelAwait\": {}, \"rejectedDynamicallyImported\": {}, \"rejectedChunkRelativePath\": {}, \"rejectedEmitted\": {}, \"rejectedNoConsumer\": {}, \"rejectedNoCarrier\": {}, \"rejectedReexported\": {}, \"chunksInStaticCycles\": {}, \"factoryPlacements\": {}, \"placementsRemovedByElimination\": {}}},\n  \"inlinedChunks\": [\n{}\n  ]\n}}\n",
      disabled_reason,
      self.options.inline_common_chunks_max_size(),
      stats.candidate_chunks,
      stats.selected_chunks,
      stats.rejected_too_large,
      stats.rejected_top_level_await,
      stats.rejected_dynamically_imported,
      stats.rejected_chunk_relative_path,
      stats.rejected_emitted,
      stats.rejected_no_consumer,
      stats.rejected_no_carrier,
      stats.rejected_reexported,
      stats.chunks_in_static_cycles,
      stats.factory_placements,
      stats.placements_removed_by_elimination,
      chunks,
    );
    let _ = std::fs::write(path, body);
  }

  fn chunk_emits_chunk_relative_url(&self, chunk: &Chunk) -> bool {
    chunk.modules.iter().any(|idx| {
      self.link_output.module_table[*idx]
        .as_normal()
        .is_some_and(|module| !module.ecma_view.rolldown_file_url_references.is_empty())
    })
  }

  fn chunk_has_top_level_await(&self, chunk: &Chunk) -> bool {
    chunk
      .modules
      .iter()
      .any(|idx| self.link_output.metas[*idx].is_tla_or_contains_tla_dependency)
  }
}

/// Tarjan's algorithm over `importer -> importee` edges, iterative so a deep chunk graph cannot
/// overflow the stack. Components come out with dependencies before dependents, which is the order
/// ESM evaluates them in.
fn strongly_connected_components(
  live: &[ChunkIdx],
  imports: &FxHashMap<ChunkIdx, Vec<ChunkIdx>>,
) -> Vec<Vec<ChunkIdx>> {
  let live_set: FxHashSet<ChunkIdx> = live.iter().copied().collect();
  let mut index_of: FxHashMap<ChunkIdx, usize> = FxHashMap::default();
  let mut low_of: FxHashMap<ChunkIdx, usize> = FxHashMap::default();
  let mut on_stack: FxHashSet<ChunkIdx> = FxHashSet::default();
  let mut stack: Vec<ChunkIdx> = Vec::new();
  let mut components: Vec<Vec<ChunkIdx>> = Vec::new();
  let mut next_index = 0usize;

  for &root in live {
    if index_of.contains_key(&root) {
      continue;
    }
    // (node, position in its import list)
    let mut work: Vec<(ChunkIdx, usize)> = vec![(root, 0)];
    index_of.insert(root, next_index);
    low_of.insert(root, next_index);
    next_index += 1;
    stack.push(root);
    on_stack.insert(root);

    while let Some((node, cursor)) = work.pop() {
      let targets = &imports[&node];
      if cursor < targets.len() {
        work.push((node, cursor + 1));
        let target = targets[cursor];
        if !live_set.contains(&target) {
          continue;
        }
        if !index_of.contains_key(&target) {
          index_of.insert(target, next_index);
          low_of.insert(target, next_index);
          next_index += 1;
          stack.push(target);
          on_stack.insert(target);
          work.push((target, 0));
        } else if on_stack.contains(&target) {
          let candidate = index_of[&target];
          let current = low_of[&node];
          low_of.insert(node, current.min(candidate));
        }
        continue;
      }
      if low_of[&node] == index_of[&node] {
        let mut component = Vec::new();
        loop {
          let member = stack.pop().expect("tarjan stack cannot be empty");
          on_stack.remove(&member);
          component.push(member);
          if member == node {
            break;
          }
        }
        components.push(component);
      }
      if let Some(&(parent, _)) = work.last() {
        let candidate = low_of[&node];
        let current = low_of[&parent];
        low_of.insert(parent, current.min(candidate));
      }
    }
  }

  components
}
