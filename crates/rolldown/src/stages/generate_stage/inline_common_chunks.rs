//! `experimentalInlineCommonChunks`: replace small automatic common chunks with factory
//! definitions placed in their consumers, linked at runtime through one shared registry.
//!
//! See `internal-docs/inline-common-chunks/design.md` for the model and its limits.
//!
//! The pass runs after `compute_cross_chunk_links`, because it needs the final static chunk import
//! graph and the symbol-to-chunk table, and before deconfliction, because a host chunk must reserve
//! the names an inlined chunk's body already uses.

use std::fmt::Write as _;

use arcstr::ArcStr;
use petgraph::prelude::DiGraphMap;
use rolldown_common::{Chunk, ChunkIdx, ChunkKind, ChunkReasonType, EcmaModuleAstUsage};
use rolldown_utils::xxhash::xxhash_base64_url;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

/// The registry binding names emitted code refers to.
pub const SHARE_DEFINE_NAME: &str = "__rd_share";
pub const SHARE_REQUIRE_NAME: &str = "__rd_share_require";
pub const SHARE_FACTORY_PARAM_NAMES: [&str; 4] = ["__rd_m", "__rd_e", "__rd_def", "__rd_req"];
pub const SHARE_REGISTRY_INTERNAL_NAMES: [&str; 3] =
  ["__rd_factories", "__rd_records", "__rd_share_define"];
const ALL_GENERATED_BINDING_NAMES: [&str; 9] = [
  SHARE_DEFINE_NAME,
  SHARE_REQUIRE_NAME,
  "__rd_m",
  "__rd_e",
  "__rd_def",
  "__rd_req",
  "__rd_factories",
  "__rd_records",
  "__rd_share_define",
];

#[derive(Debug, Default)]
pub struct InlinedCommonChunks {
  /// Chunks replaced by factory placements, in evaluation order.
  pub chunks: Vec<ChunkIdx>,
  /// The live chunk that carries the runtime module, which also carries the registry.
  pub registry_chunk: Option<ChunkIdx>,
}

impl InlinedCommonChunks {
  pub fn has_chunks(&self) -> bool {
    !self.chunks.is_empty()
  }
}

impl GenerateStage<'_> {
  /// Selects the common chunks the feature may replace, decides where their factories go, and
  /// rewires the chunk graph so no chunk imports an inlined chunk as a file any more.
  #[expect(clippy::too_many_lines)]
  pub(super) fn select_inline_common_chunks(
    &self,
    chunk_graph: &mut ChunkGraph,
  ) -> InlinedCommonChunks {
    let max_size = self.options.inline_common_chunks_max_size();
    if max_size <= 0.0 {
      return InlinedCommonChunks::default();
    }
    if !matches!(self.options.format, rolldown_common::OutputFormat::Esm) {
      // Every other format resolves cross-chunk references through its own binding shape; the
      // factory interface is only defined for ESM output here.
      return InlinedCommonChunks::default();
    }
    if !self.options.is_strict_execution_order_enabled() {
      // A carrier registers and executes a factory in its own body, which ESM runs after every
      // chunk it statically imports. Without order wrapping an inlined chunk's body therefore moves
      // after chunks that the non-inlined static graph evaluates later, which is an observable
      // reordering. Under strict execution order every body is already deferred behind an `init_*`
      // wrapper, so the factory only defines and the order plan still decides when anything runs.
      return InlinedCommonChunks::default();
    }

    let runtime_module = self.link_output.runtime.id();
    let Some(registry_chunk) = chunk_graph.module_to_chunk[runtime_module] else {
      return InlinedCommonChunks::default();
    };
    if !chunk_graph.chunk_is_live(registry_chunk) {
      return InlinedCommonChunks::default();
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
      if emitted.contains(&idx) {
        continue;
      }
      if dynamically_imported.contains(&idx) {
        continue;
      }
      if self.chunk_has_external_dependencies(chunk) {
        // Moving an external import into a carrier can change both its relative resolution base and
        // its position among the carrier's own side-effect imports. Keep that chunk as a file until
        // imports can be merged with their original path and order.
        continue;
      }
      if self.chunk_has_direct_eval(chunk) {
        // Direct eval can observe any lexical name introduced around the copied body. Its string
        // contents are opaque to deconfliction, so the only sound first-version policy is to keep
        // the original file boundary.
        continue;
      }
      if self.chunk_has_unresolved_generated_name(chunk) {
        // Factory parameters are fixed protocol names. An originally unresolved reference with the
        // same spelling would be captured after wrapping and cannot be renamed by the symbol DB.
        continue;
      }
      if self.chunk_has_carrier_sensitive_syntax(chunk) {
        // A factory body is rendered once and printed into hosts that can sit in different output
        // directories. Dynamic imports can contain chunk-relative paths, and `import.meta` denotes
        // the physical host module rather than the logical source module.
        continue;
      }
      if reexported_owners.contains(&idx) {
        continue;
      }
      if self.chunk_has_top_level_await(chunk) {
        // The RFC's first policy does not select a chunk containing top-level await: the factory
        // interface is synchronous.
        continue;
      }
      if self.chunk_pre_render_size(chunk) >= max_size {
        continue;
      }
      selected.insert(idx);
    }

    if selected.is_empty() {
      return InlinedCommonChunks::default();
    }

    // A direct eval in a physical consumer could observe the registry aliases or factory
    // declarations injected into that chunk. Keep each directly imported candidate as a file.
    let eval_consumed: FxHashSet<ChunkIdx> = live
      .iter()
      .filter(|idx| self.chunk_has_direct_eval(&chunk_graph.chunk_table[**idx]))
      .flat_map(|idx| chunk_graph.chunk_table[*idx].cross_chunk_imports.iter().copied())
      .collect();
    // A carried factory's other static imports would have to be interleaved with each host's own
    // imports to preserve sibling dependency order. Support only the registry/runtime chunk and
    // other candidates here; selected-to-selected edges become registry requires below. Close both
    // exclusions to a fixed point because removing one candidate can disqualify its importers.
    loop {
      let previous = selected.clone();
      selected.retain(|idx| {
        !eval_consumed.contains(idx)
          && chunk_graph.chunk_table[*idx]
            .cross_chunk_imports
            .iter()
            .all(|target| *target == registry_chunk || previous.contains(target))
      });
      if selected == previous {
        break;
      }
    }
    if selected.is_empty() {
      return InlinedCommonChunks::default();
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
    selected.retain(|idx| has_importer.contains(idx));
    if selected.is_empty() {
      return InlinedCommonChunks::default();
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
    let mut graph = DiGraphMap::<ChunkIdx, ()>::new();
    for &idx in &live {
      graph.add_node(idx);
      for &target in &imports[&idx] {
        graph.add_edge(idx, target, ());
      }
    }
    let components = petgraph::algo::tarjan_scc(&graph);
    let mut component_of: FxHashMap<ChunkIdx, usize> = FxHashMap::default();
    for (index, component) in components.iter().enumerate() {
      for idx in component {
        component_of.insert(*idx, index);
      }
    }
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
        let mut needed: Vec<ChunkIdx> =
          reach[idx].iter().copied().filter(|target| !inherited.contains(target)).collect();
        needed.sort_unstable_by_key(|target| chunk_graph.chunk_table[*target].exec_order);
        if !needed.is_empty() {
          carried.insert(*idx, needed);
        }
        available.insert(*idx, inherited);
      }
    }

    // Every selected common chunk is reachable from a non-selected entry, so placement must assign
    // it to at least one carrier. Retain the release-mode guard because omitting an uncarried chunk
    // would discard reachable code.
    let mut with_carrier: FxHashSet<ChunkIdx> = FxHashSet::default();
    for targets in carried.values() {
      for target in targets {
        with_carrier.insert(*target);
      }
    }
    debug_assert!(selected.is_subset(&with_carrier), "every selected chunk should have a carrier");
    selected.retain(|idx| with_carrier.contains(idx));
    if selected.is_empty() {
      return InlinedCommonChunks::default();
    }

    let mut inlined: Vec<ChunkIdx> = selected.iter().copied().collect();
    inlined.sort_unstable_by_key(|idx| chunk_graph.chunk_table[*idx].exec_order);
    // The registry key follows logical chunk identity, not content or graph position. A source-only
    // edit therefore leaves every `__rd_share_require` site unchanged, while different chunks with
    // byte-identical output still have different identities. Sort identities before resolving the
    // vanishingly unlikely hash collision so even that fallback is deterministic.
    let mut identities: Vec<(ChunkIdx, String)> = inlined
      .iter()
      .map(|idx| (*idx, self.inline_chunk_identity(&chunk_graph.chunk_table[*idx])))
      .collect();
    identities.sort_unstable_by(|left, right| left.1.cmp(&right.1));
    let mut key_counts: FxHashMap<String, u32> = FxHashMap::default();
    for (idx, identity) in identities {
      let base = xxhash_base64_url(identity.as_bytes());
      let count = key_counts.entry(base.clone()).or_default();
      let key = if *count == 0 { base } else { format!("{base}-{count}") };
      *count += 1;
      chunk_graph.chunk_table[idx].inline_share_key = Some(ArcStr::from(key));
    }

    for (host, mut targets) in carried {
      targets.sort_unstable_by_key(|idx| chunk_graph.chunk_table[*idx].exec_order);
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

    InlinedCommonChunks { chunks: inlined, registry_chunk: Some(registry_chunk) }
  }

  /// After selection, no chunk may import an inlined chunk as a file. A host takes over the
  /// imports the inlined chunk needed for its own body, and every chunk that touches the registry
  /// imports the chunk that defines it.
  fn rewire_chunk_imports(
    &self,
    chunk_graph: &mut ChunkGraph,
    live: &[ChunkIdx],
    selected: &FxHashSet<ChunkIdx>,
    registry_chunk: ChunkIdx,
  ) {
    // Collected first because a host reads the inlined chunks' import lists while its own is
    // rewritten.
    let inlined_imports: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = selected
      .iter()
      .map(|idx| {
        let chunk = &chunk_graph.chunk_table[*idx];
        (*idx, chunk.cross_chunk_imports.clone())
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
      let mut seen: FxHashSet<ChunkIdx> = FxHashSet::default();
      let push_static =
        |target: ChunkIdx, seen: &mut FxHashSet<ChunkIdx>, out: &mut Vec<ChunkIdx>| {
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
        for target in &inlined_imports[carried_idx] {
          if selected.contains(target) {
            continue;
          }
          push_static(*target, &mut seen, &mut static_imports);
        }
      }
      if touches_registry {
        push_static(registry_chunk, &mut seen, &mut static_imports);
      }

      static_imports.sort_unstable_by_key(|target| chunk_graph.chunk_table[*target].exec_order);
      let chunk = &mut chunk_graph.chunk_table[idx];
      chunk.cross_chunk_imports = static_imports;
      // The host no longer imports the inlined chunk's symbols as bindings; the module finalizer
      // rewrites those references to `<binding>.<export>` instead.
      chunk.imports_from_other_chunks.retain(|target, _| !selected.contains(target));
    }
  }

  fn chunk_pre_render_size(&self, chunk: &Chunk) -> f64 {
    // The same unit `codeSplitting.maxSize` uses for manual groups: the module's source size, the
    // only size that exists while chunks are still being placed.
    #[expect(clippy::cast_precision_loss)]
    chunk.modules.iter().map(|idx| self.link_output.module_table[*idx].size() as f64).sum::<f64>()
  }

  fn inline_chunk_identity(&self, chunk: &Chunk) -> String {
    let mut module_ids: Vec<&str> = chunk
      .modules
      .iter()
      .map(|idx| self.link_output.module_table[*idx].stable_id().as_str())
      .collect();
    module_ids.sort_unstable();
    let mut identity = String::new();
    for id in module_ids {
      // Length-prefixing keeps virtual ids containing NUL or separators unambiguous.
      let _ = write!(identity, "{}:{id}", id.len());
    }
    identity
  }

  fn chunk_has_carrier_sensitive_syntax(&self, chunk: &Chunk) -> bool {
    chunk.modules.iter().any(|idx| {
      self.link_output.module_table[*idx].as_normal().is_some_and(|module| {
        module
          .ecma_view
          .ast_usage
          .intersects(EcmaModuleAstUsage::ImportMeta | EcmaModuleAstUsage::DynamicImport)
      })
    })
  }

  fn chunk_has_external_dependencies(&self, chunk: &Chunk) -> bool {
    !chunk.direct_imports_from_external_modules.is_empty()
      || !chunk.dynamic_imports_from_external_modules.is_empty()
      || !chunk.import_symbol_from_external_modules.is_empty()
      || !chunk.entry_level_external_module_idx.is_empty()
  }

  fn chunk_has_direct_eval(&self, chunk: &Chunk) -> bool {
    chunk.modules.iter().any(|idx| {
      self.link_output.module_table[*idx]
        .as_normal()
        .is_some_and(|module| module.ecma_view.meta.has_eval())
    })
  }

  fn chunk_has_unresolved_generated_name(&self, chunk: &Chunk) -> bool {
    chunk.modules.iter().copied().any(|idx| {
      self.link_output.symbol_db[idx].as_ref().is_some_and(|scopes| {
        scopes.ast_scopes.scoping().root_unresolved_references().keys().any(|name| {
          ALL_GENERATED_BINDING_NAMES.iter().any(|generated| name.as_str() == *generated)
        })
      })
    })
  }

  fn chunk_has_top_level_await(&self, chunk: &Chunk) -> bool {
    chunk.modules.iter().any(|idx| self.link_output.metas[*idx].is_tla_or_contains_tla_dependency)
  }
}
