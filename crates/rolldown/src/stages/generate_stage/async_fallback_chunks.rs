use rolldown_common::{
  ChunkIdx, ChunkKind, ChunkModulesOrderBy, EcmaModuleAstUsage, ImportKind, ModuleIdx, StmtInfos,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{GenerateStage, lazy_host_inits::HostPlacement};
use crate::chunk_graph::ChunkGraph;

struct AsyncConsumerLayers {
  /// Position of the module whose body suspends, in execution order.
  async_position: usize,
  /// Number of modules in each layer after the async module, in execution order.
  widths: Vec<usize>,
}

impl GenerateStage<'_> {
  /// An eager fallback relies on native ESM to start independent requests before awaiting
  /// any of them. See internal-docs/import-defer/implementation.md.
  pub(super) fn split_async_fallback_chunks(&self, graph: &mut ChunkGraph) {
    let link = &self.link_output;
    if link.async_fallback_importers.is_empty() {
      return;
    }
    let mut stack: Vec<_> = link
      .async_fallback_importers
      .iter()
      .copied()
      .filter(|&idx| {
        link.metas[idx].is_included && link.metas[idx].is_tla_or_contains_tla_dependency
      })
      .collect();
    let mut seen = FxHashSet::default();
    let mut private_children = FxHashSet::default();
    while let Some(idx) = stack.pop() {
      if !seen.insert(idx) {
        continue;
      }
      let mut deps = link.module_table[idx]
        .import_records()
        .iter()
        .filter(|rec| rec.kind == ImportKind::Import)
        .filter_map(|rec| rec.resolved_module);
      // A synchronous request can release a wait through any number of imports or re-exports.
      stack.extend(deps.clone());
      // A module and its sole, privately imported dependency can share an evaluation
      // boundary. Use the parent as the representative: it inherits any top-level await
      // in the chain. Decreasing execution order breaks cycles.
      if let Some(child_idx) = deps.next()
        && deps.all(|other| other == child_idx)
        && graph.module_to_chunk[idx].is_some()
        && graph.module_to_chunk[idx] == graph.module_to_chunk[child_idx]
        && let Some(child) = link.module_table[child_idx].as_normal()
        && child.importers_idx.len() == 1
        && !link.entries.contains_key(&child_idx)
        && child.exec_order < link.module_table[idx].exec_order()
      {
        private_children.insert(child_idx);
      }
    }
    let mut hosts = FxHashMap::default();
    let mut chunks = FxHashSet::default();
    for &idx in seen.difference(&private_children) {
      let is_async = link.metas[idx].is_tla_or_contains_tla_dependency;
      if let Some(host) = graph.module_to_chunk[idx]
        && let Some(previous_async) = hosts.insert(host, is_async)
        && (is_async || previous_async)
      {
        chunks.insert(host);
      }
    }
    let mut chunks: Vec<_> = chunks.into_iter().collect();
    chunks.sort_unstable();
    for chunk_idx in chunks {
      let chunk = &graph.chunk_table[chunk_idx];
      let mut modules = chunk.modules.clone();
      modules.sort_unstable_by_key(|&idx| link.module_table[idx].exec_order());
      if let Some(layers) = self.local_async_consumer_layers(graph, chunk_idx, &modules) {
        // Keep the await in a native dependency. Its consumers and external importers of the
        // entry then resume in the same job, before microtasks queued by a consumer.
        let (prerequisites, rest) = modules.split_at(layers.async_position);
        if !prerequisites.is_empty() {
          // A throw here must stop the import walk synchronously, before the async body starts.
          self.move_host_modules(graph, chunk_idx, prerequisites, HostPlacement::Together);
        }
        self.move_host_modules(graph, chunk_idx, &rest[..1], HostPlacement::Together);
        if layers.widths.iter().any(|&width| width > 1) {
          graph.async_consumer_layers.insert(chunk_idx, layers.widths);
        }
        continue;
      }
      // Keep the public entry at its configured filename. A common chunk keeps one
      // member and its existing name, including a user-supplied manual group name.
      let anchor = match chunk.kind {
        ChunkKind::EntryPoint { module, .. } => module,
        ChunkKind::Common => modules[0],
      };
      modules.retain(|&idx| idx != anchor);
      self.move_host_modules(graph, chunk_idx, &modules, HostPlacement::Separate);
    }
  }

  /// Can this entry schedule the consumers of its one local async dependency? After it,
  /// each layer must wait for *every* member of the preceding layer. Thus none of the consumer
  /// bodies can release a pending await, and a failure skips all subsequent layers.
  fn local_async_consumer_layers(
    &self,
    graph: &ChunkGraph,
    host: ChunkIdx,
    modules: &[ModuleIdx],
  ) -> Option<AsyncConsumerLayers> {
    let link = &self.link_output;
    let entry = graph.chunk_table[host].entry_module_idx()?;
    if modules.last() != Some(&entry)
      || self.options.is_dev_mode_enabled()
      || matches!(
        self.options.experimental.chunk_modules_order,
        Some(ChunkModulesOrderBy::ModuleId)
      )
    {
      return None;
    }

    let mut layer_of = FxHashMap::default();
    let mut widths = vec![];
    let mut async_module = None;
    let mut dependencies = FxHashSet::default();
    for (position, &idx) in modules.iter().enumerate() {
      let module = link.module_table[idx].as_normal()?;
      let meta = &link.metas[idx];
      // Entry facades and other chunks can evaluate a proper subset of these consumers.
      if !meta.wrap_kind().is_none()
        || module.meta.has_eval()
        || (idx != entry && link.entries.contains_key(&idx))
        || module.importers_idx.iter().any(|&i| graph.module_to_chunk[i] != Some(host))
        || !module.dynamic_importers_idx.is_empty()
      {
        return None;
      }

      if module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait)
        && async_module.replace((idx, position)).is_some()
      {
        return None;
      }

      dependencies.clear();
      let mut latest = None;
      for rec in &module.import_records {
        if rec.kind != ImportKind::Import {
          return None;
        }
        let target = rec.resolved_module?;
        if target == link.runtime.id() {
          continue;
        }
        // Dependencies outside this entry and back edges need the native loader.
        let &layer = layer_of.get(&target)?;
        latest = Some(latest.map_or(layer, |prev: usize| prev.max(layer)));
        dependencies.insert(target);
      }
      let layer = if let Some(latest) = latest {
        if dependencies.iter().filter(|&dep| layer_of[dep] == latest).count() != widths[latest] {
          return None;
        }
        latest + 1
      } else {
        0
      };
      // An edge from an earlier consumer cannot wait for a peer encountered after it.
      if layer + 1 < widths.len() {
        return None;
      }
      if layer == widths.len() {
        widths.push(0);
      }
      widths[layer] += 1;
      layer_of.insert(idx, layer);
    }
    let (root, position) = async_module?;
    let level = layer_of[&root];
    if widths[level] != 1 || position + 1 == modules.len() {
      return None;
    }
    // Siblings run in separate lexical scopes so an exception in one doesn't skip the next.
    // Bindings read from outside that scope still need a native module boundary.
    for &idx in &modules[position + 1..] {
      let meta = &link.metas[idx];
      if widths[layer_of[&idx]] > 1
        && (!meta.resolved_exports.is_empty()
          || meta.stmt_info_included.has_bit(StmtInfos::NAMESPACE_STMT_IDX))
      {
        return None;
      }
    }
    Some(AsyncConsumerLayers { async_position: position, widths: widths.split_off(level + 1) })
  }
}
