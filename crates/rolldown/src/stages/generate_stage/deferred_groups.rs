use itertools::Itertools;
use rolldown_common::{
  ConcatenateWrappedModuleKind, ImportKind, Module, ModuleGroup, ModuleIdx, OutputFormat, WrapKind,
};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph, esm_init_obligations::WrappedEsmInitTarget,
  types::linking_metadata::DeferredAccess,
};

use super::{FinalEsmInitMetadata, GenerateStage};

impl GenerateStage<'_> {
  /// Puts the private modules of each deferred module under the init wrapper of that module (a
  /// deferred group), so that the engine parses one wrapper for each group, not one for each
  /// module.
  ///
  /// - A root: a module of an `import defer` closure that keeps its `init_*` wrapper.
  /// - A member of the group of root `R`: a module of the closure that only the bodies of `R` and
  ///   of the other members of the group call. The member has no wrapper. The wrapper of `R` runs
  ///   the body of each member, in the order of evaluation (`render_esm`).
  ///
  /// A module keeps its wrapper, and is a root itself, when a place other than a body in the same
  /// chunk can call its init: a deferred access or a deferred namespace, a dynamic `import()`, a
  /// `require()`, eager code, another chunk or an entry. A module also keeps its wrapper when a
  /// chunk exports the wrapper, because the export needs the declaration.
  ///
  /// Call it after `commit_cross_chunk_links`, which writes the chunk exports, and after
  /// `ensure_lazy_module_initialization_order`, because a member must not have a moved init call.
  /// Call it before `finalize_modules`, which reads the groups.
  /// See internal-docs/import-defer/implementation.md ("Deferred groups").
  pub fn group_deferred_modules(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    final_esm_init_metadata: &FinalEsmInitMetadata,
  ) {
    let options = self.options;
    // Only `render_esm` renders the groups. The other conditions keep paths that put their own
    // calls or names on each wrapper: strict execution order (its own init obligations), dev mode
    // (the HMR header of each module), `profilerNames` (the name of each wrapper) and inline
    // common chunks (the modules of a record are printed for each carrier).
    if !options.experimental.is_import_defer_enabled()
      || !matches!(options.format, OutputFormat::Esm)
      || options.is_strict_execution_order_enabled()
      || options.is_dev_mode_enabled()
      || options.profiler_names
      || options.inline_common_chunks.is_some()
    {
      return;
    }
    let module_table = &self.link_output.module_table;
    let metas = &self.link_output.metas;
    if !metas.iter().any(|meta| meta.wrapped_by_import_defer) {
      return;
    }
    let module_to_chunk = &chunk_graph.module_to_chunk;
    let same_chunk = |a: ModuleIdx, b: ModuleIdx| {
      module_to_chunk[a].is_some() && module_to_chunk[a] == module_to_chunk[b]
    };

    let keeps_wrapper = self.modules_that_keep_wrapper(chunk_graph);

    // A module that can be a root or a member: a lazy ESM wrapper with a plain init.
    let runtime_idx = self.link_output.runtime.id();
    let groupable = module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .map(|module| module.idx)
      .filter(|&idx| {
        let meta = &metas[idx];
        idx != runtime_idx
          && module_to_chunk[idx].is_some()
          && meta.is_included
          && meta.wrapped_by_import_defer
          && matches!(meta.wrap_kind(), WrapKind::Esm)
          && meta.wrapper_ref.is_some()
          && !meta.is_tla_or_contains_tla_dependency
          && meta.import_defer_tracked_deps.is_none()
          && !meta.dynamic_defer_target
      })
      .collect::<FxIndexSet<_>>();
    // One root is enough for a group: a root can have only CommonJS members
    // (`commonjs_modules_of_groups`).
    if groupable.is_empty() {
      return;
    }

    // The init calls of each groupable module: from the import records of the included modules,
    // and from the calls that the finalizer restores for excluded statements
    // (`transitive_init_targets`). `plain`: the call is a statement in the body of a module of
    // the same chunk.
    let mut callers: FxHashMap<ModuleIdx, Vec<(ModuleIdx, bool)>> = FxHashMap::default();
    let mut callees: FxHashMap<ModuleIdx, Vec<ModuleIdx>> = FxHashMap::default();
    let mut add_call = |caller: ModuleIdx, target: ModuleIdx, plain: bool| {
      let plain = plain && same_chunk(caller, target);
      callers.entry(target).or_default().push((caller, plain));
      if plain && groupable.contains(&caller) {
        callees.entry(caller).or_default().push(target);
      }
    };
    for module in module_table.modules.iter().filter_map(Module::as_normal) {
      if !metas[module.idx].is_included {
        continue;
      }
      for rec in &module.import_records {
        if let Some(target) = rec.resolved_module
          && groupable.contains(&target)
        {
          add_call(module.idx, target, rec.kind == ImportKind::Import && !rec.is_defer_phase());
        }
      }
      for targets in final_esm_init_metadata
        .transitive_init_targets(module.idx)
        .into_iter()
        .flat_map(|m| m.values())
      {
        for target in targets {
          if let WrappedEsmInitTarget::Module(target) = *target
            && groupable.contains(&target)
          {
            add_call(module.idx, target, true);
          }
        }
      }
    }
    let candidates = groupable
      .iter()
      .copied()
      .filter(|idx| {
        !keeps_wrapper.contains(idx)
          && callers.get(idx).is_some_and(|list| list.iter().all(|&(_, plain)| plain))
      })
      .collect::<FxHashSet<_>>();

    // Greatest fixed point: a candidate that only one root reaches through candidates is a
    // member of that root, unless it has a caller outside the group. A candidate with no owner
    // (a shared module) becomes a root, and its own private modules can then form its group. A
    // root with a no-op init has no group, because an importer can mark a no-op init call pure,
    // and the minifier then removes the call. With no candidates, the first round ends with no
    // owners, and the groups get only CommonJS members.
    let mut roots: FxIndexSet<ModuleIdx> =
      groupable.iter().copied().filter(|idx| !candidates.contains(idx)).collect();
    let owner = loop {
      let mut reach: FxHashMap<ModuleIdx, (ModuleIdx, u32)> = FxHashMap::default();
      for &root in &roots {
        if final_esm_init_metadata.init_is_noop(root) {
          continue;
        }
        let mut seen = FxHashSet::default();
        let mut stack = vec![root];
        while let Some(idx) = stack.pop() {
          for &callee in callees.get(&idx).map_or(&[][..], Vec::as_slice) {
            if callee != root && !roots.contains(&callee) && seen.insert(callee) {
              stack.push(callee);
            }
          }
        }
        for idx in seen {
          reach.entry(idx).and_modify(|(_, count)| *count += 1).or_insert((root, 1));
        }
      }
      let mut owner: FxHashMap<ModuleIdx, ModuleIdx> = reach
        .into_iter()
        .filter_map(|(idx, (root, count))| (count == 1).then_some((idx, root)))
        .collect();
      loop {
        let outside = owner
          .iter()
          .filter(|&(idx, root)| {
            callers.get(idx).is_some_and(|list| {
              list.iter().any(|(caller, _)| caller != root && owner.get(caller) != Some(root))
            })
          })
          .map(|(idx, _)| *idx)
          .collect_vec();
        if outside.is_empty() {
          break;
        }
        for idx in outside {
          owner.remove(&idx);
        }
      }
      let unowned = candidates
        .iter()
        .copied()
        .filter(|idx| !roots.contains(idx) && !owner.contains_key(idx))
        .collect::<FxHashSet<_>>();
      if unowned.is_empty() {
        break owner;
      }
      // Only the shared modules that a module outside the shared part calls become roots in this
      // round. A shared module below them can be a private module of one of them in the next
      // round. If each shared module has only shared callers (a cycle), all of them become roots.
      let mut new_roots = unowned
        .iter()
        .copied()
        .filter(|idx| {
          callers
            .get(idx)
            .is_some_and(|list| list.iter().any(|(caller, _)| !unowned.contains(caller)))
        })
        .sorted_unstable()
        .collect_vec();
      if new_roots.is_empty() {
        new_roots = unowned.into_iter().sorted_unstable().collect_vec();
      }
      roots.extend(new_roots);
    };

    // Each groupable module that is not a member is a root.
    let root_of =
      |idx: ModuleIdx| owner.get(&idx).copied().or_else(|| groupable.contains(&idx).then_some(idx));
    let commonjs = self.commonjs_modules_of_groups(
      chunk_graph,
      final_esm_init_metadata,
      &keeps_wrapper,
      &root_of,
    );

    let mut members_by_root: FxHashMap<ModuleIdx, Vec<ModuleIdx>> = FxHashMap::default();
    for (&idx, &root) in owner.iter().chain(&commonjs) {
      members_by_root.entry(root).or_default().push(idx);
    }
    let mut groups = members_by_root.into_iter().collect_vec();
    groups.sort_unstable_by_key(|(root, _)| module_table[*root].exec_order());
    for (root, mut members) in groups {
      members.sort_unstable_by_key(|idx| module_table[*idx].exec_order());
      let chunk_idx = chunk_graph.module_to_chunk[root].expect("a root is in a chunk");
      let metas = &mut self.link_output.metas;
      metas[root].concatenated_wrapped_module_kind = ConcatenateWrappedModuleKind::Root;
      for &member in &members {
        // A CommonJS module keeps its `require_*` wrapper and its kind `None`. The renderer puts
        // the whole wrapper declaration in the wrapper of the root.
        if !commonjs.contains_key(&member) {
          metas[member].concatenated_wrapped_module_kind = ConcatenateWrappedModuleKind::Inner;
        }
      }
      members.push(root);
      chunk_graph.chunk_table[chunk_idx].module_groups.push(ModuleGroup::new(members, root));
    }
  }

  /// Finds the CommonJS modules that only the bodies of one group can call, and returns the root
  /// of that group for each of them. The renderer puts the wrapper declaration
  /// (`var require_x = __commonJSMin(...)`) of such a module in the wrapper of the root, so that
  /// the engine does not compile its factory at startup.
  ///
  /// Each caller must be in the same chunk, and must be one of these (least fixed point):
  /// - a plain `import` of an ESM module of the group. The finalizer puts the `require_x()` call
  ///   of a static import in the body of the importer.
  /// - a `require()` of a CommonJS module that moves into the same group. The whole code of a
  ///   CommonJS module is in its factory.
  ///
  /// A `require()` of an ESM module can be in a hoisted function at the top level, so it does not
  /// qualify. Any other caller (a deferred import, a dynamic `import()`, another chunk, eager code)
  /// keeps the declaration at the top level.
  fn commonjs_modules_of_groups(
    &self,
    chunk_graph: &ChunkGraph,
    final_esm_init_metadata: &FinalEsmInitMetadata,
    keeps_wrapper: &FxHashSet<ModuleIdx>,
    root_of: &dyn Fn(ModuleIdx) -> Option<ModuleIdx>,
  ) -> FxHashMap<ModuleIdx, ModuleIdx> {
    let module_table = &self.link_output.module_table;
    let metas = &self.link_output.metas;
    let module_to_chunk = &chunk_graph.module_to_chunk;
    let runtime_idx = self.link_output.runtime.id();
    let is_candidate = |idx: ModuleIdx| {
      let meta = &metas[idx];
      idx != runtime_idx
        && module_to_chunk[idx].is_some()
        && meta.is_included
        && meta.wrapped_by_import_defer
        && matches!(meta.wrap_kind(), WrapKind::Cjs)
        && meta.wrapper_ref.is_some()
        && !keeps_wrapper.contains(&idx)
    };
    // `Some((caller, caller_is_commonjs))` for a caller that can qualify, `None` for one that
    // cannot.
    let mut callers: FxHashMap<ModuleIdx, Vec<Option<(ModuleIdx, bool)>>> = FxHashMap::default();
    for module in module_table.modules.iter().filter_map(Module::as_normal) {
      if !metas[module.idx].is_included {
        continue;
      }
      let caller_is_commonjs = matches!(metas[module.idx].wrap_kind(), WrapKind::Cjs);
      for rec in &module.import_records {
        let Some(target) = rec.resolved_module else { continue };
        if !is_candidate(target) {
          continue;
        }
        let same_chunk = module_to_chunk[module.idx] == module_to_chunk[target];
        let caller = match rec.kind {
          ImportKind::Import if same_chunk && !rec.is_defer_phase() && !caller_is_commonjs => {
            Some((module.idx, false))
          }
          ImportKind::Require if same_chunk && caller_is_commonjs => Some((module.idx, true)),
          _ => None,
        };
        callers.entry(target).or_default().push(caller);
      }
      // A restored init call is a statement in the body of the module.
      for targets in final_esm_init_metadata
        .transitive_init_targets(module.idx)
        .into_iter()
        .flat_map(|map| map.values())
      {
        for target in targets {
          match *target {
            WrappedEsmInitTarget::Module(target) => {
              if is_candidate(target) {
                let caller = (module_to_chunk[module.idx] == module_to_chunk[target]
                  && !caller_is_commonjs)
                  .then_some((module.idx, false));
                callers.entry(target).or_default().push(caller);
              }
            }
            // A carrier route of strict execution order, which has no groups. Keep its target at
            // the top level.
            WrappedEsmInitTarget::CjsCarrier(key) => {
              if let Module::Normal(importer) = &module_table[key.importer]
                && let Some(target) = importer.import_records[key.record].resolved_module
                && is_candidate(target)
              {
                callers.entry(target).or_default().push(None);
              }
            }
          }
        }
      }
    }
    let mut placed: FxHashMap<ModuleIdx, ModuleIdx> = FxHashMap::default();
    loop {
      let mut changed = false;
      for (&commonjs, list) in &callers {
        if placed.contains_key(&commonjs) {
          continue;
        }
        let mut root = None;
        let qualifies = list.iter().all(|caller| {
          let Some((caller, caller_is_commonjs)) = *caller else { return false };
          let caller_root =
            if caller_is_commonjs { placed.get(&caller).copied() } else { root_of(caller) };
          match (caller_root, root) {
            (Some(caller_root), None) => {
              root = Some(caller_root);
              true
            }
            (Some(caller_root), Some(root)) => caller_root == root,
            (None, _) => false,
          }
        });
        if qualifies && let Some(root) = root {
          placed.insert(commonjs, root);
          changed = true;
        }
      }
      if !changed {
        return placed;
      }
    }
  }

  /// The modules that must keep their own wrapper (`init_*` or `require_*`), because a place
  /// other than a body of a module in the same chunk can call it, or because a chunk exports it.
  fn modules_that_keep_wrapper(&self, chunk_graph: &ChunkGraph) -> FxHashSet<ModuleIdx> {
    let module_table = &self.link_output.module_table;
    let metas = &self.link_output.metas;
    let symbols = &self.link_output.symbol_db;
    let mut keeps_wrapper: FxHashSet<ModuleIdx> =
      self.link_output.entries.keys().copied().collect();
    // A chunk can export a wrapper that the importing chunk does not call: the cross-chunk links
    // add the wrapper of the owner of each imported symbol
    // (`add_depended_symbol_with_wrapped_esm_init`). Only one branch of
    // `deconflict_exported_names` stores canonical keys, so compare canonical refs.
    let exported_symbols = chunk_graph
      .chunk_table
      .iter()
      .flat_map(|chunk| chunk.exports_to_other_chunks.keys())
      .map(|&symbol_ref| symbols.canonical_ref_for(symbol_ref))
      .collect::<FxHashSet<_>>();
    for (idx, meta) in metas.iter_enumerated() {
      // `x_deferred` calls `init_x`.
      if meta.deferred_namespace_ref.is_some() {
        keeps_wrapper.insert(idx);
      }
      if meta.wrapper_ref.is_some_and(|wrapper_ref| {
        exported_symbols.contains(&symbols.canonical_ref_for(wrapper_ref))
      }) {
        keeps_wrapper.insert(idx);
      }
      for access in meta.deferred_access_targets.values() {
        if let DeferredAccess::Esm(target) = access {
          keeps_wrapper.insert(*target);
        }
      }
    }
    // A module that eager code reaches through static imports can get an init call from a
    // statement that skips the modules between them: the init prelude of an unwrapped entry, or
    // the init calls behind an unwrapped re-export module.
    let mut eager_stack = module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .filter(|module| !metas[module.idx].wrapped_by_import_defer)
      .map(|module| module.idx)
      .collect_vec();
    let mut eager_reached: FxHashSet<ModuleIdx> = eager_stack.iter().copied().collect();
    while let Some(module_idx) = eager_stack.pop() {
      let Module::Normal(module) = &module_table[module_idx] else { continue };
      for rec in &module.import_records {
        if rec.kind.is_static()
          && !rec.is_defer_phase()
          && let Some(target) = rec.resolved_module
          && eager_reached.insert(target)
        {
          eager_stack.push(target);
        }
      }
    }
    keeps_wrapper.extend(eager_reached);
    // `ensure_lazy_module_initialization_order` moves an init call to another module.
    for (chunk_idx, chunk) in chunk_graph.chunk_table.iter_enumerated() {
      if self.inline_state.is_record(chunk_idx) {
        keeps_wrapper.extend(chunk.modules.iter().copied());
        continue;
      }
      keeps_wrapper.extend(chunk.remove_map.keys().copied());
      for moved in chunk.insert_map.values() {
        for &(importer_idx, rec_idx) in moved {
          if let Module::Normal(importer) = &module_table[importer_idx]
            && let Some(target) = importer.import_records[rec_idx].resolved_module
          {
            keeps_wrapper.insert(target);
          }
        }
      }
    }
    keeps_wrapper
  }
}
