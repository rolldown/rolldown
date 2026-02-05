use std::{cmp::Ordering, collections::VecDeque, path::Path};

use crate::{
  chunk_graph::ChunkGraph,
  stages::generate_stage::{chunk_ext::ChunkDebugExt, chunk_optimizer::ChunkOptimizationGraph},
  types::linking_metadata::LinkingMetadataVec,
  utils::chunk::normalize_preserve_entry_signature,
};
use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, EntryPointKind, ExportsKind, ImportKind, ImportRecordIdx,
  ImportRecordMeta, IndexModules, Module, ModuleIdx, ModuleNamespaceIncludedReason,
  PreserveEntrySignatures, SymbolRef, WrapKind,
};
use rolldown_error::BuildResult;
use rolldown_utils::{
  BitSet, commondir,
  index_vec_ext::IndexVecRefExt,
  indexmap::FxIndexMap,
  rayon::ParallelIterator,
  rustc_hash::{FxHashMapExt, FxHashSetExt},
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{GenerateStage, chunk_ext::ChunkCreationReason};

#[derive(Clone, Debug)]
pub struct SplittingInfo {
  pub bits: BitSet,
  pub share_count: u32,
}

pub type IndexSplittingInfo = IndexVec<ModuleIdx, SplittingInfo>;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate_chunks(&mut self) -> BuildResult<ChunkGraph> {
    // Count total entry points (not unique modules) to handle duplicates correctly
    let entries_len: u32 = self
      .link_output
      .entries
      .values()
      .map(Vec::len)
      .sum::<usize>()
      .try_into()
      .expect("Too many entries, u32 overflowed.");
    // If we are in test environment, to make the runtime module always fall into a standalone chunk,
    // we create a facade entry point for it.

    let mut chunk_graph = ChunkGraph::new(self.link_output.module_table.modules.len());
    chunk_graph.chunk_table.chunks.reserve(entries_len as usize);

    let mut index_splitting_info: IndexSplittingInfo = oxc_index::index_vec![SplittingInfo {
        bits: BitSet::new(entries_len),
        share_count: 0
      }; self.link_output.module_table.modules.len()];
    let mut bits_to_chunk = FxHashMap::with_capacity(entries_len as usize);

    let input_base = ArcStr::from(
      self
        .get_common_dir_of_all_modules(self.link_output.module_table.modules.as_vec())
        .unwrap_or_default(),
    );
    if self.options.preserve_modules {
      let modules_len = self
        .link_output
        .module_table
        .modules
        .len()
        .try_into()
        .expect("Entry length exceeds u32::MAX");
      for (idx, module) in self.link_output.module_table.modules.iter_enumerated() {
        let Module::Normal(module) = module else {
          continue;
        };
        let matched_entry =
          self.link_output.entries.get(&module.idx).and_then(|entries| entries.first());
        if !self.link_output.metas[module.idx].is_included {
          continue;
        }

        let count = idx.raw();
        let mut bits = BitSet::new(modules_len);
        bits.set_bit(count);
        let mut chunk = Chunk::new(
          matched_entry.and_then(|item| item.name.clone()),
          matched_entry.and_then(|item| item.file_name.clone()),
          bits.clone(),
          vec![],
          ChunkKind::EntryPoint {
            meta: {
              let mut meta = ChunkMeta::default();
              meta.set(
                ChunkMeta::UserDefinedEntry,
                matched_entry
                  .map(|entry_point| matches!(entry_point.kind, EntryPointKind::UserDefined))
                  .unwrap_or(false),
              );
              meta.set(ChunkMeta::DynamicImported, !module.dynamic_importers.is_empty());
              meta.set(
                ChunkMeta::EmittedChunk,
                matched_entry
                  .map(|entry_point| entry_point.kind.is_emitted_user_defined())
                  .unwrap_or(false),
              );
              meta
            },
            bit: count,
            module: module.idx,
          },
          input_base.clone(),
          // The preserve_entry_signatures has no effect when `preserve_modules` is enabled.
          None,
        );
        chunk.add_creation_reason(
          ChunkCreationReason::PreserveModules {
            is_user_defined_entry: module.is_user_defined_entry,
            module_stable_id: &module.stable_id,
          },
          self.options,
        );
        let chunk_idx = chunk_graph.add_chunk(chunk);
        if let Some(entries) = self.link_output.entries.get(&module.idx) {
          for entry in entries {
            if let Some(reference_ids) = self.link_output.entry_point_to_reference_ids.get(entry) {
              chunk_graph
                .chunk_idx_to_reference_ids
                .entry(chunk_idx)
                .or_default()
                .extend(reference_ids.iter().cloned());
            }
          }
        }
        chunk_graph.add_module_to_chunk(
          module.idx,
          chunk_idx,
          self.link_output.metas[module.idx].depended_runtime_helper,
        );
        // bits_to_chunk.insert(bits, chunk); // This line is intentionally commented out because `bits_to_chunk` is not used in this loop. It is updated elsewhere in the `init_entry_point` and `split_chunks` methods.
        chunk_graph.entry_module_to_entry_chunk.entry(module.idx).or_insert(chunk_idx);
      }
    } else {
      self.init_entry_point(&mut chunk_graph, &mut bits_to_chunk, entries_len, &input_base);

      self
        .split_chunks(&mut index_splitting_info, &mut chunk_graph, &mut bits_to_chunk, &input_base)
        .await?;
    }
    // Merge external import namespaces at chunk level.
    for symbol_set in self.link_output.external_import_namespace_merger.values() {
      for (_, mut group) in symbol_set
        .iter()
        .filter_map(|item| {
          let module = self.link_output.module_table[item.owner].as_normal()?;
          self.link_output.metas[module.idx].is_included.then_some(item)
        })
        .into_group_map_by(|item| {
          chunk_graph.module_to_chunk[item.owner].expect("should have chunk idx")
        })
      {
        if group.len() <= 1 {
          continue;
        }
        group.sort_unstable_by_key(|item| self.link_output.module_table[item.owner].exec_order());
        let Some(idx) =
          group.iter().position(|item| self.link_output.used_symbol_refs.contains(item))
        else {
          continue;
        };
        // In the extreme case, idx would eq to group.len() - 1, which means the first symbol is the only one that is used.
        // `idx + 1` would eq to len of the group, the iteration is still safe,
        // https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=38bb53e79b4f7aaa73ef9d6b4cfb3cc2
        for symbol in &group[idx + 1..] {
          self.link_output.symbol_db.link(**symbol, *group[idx]);
        }
      }
    }

    chunk_graph.sort_chunk_modules(self.link_output, self.options);

    chunk_graph
      .chunk_table
      .iter_mut()
      .sorted_by(|a, b| {
        let a_should_be_first = Ordering::Less;
        let b_should_be_first = Ordering::Greater;

        match (&a.kind, &b.kind) {
          (
            ChunkKind::EntryPoint { module: a_module_id, .. },
            ChunkKind::EntryPoint { module: b_module_id, .. },
          ) => self.link_output.module_table[*a_module_id]
            .exec_order()
            .cmp(&self.link_output.module_table[*b_module_id].exec_order()),
          (ChunkKind::EntryPoint { module: a_module_id, .. }, ChunkKind::Common) => {
            let a_module_exec_order = self.link_output.module_table[*a_module_id].exec_order();
            let b_chunk_first_module_exec_order =
              self.link_output.module_table[b.modules[0]].exec_order();
            if a_module_exec_order == b_chunk_first_module_exec_order {
              a_should_be_first
            } else {
              a_module_exec_order.cmp(&b_chunk_first_module_exec_order)
            }
          }
          (ChunkKind::Common, ChunkKind::EntryPoint { module: b_module_id, .. }) => {
            let b_module_exec_order = self.link_output.module_table[*b_module_id].exec_order();
            let a_chunk_first_module_exec_order =
              self.link_output.module_table[a.modules[0]].exec_order();
            if a_chunk_first_module_exec_order == b_module_exec_order {
              b_should_be_first
            } else {
              a_chunk_first_module_exec_order.cmp(&b_module_exec_order)
            }
          }
          (ChunkKind::Common, ChunkKind::Common) => {
            let a_chunk_first_module_exec_order =
              self.link_output.module_table[a.modules[0]].exec_order();
            let b_chunk_first_module_exec_order =
              self.link_output.module_table[b.modules[0]].exec_order();
            a_chunk_first_module_exec_order.cmp(&b_chunk_first_module_exec_order)
          }
        }
      })
      .enumerate()
      .for_each(|(i, chunk)| {
        chunk.exec_order = i.try_into().expect("Too many chunks, u32 overflowed.");
      });
    // The esbuild using `Chunk#bits` to sorted chunks, but the order of `Chunk#bits` is not stable, eg `BitSet(0) 00000001_00000000` > `BitSet(8) 00000000_00000001`. It couldn't ensure the order of dynamic chunks and common chunks.
    // Consider the compare `Chunk#exec_order` should be faster than `Chunk#bits`, we use `Chunk#exec_order` to sort chunks.
    // Note Here could be make sure the order of chunks.
    // - entry chunks are always before other chunks
    // - static chunks are always before dynamic chunks
    // - other chunks has stable order at per entry chunk level
    // i.e.
    // EntryPoint (is_user_defined: true) < EntryPoint (is_user_defined: false) or Common
    // [order by chunk index]               [order by exec order]

    let sorted_chunk_idx_vec = chunk_graph
      .chunk_table
      .iter_enumerated()
      .sorted_by_key(|(index, chunk)| match &chunk.kind {
        ChunkKind::EntryPoint { meta, .. } if meta.contains(ChunkMeta::UserDefinedEntry) => {
          (0, index.raw())
        }
        _ => (1, chunk.exec_order),
      })
      .map(|(idx, _)| idx)
      .collect::<Vec<_>>();

    chunk_graph.sorted_chunk_idx_vec = sorted_chunk_idx_vec;

    self.find_entry_level_external_module(&mut chunk_graph);

    Ok(chunk_graph)
  }

  pub fn ensure_lazy_module_initialization_order(&self, chunk_graph: &mut ChunkGraph) {
    if self.options.is_strict_execution_order_enabled() {
      // If `strict_execution_order` is enabled, the lazy module initialization order is already
      // guaranteed.
      return;
    }
    chunk_graph
      .chunk_table
      .iter_mut()
      .filter(|chunk| matches!(chunk.kind, ChunkKind::EntryPoint { .. }))
      .for_each(|chunk| {
        let ChunkKind::EntryPoint { module: entry_module, .. } = &chunk.kind else {
          return;
        };
        // After modules in chunk is sorted, it is always sorted by execution order whatever the
        // `chunk_modules_order` is `exec_order` or `module_id`. Because for `module_id` we only sort
        // by `module_id` for side effects free leaf modules, those should always execute first and
        // has no wrapping.
        let mut wrapped_modules = vec![];
        // If a none wrapped module has higher execution order than a wrapped module
        // we called the none wrapped module depended on the wrapped module(e.g. the none wrapped
        // module may depended on a global variable initialization in the wrapped module, however
        // the wrapped module are usually lazy evaluate). So we need to adjust the initialization
        // order
        // manually.
        let imported_symbol_owner_from_other_chunk = chunk
          .imports_from_other_chunks
          .iter()
          .flat_map(|(_, import_items)| {
            import_items
              .iter()
              .map(|item| self.link_output.symbol_db.canonical_ref_for(item.import_ref).owner)
          })
          .filter_map(|idx| {
            let module = self.link_output.module_table[idx].as_normal()?;
            (!self.link_output.metas[module.idx].original_wrap_kind().is_none()).then_some(idx)
          })
          .collect::<FxHashSet<_>>();
        let chunk_module_to_exec_order = chunk
          .modules
          .iter()
          .chain(imported_symbol_owner_from_other_chunk.iter())
          .map(|idx| (*idx, self.link_output.module_table[*idx].exec_order()))
          .collect::<FxHashMap<_, _>>();

        // the key is the module_idx of none wrapped module
        // the value is the how many wrapped modules did the none wrapped module depends on.
        // when getting all depended wrapped modules, just use wrapped_modules[0..none_wrapped_module_to_wrapped_dependency_length[none_wrap_module_idx]].
        let mut none_wrapped_module_to_wrapped_dependency_length = FxHashMap::default();
        let js_import_order = self.js_import_order(*entry_module, &chunk_module_to_exec_order);
        for idx in js_import_order {
          match self.link_output.metas[idx].original_wrap_kind() {
            WrapKind::None => {
              if !wrapped_modules.is_empty() {
                none_wrapped_module_to_wrapped_dependency_length.insert(idx, wrapped_modules.len());
              }
            }
            WrapKind::Cjs | WrapKind::Esm => {
              wrapped_modules.push(idx);
            }
          }
        }
        // All modules that we need to ensure the initialization order.
        let mut modules_need_to_check: FxHashSet<ModuleIdx> = FxHashSet::default();
        let mut max_length = 0;
        for (none_wrapped, dep_length) in &none_wrapped_module_to_wrapped_dependency_length {
          modules_need_to_check.insert(*none_wrapped);
          max_length = max_length.max(*dep_length);
        }
        modules_need_to_check.extend(&wrapped_modules[0..max_length]);

        if modules_need_to_check.is_empty() {
          // No wrapped modules or none wrapped modules that depends on wrapped modules, so we can
          // skip the initialization order check.
          return;
        }

        // Record each module in `modules_need_to_check` first init position.
        let mut module_init_position = FxIndexMap::default();

        for idx in &chunk.modules {
          let Some(module) = self.link_output.module_table[*idx].as_normal() else {
            continue;
          };
          module
            .import_records
            .iter_enumerated()
            .filter_map(|(rec_idx, rec)| {
              rec.resolved_module.map(|module_idx| (rec_idx, rec, module_idx))
            })
            .for_each(|(rec_idx, rec, module_idx)| {
              if rec.kind == ImportKind::Import && modules_need_to_check.contains(&module_idx) {
                module_init_position.entry(module_idx).or_insert((*idx, rec_idx));
              }
            });
          if module_init_position.len() == modules_need_to_check.len() {
            break;
          }
        }

        let mut module_init_position = module_init_position.into_iter().collect_vec();
        module_init_position.sort_by_cached_key(|(idx, _)| chunk_module_to_exec_order[idx]);

        let mut pending_transfer = vec![];
        let mut insert_map: FxHashMap<ModuleIdx, Vec<(ModuleIdx, ImportRecordIdx)>> =
          FxHashMap::default();
        let mut remove_map: FxHashMap<ModuleIdx, Vec<ImportRecordIdx>> = FxHashMap::default();
        for (module_idx, (importer_idx, rec_idx)) in module_init_position {
          match self.link_output.metas[module_idx].original_wrap_kind() {
            WrapKind::None => {
              if let Some(deps_length) =
                none_wrapped_module_to_wrapped_dependency_length.get(&module_idx)
              {
                let transfer_item = pending_transfer
                  .extract_if(0.., |(midx, _, _)| wrapped_modules[0..*deps_length].contains(midx));
                for (_midx, iidx, ridx) in transfer_item {
                  // Should always avoid transfer any initialization from a low execution order module to a high execution order module.
                  if chunk_module_to_exec_order[&iidx] <= chunk_module_to_exec_order[&module_idx] {
                    // If the module is the same, we can skip the transfer.
                    continue;
                  }
                  insert_map.entry(module_idx).or_default().push((iidx, ridx));
                  remove_map.entry(iidx).or_default().push(ridx);
                }
              }
            }
            WrapKind::Cjs | WrapKind::Esm => {
              pending_transfer.push((module_idx, importer_idx, rec_idx));
            }
          }
        }
        chunk.insert_map = insert_map;
        chunk.remove_map = remove_map;
      });
  }

  /// Only considering module eager initialization order, both `require()` and `import()` are lazy
  /// initialization.
  fn js_import_order(
    &self,
    entry: ModuleIdx,
    chunk_modules_map: &FxHashMap<ModuleIdx, u32>,
  ) -> Vec<ModuleIdx> {
    // traverse module graph with depth-first search to determine the order of JS imports
    let mut stack = vec![entry];
    let mut visited = FxHashSet::default();
    let mut js_import_order = vec![];
    while let Some(module_idx) = stack.pop() {
      if !visited.insert(module_idx) {
        continue;
      }
      let Some(normal_module) = self.link_output.module_table[module_idx].as_normal() else {
        continue;
      };
      js_import_order.push(module_idx);
      normal_module
        .import_records
        .iter()
        .rev()
        .filter_map(|rec| rec.resolved_module.map(|module_idx| (rec, module_idx)))
        .for_each(|(rec, module_idx)| {
          if rec.kind == ImportKind::Import && chunk_modules_map.contains_key(&module_idx) {
            stack.push(module_idx);
          }
        });
    }
    js_import_order
  }

  pub fn merge_cjs_namespace(&mut self, chunk_graph: &mut ChunkGraph) {
    let mut chunk_list: IndexVec<ChunkIdx, FxHashMap<(ModuleIdx, usize), Vec<SymbolRef>>> =
      index_vec![FxHashMap::default(); chunk_graph.chunk_table.len()];
    for (k, info) in &self.link_output.safely_merge_cjs_ns_map {
      for symbol_ref in info
        .namespace_refs
        .iter()
        .filter_map(|ns| {
          // We must check statement inclusion here (not in linking stage) because
          // `include_statements` runs after `reference_needed_symbols` where the
          // `safely_merge_cjs_ns_map` is populated. At that point, we don't yet
          // know which statements will be tree-shaken.
          // related context: https://github.com/rolldown/rolldown/blob/dbd0f6de5d44be2327e7532bb6f0a38bc04a1047/crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs#L187-L194
          let importer = self.link_output.module_table[ns.owner].as_normal()?;
          let is_stmt_included = importer
            .stmt_infos
            .declared_stmts_by_symbol(ns)
            .iter()
            .all(|item| self.link_output.metas[importer.idx].stmt_info_included[*item]);
          is_stmt_included.then_some(ns)
        })
        // Determine safely merged cjs ns binding should put in where
        // We should put it in the importRecord which first reference the cjs ns binding.
        .sorted_by_key(|item| self.link_output.module_table[item.owner].exec_order())
      {
        let owner = symbol_ref.owner;
        let chunk_idx = chunk_graph.module_to_chunk[owner].expect("Module should be in chunk");

        let group_idx = if self.link_output.metas[owner].wrap_kind().is_none() {
          Some(usize::MAX)
        } else {
          chunk_graph.chunk_table[chunk_idx].module_idx_to_group_idx.get(&owner).copied()
        };

        let Some(group_idx) = group_idx else {
          continue;
        };
        chunk_list[chunk_idx].entry((*k, group_idx)).or_default().push(*symbol_ref);
      }
    }

    for (chunk_idx, mut safely_merge_cjs_ns_map) in chunk_list.into_iter_enumerated() {
      let finalized_cjs_ns_map = &mut chunk_graph.finalized_cjs_ns_map_idx_vec[chunk_idx];
      for symbol_refs in safely_merge_cjs_ns_map.values_mut() {
        let mut iter = symbol_refs.iter();
        let first = iter.next();
        if let Some(first) = first {
          finalized_cjs_ns_map.insert(*first, *first);
          for symbol_ref in iter {
            self.link_output.symbol_db.link(*symbol_ref, *first);
            finalized_cjs_ns_map.insert(*symbol_ref, *first);
          }
        }
      }
    }
  }

  /// Finalizes which module namespace objects should be included in the output bundle.
  ///
  /// This method determines whether each ESM module's namespace object (e.g., `import * as ns from './module'`)
  /// should be kept in the final bundle based on how it's used. This is an optimization to avoid
  /// generating unnecessary namespace objects that aren't actually referenced.
  ///
  /// The decision is based on three cases:
  /// 1. **Unknown usage**: Keep the namespace (conservative approach when we can't prove it's unused)
  /// 2. **Re-exporting external modules**: Keep only if the module has dynamic exports after flattening
  ///    entry-level external modules (see [`find_entry_level_external_module`]), this is used for
  ///    indirect external module re-exports optimization.
  /// 3. **All other cases**: Remove the namespace object
  ///
  pub fn finalized_module_namespace_ref_usage(&mut self) {
    let to_eliminate = self
      .link_output
      .module_table
      .iter_enumerated()
      .filter_map(|(module_idx, module)| {
        let m = module.as_normal()?;
        let meta = &self.link_output.metas[module_idx];

        let module_namespace_included_reason = &meta.module_namespace_included_reason;
        let is_namespace_referenced = matches!(m.exports_kind, ExportsKind::Esm)
          && if module_namespace_included_reason.intersects(
            ModuleNamespaceIncludedReason::Unknown
              | ModuleNamespaceIncludedReason::SimulateFacadeChunk,
          ) {
            true
          } else if module_namespace_included_reason
            .contains(ModuleNamespaceIncludedReason::ReExportDynamicExports)
          {
            // If the module namespace is only used to reexport external module,
            // then we need to ensure if it is still has dynamic exports after flatten entry level
            // external module, see `find_entry_level_external_module`
            meta.has_dynamic_exports
          } else {
            false
          };
        Some((m.namespace_object_ref, is_namespace_referenced))
      })
      .collect_vec();
    for (namespace_ref, flag) in to_eliminate {
      if flag {
        self.link_output.used_symbol_refs.insert(namespace_ref);
      } else {
        self.link_output.used_symbol_refs.remove(&namespace_ref);
      }
    }
  }

  /// Find all entry level external modules, and re propagate `has_dynamic_exports` for affected modules.
  fn find_entry_level_external_module(&mut self, chunk_graph: &mut ChunkGraph) {
    let module_to_entry_level_external_rec_list_maps = chunk_graph
      .chunk_table
      .par_iter_enumerated()
      .filter_map(|(idx, chunk)| {
        let ChunkKind::EntryPoint { module, .. } = &chunk.kind else {
          return None;
        };
        let mut q = VecDeque::from_iter([*module]);
        let mut visited = FxHashSet::default();
        let mut entry_external_module_map: FxHashMap<ModuleIdx, Vec<ImportRecordIdx>> =
          FxHashMap::default();
        while let Some(module_idx) = q.pop_front() {
          if !visited.insert(module_idx) {
            continue;
          }
          let Module::Normal(module) = &self.link_output.module_table[module_idx] else {
            // In theory we will not append external module to `q`.
            continue;
          };
          module
            .import_records
            .iter_enumerated()
            .filter_map(|(idx, rec)| rec.resolved_module.map(|module_idx| (idx, rec, module_idx)))
            .for_each(|(idx, rec, resolved_module_idx)| {
              if !rec.meta.contains(ImportRecordMeta::IsExportStar) {
                return;
              }
              match &self.link_output.module_table[resolved_module_idx] {
                Module::Normal(_) => {
                  q.push_back(resolved_module_idx);
                }
                Module::External(_) => {
                  entry_external_module_map.entry(module_idx).or_default().push(idx);
                }
              }
            });
        }
        (!entry_external_module_map.is_empty()).then_some((idx, entry_external_module_map))
      })
      .collect::<Vec<(ChunkIdx, FxHashMap<ModuleIdx, Vec<ImportRecordIdx>>)>>();
    let mut invalidated_modules = FxHashSet::default();
    for (chunk_idx, entry_external_module_map) in module_to_entry_level_external_rec_list_maps {
      let mut entry_level_external_modules = FxHashSet::default();
      for (module_idx, rec_list) in entry_external_module_map {
        let Some(module) = self.link_output.module_table[module_idx].as_normal_mut() else {
          continue;
        };
        // If a module namespace is not included due to reexport a entry level external module, we
        // can't do any further optimization. e.g.
        // ```js
        // // index.js
        // import * as ns from './lib.js';
        // export {ns}
        // // lib.js
        // export * from 'external'
        // ```
        // since the `ns` is exported from entry module, it(namespace object) needs to include all exported symbol
        // from external module.
        for rec_idx in rec_list {
          let rec = &mut module.import_records[rec_idx];
          if let Some(module_idx) = rec.resolved_module {
            rec.meta.insert(ImportRecordMeta::EntryLevelExternal);
            entry_level_external_modules.insert(module_idx);
          }
        }

        if !self.link_output.metas[module_idx]
          .module_namespace_included_reason
          .contains(ModuleNamespaceIncludedReason::Unknown)
        {
          invalidated_modules.insert(module.idx);
        }
      }
      let mut vec = entry_level_external_modules.into_iter().collect_vec();
      vec.sort_by_key(|idx| self.link_output.module_table[*idx].exec_order());
      chunk_graph.chunk_table[chunk_idx].entry_level_external_module_idx = vec;
    }
    // re propagate `meta.has_dynamic_exports` for affect modules
    let mut q = invalidated_modules.iter().copied().collect::<VecDeque<_>>();
    while let Some(idx) = q.pop_front() {
      if !invalidated_modules.insert(idx) {
        continue;
      }
      let Module::Normal(module) = &self.link_output.module_table[idx] else {
        continue;
      };
      q.extend(module.importers_idx.iter());
    }

    if invalidated_modules.is_empty() {
      return;
    }

    let mut visited = FxHashSet::with_capacity(invalidated_modules.len());
    for module_idx in invalidated_modules.clone() {
      propagate_has_dynamic_exports(
        module_idx,
        &self.link_output.module_table,
        &mut self.link_output.metas,
        &mut visited,
        &mut invalidated_modules,
      );
    }
  }

  // ref:
  // - https://github.com/rollup/rollup/blob/99d4bee3277b96b30e871fb471f6c7ed55f94850/src/Bundle.ts?plain=1#L267-L278
  // - https://github.com/rollup/rollup/blob/99d4bee3277b96b30e871fb471f6c7ed55f94850/src/utils/commondir.ts?plain=1#L4-L24
  pub fn get_common_dir_of_all_modules(&self, modules: &[Module]) -> Option<String> {
    let mut ret: Option<String> = None;
    let iter = modules.iter().filter_map(|m| match m {
      Module::Normal(item) => {
        if !self.link_output.metas[item.idx].is_included {
          return None;
        }
        if self.options.preserve_modules || item.is_user_defined_entry {
          Path::new(item.id.as_ref()).is_absolute().then_some(item.id.as_ref())
        } else {
          None
        }
      }
      Module::External(external_module) => {
        if self.options.preserve_modules {
          Path::new(external_module.id.as_str())
            .is_absolute()
            .then_some(external_module.id.as_ref())
        } else {
          None
        }
      }
    });
    let mut modules_count = 0;
    for id in iter {
      if let Some(ref mut ret_id) = ret {
        *ret_id = commondir::extract_longest_common_path(ret_id.as_str(), id);
      } else {
        ret = Some(id.to_string());
      }
      modules_count += 1;
    }
    match modules_count {
      0 => None,
      1 => ret.and_then(|item| Path::new(&item).parent().map(|p| p.to_string_lossy().to_string())),
      _ => ret,
    }
  }

  fn init_entry_point(
    &self,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    entries_len: u32,
    input_base: &ArcStr,
  ) {
    // Create chunk for each static and dynamic entry
    for (entry_index, (&module_idx, entry_point)) in self
      .link_output
      .entries
      .iter()
      .flat_map(|(idx, entries)| entries.iter().map(move |e| (idx, e)))
      .enumerate()
    {
      let Module::Normal(module) = &self.link_output.module_table[module_idx] else {
        continue;
      };

      let count: u32 = entry_index.try_into().expect("Too many entries, u32 overflowed.");
      let mut bits = BitSet::new(entries_len);
      bits.set_bit(count);

      // Override `preserve_entry_signatures` if the entry point emitted by `this.emitFile({})` has
      // specified `preserveSignatures`.
      let finalized_preserve_entry_signatures = normalize_preserve_entry_signature(
        &self.link_output.overrode_preserve_entry_signature_map,
        self.options,
        module.idx,
      );

      let preserve_entry_signature = if module.is_user_defined_entry {
        match finalized_preserve_entry_signatures {
          PreserveEntrySignatures::AllowExtension
          | PreserveEntrySignatures::Strict
          | PreserveEntrySignatures::False => Some(finalized_preserve_entry_signatures),
          PreserveEntrySignatures::ExportsOnly => {
            let meta = &self.link_output.metas[module.idx];
            if meta.sorted_and_non_ambiguous_resolved_exports.is_empty() {
              Some(PreserveEntrySignatures::AllowExtension)
            } else {
              Some(PreserveEntrySignatures::Strict)
            }
          }
        }
      } else {
        None
      };
      let mut chunk = Chunk::new(
        entry_point.name.clone(),
        entry_point.file_name.clone(),
        bits.clone(),
        vec![],
        ChunkKind::EntryPoint {
          meta: {
            let mut meta = ChunkMeta::default();
            meta.set(
              ChunkMeta::UserDefinedEntry,
              matches!(entry_point.kind, EntryPointKind::UserDefined),
            );
            meta.set(ChunkMeta::DynamicImported, !module.dynamic_importers.is_empty());
            meta.set(ChunkMeta::EmittedChunk, entry_point.kind.is_emitted_user_defined());
            meta
          },
          bit: count,
          module: module_idx,
        },
        input_base.clone(),
        preserve_entry_signature,
      );
      chunk.add_creation_reason(
        ChunkCreationReason::Entry {
          is_user_defined_entry: module.is_user_defined_entry,
          entry_module_id: &module.debug_id,
          name: entry_point.name.as_ref(),
        },
        self.options,
      );
      let chunk_idx = chunk_graph.add_chunk(chunk);
      if let Some(reference_ids) = self.link_output.entry_point_to_reference_ids.get(entry_point) {
        chunk_graph.chunk_idx_to_reference_ids.insert(chunk_idx, reference_ids.clone());
      }

      bits_to_chunk.insert(bits, chunk_idx);
      // Use or_insert to keep the first entry's chunk when multiple entry points share the same module
      chunk_graph.entry_module_to_entry_chunk.entry(module_idx).or_insert(chunk_idx);
    }
  }

  async fn split_chunks(
    &mut self,
    index_splitting_info: &mut IndexSplittingInfo,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
  ) -> BuildResult<()> {
    // Determine which modules belong to which chunk. A module could belong to multiple chunks.
    for (entry_index, (&module_idx, _)) in self
      .link_output
      .entries
      .iter()
      .flat_map(|(idx, entries)| entries.iter().map(move |e| (idx, e)))
      .enumerate()
    {
      self.determine_reachable_modules_for_entry(
        module_idx,
        entry_index.try_into().expect("Too many entries, u32 overflowed."),
        index_splitting_info,
      );
    }

    let mut module_to_assigned: IndexVec<ModuleIdx, bool> =
      oxc_index::index_vec![false; self.link_output.module_table.modules.len()];

    self
      .apply_manual_code_splitting(
        index_splitting_info,
        &mut module_to_assigned,
        chunk_graph,
        input_base,
      )
      .await?;

    // If it is allow to allow that entry chunks have the different exports as the underlying entry module.
    // This is used to generate less chunks when possible.
    // TODO: maybe we could bailout peer chunk?
    let allow_chunk_optimization = self.options.experimental.is_chunk_optimization_enabled()
      && !self.link_output.metas.iter().any(|meta| meta.is_tla_or_contains_tla_dependency);
    let mut temp_chunk_graph =
      ChunkOptimizationGraph::new(allow_chunk_optimization, chunk_graph, bits_to_chunk);

    // 1. Assign modules to corresponding chunks
    // 2. Create shared chunks to store modules that belong to multiple chunks.
    for idx in &self.link_output.sorted_modules {
      let Some(normal_module) = self.link_output.module_table[*idx].as_normal() else {
        continue;
      };
      if !self.link_output.metas[normal_module.idx].is_included {
        continue;
      }

      if module_to_assigned[normal_module.idx] {
        continue;
      }

      module_to_assigned[normal_module.idx] = true;

      let bits = &index_splitting_info[normal_module.idx].bits;
      debug_assert!(
        !bits.is_empty(),
        "Empty bits means the module is not reachable, so it should bail out with `is_included: false` {:?}",
        normal_module.stable_id
      );
      if let Some(chunk_id) = bits_to_chunk.get(bits).copied() {
        chunk_graph.add_module_to_chunk(
          normal_module.idx,
          chunk_id,
          self.link_output.metas[normal_module.idx].depended_runtime_helper,
        );
        if allow_chunk_optimization {
          temp_chunk_graph.add_module_to_chunk(normal_module.idx, chunk_id);
        }
      } else if normal_module.is_user_defined_entry
        && self.link_output.metas[normal_module.idx].wrap_kind().is_none()
        // Don't apply this optimization when multiple entries point to the same module
        // (duplicate entries). In that case, we need the normal chunk optimization to
        // ensure the second entry properly imports from the first.
        && self
          .link_output
          .entries
          .get(&normal_module.idx)
          .is_some_and(|entries| entries.len() <= 1)
      {
        // User-defined entry modules that are NOT wrapped should stay in their own entry chunk,
        // even when reachable from multiple entries. This avoids creating unnecessary
        // common chunks that would turn the entry into a facade.
        //
        // Wrapped modules (CJS or ESM wrapping for circular dependencies) need to go through
        // the normal chunk optimization to ensure proper execution semantics.
        let entry_chunk_idx = chunk_graph.entry_module_to_entry_chunk.get(&normal_module.idx);
        debug_assert!(
          entry_chunk_idx.is_some(),
          "User-defined entry module should have an entry chunk"
        );
        if let Some(&entry_chunk_idx) = entry_chunk_idx {
          chunk_graph.add_module_to_chunk(
            normal_module.idx,
            entry_chunk_idx,
            self.link_output.metas[normal_module.idx].depended_runtime_helper,
          );

          if allow_chunk_optimization {
            temp_chunk_graph.add_module_to_chunk(normal_module.idx, entry_chunk_idx);
          }
        }
      } else if allow_chunk_optimization {
        temp_chunk_graph.init_module_assignment(normal_module.idx, bits);
      } else {
        let mut chunk =
          Chunk::new(None, None, bits.clone(), vec![], ChunkKind::Common, input_base.clone(), None);
        chunk.add_creation_reason(
          ChunkCreationReason::CommonChunk { bits, link_output: self.link_output },
          self.options,
        );
        let chunk_id = chunk_graph.add_chunk(chunk);
        chunk_graph.add_module_to_chunk(
          normal_module.idx,
          chunk_id,
          self.link_output.metas[normal_module.idx].depended_runtime_helper,
        );
        bits_to_chunk.insert(bits.clone(), chunk_id);
      }
    }

    if allow_chunk_optimization {
      temp_chunk_graph.calc_chunk_dependencies(&self.link_output.metas);

      self.try_insert_common_module_to_exist_chunk(
        chunk_graph,
        bits_to_chunk,
        input_base,
        &mut temp_chunk_graph,
      );

      self.optimize_facade_dynamic_entry_chunks(
        chunk_graph,
        index_splitting_info,
        input_base,
        &mut module_to_assigned,
        &temp_chunk_graph,
      );
    }

    Ok(())
  }

  fn determine_reachable_modules_for_entry(
    &self,
    entry_module_idx: ModuleIdx,
    entry_index: u32,
    index_splitting_info: &mut IndexSplittingInfo,
  ) {
    let mut q = VecDeque::from([entry_module_idx]);
    while let Some(module_idx) = q.pop_front() {
      if !self.link_output.module_table[module_idx].is_normal() {
        continue;
      }

      let meta = &self.link_output.metas[module_idx];

      if !self.link_output.metas[module_idx].is_included {
        continue;
      }

      if index_splitting_info[module_idx].bits.has_bit(entry_index) {
        continue;
      }

      index_splitting_info[module_idx].bits.set_bit(entry_index);
      index_splitting_info[module_idx].share_count += 1;
      meta.dependencies.iter().copied().for_each(|dep_idx| {
        q.push_back(dep_idx);
      });
    }
  }
}

fn propagate_has_dynamic_exports(
  target: ModuleIdx,
  modules: &IndexModules,
  linking_infos: &mut LinkingMetadataVec,
  visited_modules: &mut FxHashSet<ModuleIdx>,
  invalidate_modules: &mut FxHashSet<ModuleIdx>,
) -> bool {
  if !invalidate_modules.contains(&target) || visited_modules.contains(&target) {
    return linking_infos[target].has_dynamic_exports;
  }
  visited_modules.insert(target);

  let has_dynamic_exports = match &modules[target] {
    Module::Normal(module) => {
      if matches!(module.exports_kind, ExportsKind::CommonJs) {
        true
      } else {
        module
          .import_records
          .iter()
          .filter_map(|rec| rec.resolved_module.map(|module_idx| (rec, module_idx)))
          .any(|(rec, module_idx)| {
            if module_idx == target || !rec.meta.contains(ImportRecordMeta::IsExportStar) {
              return false;
            }
            if rec.meta.contains(ImportRecordMeta::EntryLevelExternal) {
              return false;
            }
            propagate_has_dynamic_exports(
              module_idx,
              modules,
              linking_infos,
              visited_modules,
              invalidate_modules,
            )
          })
      }
    }
    Module::External(_) => true,
  };

  linking_infos[target].has_dynamic_exports = has_dynamic_exports;
  invalidate_modules.remove(&target);
  has_dynamic_exports
}
