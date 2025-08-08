use std::{cmp::Ordering, collections::VecDeque, path::Path};

use crate::{
  chunk_graph::ChunkGraph, stages::generate_stage::chunk_ext::ChunkDebugExt,
  types::linking_metadata::LinkingMetadataVec, utils::chunk::normalize_preserve_entry_signature,
};
use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, ChunkMeta, ExportsKind, ImportKind, ImportRecordIdx,
  ImportRecordMeta, IndexModules, Module, ModuleIdx, ModuleNamespaceIncludedReason,
  PreserveEntrySignatures, WrapKind,
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

#[derive(Debug)]
enum CombineChunkRet {
  DynamicVec(Vec<ChunkIdx>),
  Entry(ChunkIdx),
  None,
}

pub type IndexSplittingInfo = IndexVec<ModuleIdx, SplittingInfo>;

impl GenerateStage<'_> {
  #[allow(clippy::too_many_lines)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate_chunks(&mut self) -> BuildResult<ChunkGraph> {
    let entries_len: u32 =
      self.link_output.entries.len().try_into().expect("Too many entries, u32 overflowed.");
    // If we are in test environment, to make the runtime module always fall into a standalone chunk,
    // we create a facade entry point for it.

    let mut chunk_graph = ChunkGraph::new(self.link_output.module_table.modules.len());
    chunk_graph.chunk_table.chunks.reserve(self.link_output.entries.len());

    let mut index_splitting_info: IndexSplittingInfo = oxc_index::index_vec![SplittingInfo {
        bits: BitSet::new(entries_len),
        share_count: 0
      }; self.link_output.module_table.modules.len()];
    let mut bits_to_chunk = FxHashMap::with_capacity(self.link_output.entries.len());

    let mut entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx> =
      FxHashMap::with_capacity(self.link_output.entries.len());
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
          self.link_output.entries.iter().find(|entry_point| entry_point.id == module.idx);
        if !module.is_included() {
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
              meta.set(ChunkMeta::UserDefinedEntry, module.is_user_defined_entry);
              meta.set(ChunkMeta::DynamicImported, !module.dynamic_importers.is_empty());
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
        if let Some(entry) = matched_entry {
          if let Some(reference_ids) = self.link_output.entry_point_to_reference_ids.get(entry) {
            chunk_graph.chunk_idx_to_reference_ids.insert(chunk_idx, reference_ids.clone());
          }
        }
        chunk_graph.add_module_to_chunk(
          module.idx,
          chunk_idx,
          self.link_output.metas[module.idx].depended_runtime_helper,
        );
        // bits_to_chunk.insert(bits, chunk); // This line is intentionally commented out because `bits_to_chunk` is not used in this loop. It is updated elsewhere in the `init_entry_point` and `split_chunks` methods.
        entry_module_to_entry_chunk.insert(module.idx, chunk_idx);
      }
    } else {
      self.init_entry_point(
        &mut chunk_graph,
        &mut bits_to_chunk,
        &mut entry_module_to_entry_chunk,
        entries_len,
        &input_base,
      );
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
          module.meta.is_included().then_some(item)
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
    chunk_graph.entry_module_to_entry_chunk = entry_module_to_entry_chunk;

    self.merge_cjs_namespace(&mut chunk_graph);
    self.find_entry_level_external_module(&mut chunk_graph);
    self.ensure_lazy_module_initialization_order(&mut chunk_graph);

    Ok(chunk_graph)
  }

  fn ensure_lazy_module_initialization_order(&self, chunk_graph: &mut ChunkGraph) {
    if self.options.experimental.strict_execution_order.unwrap_or_default() {
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
        // `chunk_modules_order` is `exec_order` or `module_id`, because for `module_id` we only sort
        // by `module_id` for side effects free leaf modules, those should always execute first and
        // has no wrapping.
        let mut wrapped_modules = vec![];
        // If a none wrapped module has higher execution order than a wrapped module
        // we called the none wrapped module depended on the wrapped module(e.g. the none wrapped
        // module may depended on a global variable initialization in the wrapped module, however
        // the wrapped module are usually lazy evaluate). So we need to adjust the initialization
        // order
        // manually.
        let mut none_wrapped_module_depends_wrapped_modules: FxHashMap<ModuleIdx, Vec<ModuleIdx>> =
          FxHashMap::default();
        let chunk_module_to_exec_order = chunk
          .modules
          .iter()
          .map(|idx| (*idx, self.link_output.module_table[*idx].exec_order()))
          .collect::<FxHashMap<_, _>>();

        let js_import_order = self.js_import_order(*entry_module, &chunk_module_to_exec_order);
        for idx in js_import_order {
          match self.link_output.metas[idx].wrap_kind {
            WrapKind::None => {
              if !wrapped_modules.is_empty() {
                none_wrapped_module_depends_wrapped_modules
                  .entry(idx)
                  .or_default()
                  .extend(wrapped_modules.iter().copied());
              }
            }
            WrapKind::Cjs | WrapKind::Esm => {
              wrapped_modules.push(idx);
            }
          }
        }
        // All modules that we need to ensure the initialization order.
        let mut modules_need_to_check: FxHashSet<ModuleIdx> = FxHashSet::default();
        for (none_wrapped, deps) in &none_wrapped_module_depends_wrapped_modules {
          modules_need_to_check.insert(*none_wrapped);
          modules_need_to_check.extend(deps.iter().copied());
        }

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

          for (rec_idx, rec) in module.import_records.iter_enumerated().filter(|(_idx, rec)| {
            matches!(rec.kind, ImportKind::Import)
              && modules_need_to_check.contains(&rec.resolved_module)
          }) {
            module_init_position.entry(rec.resolved_module).or_insert((*idx, rec_idx));
          }
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
          match self.link_output.metas[module_idx].wrap_kind {
            WrapKind::None => {
              if let Some(deps) = none_wrapped_module_depends_wrapped_modules.get(&module_idx) {
                let transfer_item =
                  pending_transfer.extract_if(0.., |(midx, _, _)| deps.contains(midx));
                for (_midx, iidx, ridx) in transfer_item {
                  if module_idx == iidx {
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
      for rec in normal_module.import_records.iter().rev().filter(|rec| {
        chunk_modules_map.contains_key(&rec.resolved_module) && rec.kind == ImportKind::Import
      }) {
        stack.push(rec.resolved_module);
      }
    }
    js_import_order
  }

  fn merge_cjs_namespace(&mut self, chunk_graph: &mut ChunkGraph) {
    for (k, v) in &self.link_output.safely_merge_cjs_ns_map {
      for symbol_ref in v
        .iter()
        .filter(|item| {
          self.link_output.module_table[item.owner].as_normal().unwrap().is_included()
            && self.link_output.metas[item.owner].wrap_kind.is_none()
        })
        // Determine safely merged cjs ns binding should put in where
        // We should put it in the importRecord which first reference the cjs ns binding.
        .sorted_by_key(|item| self.link_output.module_table[item.owner].exec_order())
      {
        let owner = symbol_ref.owner;
        let chunk_idx = chunk_graph.module_to_chunk[owner].expect("Module should be in chunk");
        chunk_graph.safely_merge_cjs_ns_map_idx_vec[chunk_idx]
          .entry(*k)
          .or_default()
          .push(*symbol_ref);
      }
    }

    for (_, safely_merge_cjs_ns_map) in
      chunk_graph.safely_merge_cjs_ns_map_idx_vec.iter_mut_enumerated()
    {
      for symbol_refs in safely_merge_cjs_ns_map.values_mut() {
        let mut iter = symbol_refs.iter();
        let first = iter.next();
        if let Some(first) = first {
          for symbol_ref in iter {
            self.link_output.symbol_db.link(*symbol_ref, *first);
          }
        }
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
          for (idx, rec) in module.import_records.iter_enumerated() {
            if !rec.meta.contains(ImportRecordMeta::IsExportStar) {
              continue;
            }
            match &self.link_output.module_table[rec.resolved_module] {
              Module::Normal(_) => {
                q.push_back(rec.resolved_module);
              }
              Module::External(_) => {
                entry_external_module_map.entry(module_idx).or_default().push(idx);
              }
            }
          }
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
          rec.meta.insert(ImportRecordMeta::EntryLevelExternal);
          entry_level_external_modules.insert(rec.resolved_module);
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
        if !item.is_included() {
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
    entry_module_to_entry_chunk: &mut FxHashMap<ModuleIdx, ChunkIdx>,
    entries_len: u32,
    input_base: &ArcStr,
  ) {
    // Create chunk for each static and dynamic entry
    for (entry_index, entry_point) in self.link_output.entries.iter().enumerate() {
      let count: u32 = entry_index.try_into().expect("Too many entries, u32 overflowed.");
      let mut bits = BitSet::new(entries_len);
      bits.set_bit(count);
      let Module::Normal(module) = &self.link_output.module_table[entry_point.id] else {
        continue;
      };

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
            meta.set(ChunkMeta::UserDefinedEntry, module.is_user_defined_entry);
            meta.set(ChunkMeta::DynamicImported, !module.dynamic_importers.is_empty());
            meta
          },
          bit: count,
          module: entry_point.id,
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
      entry_module_to_entry_chunk.insert(entry_point.id, chunk_idx);
    }
  }

  async fn split_chunks(
    &self,
    index_splitting_info: &mut IndexSplittingInfo,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
  ) -> BuildResult<()> {
    // Determine which modules belong to which chunk. A module could belong to multiple chunks.
    self.link_output.entries.iter().enumerate().for_each(|(i, entry_point)| {
      self.determine_reachable_modules_for_entry(
        entry_point.id,
        i.try_into().expect("Too many entries, u32 overflowed."),
        index_splitting_info,
      );
    });

    let mut module_to_assigned: IndexVec<ModuleIdx, bool> =
      oxc_index::index_vec![false; self.link_output.module_table.modules.len()];

    self
      .apply_advanced_chunks(index_splitting_info, &mut module_to_assigned, chunk_graph, input_base)
      .await?;

    let mut pending_common_chunks: FxIndexMap<BitSet, Vec<ModuleIdx>> = FxIndexMap::default();
    // If it is allow to allow that entry chunks have the different exports as the underlying entry module.
    // This is used to generate less chunks when possible.
    let allow_extension_optimize = (!matches!(self.options.preserve_entry_signatures, PreserveEntrySignatures::Strict)
        || !self.link_output.overrode_preserve_entry_signature_map.is_empty())
          // partial workaround of https://github.com/rolldown/rolldown/issues/5026#issuecomment-2990146735
          // TODO: maybe we could bailout peer chunk?
        && !self.link_output.metas.iter().any(|meta| meta.is_tla_or_contains_tla_dependency);
    // 1. Assign modules to corresponding chunks
    // 2. Create shared chunks to store modules that belong to multiple chunks.
    for idx in &self.link_output.sorted_modules {
      let Some(normal_module) = self.link_output.module_table[*idx].as_normal() else {
        continue;
      };
      if !normal_module.meta.is_included() {
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
      } else if allow_extension_optimize {
        pending_common_chunks.entry(bits.clone()).or_default().push(normal_module.idx);
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

    if allow_extension_optimize {
      self.try_insert_common_module_to_exist_chunk(
        chunk_graph,
        bits_to_chunk,
        input_base,
        pending_common_chunks,
      );
    }
    Ok(())
  }

  fn try_insert_common_module_to_exist_chunk(
    &self,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
    pending_common_chunks: FxIndexMap<BitSet, Vec<ModuleIdx>>,
  ) {
    let static_entry_chunk_reference: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> =
      self.construct_static_entry_to_reached_dynamic_entries_map(chunk_graph);

    let entry_chunk_idx =
      chunk_graph.chunk_table.iter_enumerated().map(|(idx, _)| idx).collect::<FxHashSet<_>>();
    // extract entry chunk module relation
    // this means `key_chunk` also referenced all entry module in value `vec`
    for (bits, modules) in pending_common_chunks {
      let chunk_idxs = bits
        .index_of_one()
        .into_iter()
        .map(ChunkIdx::from_raw)
        // Some of the bits maybe not created yet, so filter it out.
        // refer https://github.com/rolldown/rolldown/blob/d373794f5ce5b793ac751bbfaf101cc9cdd261d9/crates/rolldown/src/stages/generate_stage/code_splitting.rs?plain=1#L311-L313
        .filter(|idx| entry_chunk_idx.contains(idx))
        .collect_vec();
      let item =
        Self::try_insert_into_exists_chunk(&chunk_idxs, &static_entry_chunk_reference, chunk_graph);
      match item {
        CombineChunkRet::Entry(chunk_idx)
          if !matches!(
            chunk_graph.chunk_table[chunk_idx].preserve_entry_signature,
            Some(PreserveEntrySignatures::Strict)
          ) =>
        {
          for module_idx in modules {
            chunk_graph.add_module_to_chunk(
              module_idx,
              chunk_idx,
              self.link_output.metas[module_idx].depended_runtime_helper,
            );
          }
        }
        CombineChunkRet::DynamicVec(_) | CombineChunkRet::None | CombineChunkRet::Entry(_) => {
          let mut chunk = Chunk::new(
            None,
            None,
            bits.clone(),
            vec![],
            ChunkKind::Common,
            input_base.clone(),
            None,
          );
          chunk.add_creation_reason(
            ChunkCreationReason::CommonChunk { bits: &bits, link_output: self.link_output },
            self.options,
          );
          let chunk_id = chunk_graph.add_chunk(chunk);
          for module_idx in modules {
            chunk_graph.add_module_to_chunk(
              module_idx,
              chunk_id,
              self.link_output.metas[module_idx].depended_runtime_helper,
            );
          }
          bits_to_chunk.insert(bits, chunk_id);
        }
      }
    }
  }

  fn try_insert_into_exists_chunk(
    chunk_idxs: &[ChunkIdx],
    entry_chunk_reference: &FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>>,
    chunk_graph: &ChunkGraph,
  ) -> CombineChunkRet {
    match chunk_idxs.len() {
      0 => CombineChunkRet::None,
      1 => {
        let Some(chunk) = &chunk_graph.chunk_table.get(chunk_idxs[0]) else {
          // Chunk idx maybe greater than the chunk table length.
          // Largest chunk idx equals to `entry.len() -1`.
          // But some of the bit in entry may not be created as a chunk.
          // refer https://github.com/rolldown/rolldown/blob/d373794f5ce5b793ac751bbfaf101cc9cdd261d9/crates/rolldown/src/stages/generate_stage/code_splitting.rs?plain=1#L311-L313
          return CombineChunkRet::None;
        };
        match chunk.kind {
          ChunkKind::EntryPoint { meta, .. } => {
            if meta.contains(ChunkMeta::UserDefinedEntry) {
              CombineChunkRet::Entry(chunk_idxs[0])
            } else {
              CombineChunkRet::DynamicVec(vec![chunk_idxs[0]])
            }
          }
          ChunkKind::Common => CombineChunkRet::None,
        }
      }
      _ => {
        let mid = chunk_idxs.len() / 2;
        let left = &chunk_idxs[0..mid];
        let right = &chunk_idxs[mid..];
        let left_ret = Self::try_insert_into_exists_chunk(left, entry_chunk_reference, chunk_graph);
        let right_ret =
          Self::try_insert_into_exists_chunk(right, entry_chunk_reference, chunk_graph);
        match (left_ret, right_ret) {
          (CombineChunkRet::DynamicVec(mut left), CombineChunkRet::DynamicVec(right)) => {
            left.extend(right);
            CombineChunkRet::DynamicVec(left)
          }
          (CombineChunkRet::DynamicVec(dynamic_entry_idxs), CombineChunkRet::Entry(chunk_idx)) => {
            let ret = dynamic_entry_idxs.iter().all(|idx| {
              entry_chunk_reference
                .get(&chunk_idx)
                .map(|reached_dynamic_chunk| reached_dynamic_chunk.contains(idx))
                .unwrap_or(false)
            });
            if ret { CombineChunkRet::Entry(chunk_idx) } else { CombineChunkRet::None }
          }
          (_, CombineChunkRet::None)
          | (CombineChunkRet::None, _)
          | (CombineChunkRet::Entry(_), CombineChunkRet::Entry(_)) => CombineChunkRet::None,
          (CombineChunkRet::Entry(chunk_idx), CombineChunkRet::DynamicVec(dynamic_entry_idxs)) => {
            let ret = dynamic_entry_idxs.iter().all(|idx| {
              entry_chunk_reference
                .get(&chunk_idx)
                .map(|reached_dynamic_chunk| reached_dynamic_chunk.contains(idx))
                .unwrap_or(false)
            });
            if ret { CombineChunkRet::Entry(chunk_idx) } else { CombineChunkRet::None }
          }
        }
      }
    }
  }

  fn construct_static_entry_to_reached_dynamic_entries_map(
    &self,
    chunk_graph: &ChunkGraph,
  ) -> FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> {
    let mut ret: FxHashMap<ChunkIdx, FxHashSet<ChunkIdx>> = FxHashMap::default();
    let dynamic_entry_modules = chunk_graph
      .chunk_table
      .iter_enumerated()
      .filter_map(|(idx, chunk)| match chunk.kind {
        ChunkKind::EntryPoint { meta, module, .. } => {
          (!meta.contains(ChunkMeta::UserDefinedEntry)).then_some((module, idx))
        }
        ChunkKind::Common => None,
      })
      .collect::<FxHashMap<ModuleIdx, ChunkIdx>>();
    for entry in self.link_output.entries.iter().filter(|item| item.kind.is_user_defined()) {
      let Some(entry_chunk_idx) = chunk_graph.module_to_chunk[entry.id] else {
        continue;
      };
      let mut q = VecDeque::from_iter([entry.id]);
      let mut visited = FxHashSet::default();
      while let Some(cur) = q.pop_front() {
        if visited.contains(&cur) {
          continue;
        }
        visited.insert(cur);
        let Module::Normal(module) = &self.link_output.module_table[cur] else {
          continue;
        };

        for rec in &module.import_records {
          // Can't put it at the beginning of the loop,
          if let Some(chunk_idx) = dynamic_entry_modules.get(&rec.resolved_module) {
            ret.entry(entry_chunk_idx).or_default().insert(*chunk_idx);
          }
          q.push_back(rec.resolved_module);
        }
      }
    }
    ret
  }

  fn determine_reachable_modules_for_entry(
    &self,
    entry_module_idx: ModuleIdx,
    entry_index: u32,
    index_splitting_info: &mut IndexSplittingInfo,
  ) {
    let mut q = VecDeque::from([entry_module_idx]);
    while let Some(module_idx) = q.pop_front() {
      let Module::Normal(module) = &self.link_output.module_table[module_idx] else {
        continue;
      };

      let meta = &self.link_output.metas[module_idx];

      if !module.meta.is_included() {
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
        module.import_records.iter().any(|rec| {
          if rec.resolved_module == target || !rec.meta.contains(ImportRecordMeta::IsExportStar) {
            return false;
          }
          if rec.meta.contains(ImportRecordMeta::EntryLevelExternal) {
            return false;
          }
          propagate_has_dynamic_exports(
            rec.resolved_module,
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
