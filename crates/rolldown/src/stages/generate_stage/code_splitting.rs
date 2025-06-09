use std::{cmp::Ordering, collections::VecDeque, path::Path};

use crate::{chunk_graph::ChunkGraph, stages::generate_stage::chunk_ext::ChunkDebugExt};
use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, Module, ModuleIdx, OutputFormat, PreserveEntrySignatures,
};
use rolldown_utils::{BitSet, commondir, indexmap::FxIndexMap, rustc_hash::FxHashMapExt};
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
  pub async fn generate_chunks(&mut self) -> anyhow::Result<ChunkGraph> {
    if matches!(self.options.format, OutputFormat::Iife | OutputFormat::Umd) {
      let user_defined_entry_count =
        self.link_output.entries.iter().filter(|entry| entry.kind.is_user_defined()).count();
      debug_assert!(user_defined_entry_count == 1, "IIFE/UMD format only supports one entry point");
    }
    let entries_len: u32 =
      self.link_output.entries.len().try_into().expect("Too many entries, u32 overflowed.");
    // If we are in test environment, to make the runtime module always fall into a standalone chunk,
    // we create a facade entry point for it.

    let mut chunk_graph = ChunkGraph::new(&self.link_output.module_table);
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
        if !module.is_included() {
          continue;
        }

        let count = idx.raw();
        let mut bits = BitSet::new(modules_len);
        bits.set_bit(count);
        let mut chunk = Chunk::new(
          None,
          None,
          None,
          bits.clone(),
          vec![],
          ChunkKind::EntryPoint {
            is_user_defined: module.is_user_defined_entry,
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
        let chunk = chunk_graph.add_chunk(chunk);
        chunk_graph.add_module_to_chunk(module.idx, chunk);
        // bits_to_chunk.insert(bits, chunk); // This line is intentionally commented out because `bits_to_chunk` is not used in this loop. It is updated elsewhere in the `init_entry_point` and `split_chunks` methods.
        entry_module_to_entry_chunk.insert(module.idx, chunk);
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
        for symbol in &group[1..] {
          self.link_output.symbol_db.link(**symbol, *group[0]);
        }
      }
    }

    // Sort modules in each chunk by execution order
    chunk_graph.chunk_table.iter_mut().for_each(|chunk| {
      chunk
        .modules
        .sort_unstable_by_key(|module_id| self.link_output.module_table[*module_id].exec_order());
    });

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
        ChunkKind::EntryPoint { is_user_defined, .. } if *is_user_defined => (0, index.raw()),
        _ => (1, chunk.exec_order),
      })
      .map(|(idx, _)| idx)
      .collect::<Vec<_>>();

    chunk_graph.sorted_chunk_idx_vec = sorted_chunk_idx_vec;
    chunk_graph.entry_module_to_entry_chunk = entry_module_to_entry_chunk;
    self.merge_cjs_namespace(&mut chunk_graph);
    Ok(chunk_graph)
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

      let preserve_entry_signature = if module.is_user_defined_entry {
        match self.options.preserve_entry_signatures {
          PreserveEntrySignatures::AllowExtension
          | PreserveEntrySignatures::Strict
          | PreserveEntrySignatures::False => Some(self.options.preserve_entry_signatures),
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
        entry_point.reference_id.clone(),
        entry_point.file_name.clone(),
        bits.clone(),
        vec![],
        ChunkKind::EntryPoint {
          is_user_defined: module.is_user_defined_entry,
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
      let chunk = chunk_graph.add_chunk(chunk);

      bits_to_chunk.insert(bits, chunk);
      entry_module_to_entry_chunk.insert(entry_point.id, chunk);
    }
  }

  async fn split_chunks(
    &self,
    index_splitting_info: &mut IndexSplittingInfo,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
  ) -> anyhow::Result<()> {
    // Determine which modules belong to which chunk. A module could belong to multiple chunks.
    self.link_output.entries.iter().enumerate().for_each(|(i, entry_point)| {
      if self.options.is_hmr_enabled() {
        // If HMR is enabled, we need to make sure it belongs to at least one chunk even if no module reaches it.
        self.determine_reachable_modules_for_entry(
          self.link_output.runtime.id(),
          i.try_into().expect("Too many entries, u32 overflowed."),
          index_splitting_info,
        );
      }
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
    let allow_extension_optimize =
      !matches!(self.options.preserve_entry_signatures, PreserveEntrySignatures::Strict);
    // 1. Assign modules to corresponding chunks
    // 2. Create shared chunks to store modules that belong to multiple chunks.
    for normal_module in self.link_output.module_table.modules.iter().filter_map(Module::as_normal)
    {
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
        chunk_graph.add_module_to_chunk(normal_module.idx, chunk_id);
      } else if allow_extension_optimize {
        pending_common_chunks.entry(bits.clone()).or_default().push(normal_module.idx);
      } else {
        let mut chunk = Chunk::new(
          None,
          None,
          None,
          bits.clone(),
          vec![],
          ChunkKind::Common,
          input_base.clone(),
          None,
        );
        chunk.add_creation_reason(
          ChunkCreationReason::CommonChunk { bits, link_output: self.link_output },
          self.options,
        );
        let chunk_id = chunk_graph.add_chunk(chunk);
        chunk_graph.add_module_to_chunk(normal_module.idx, chunk_id);
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
    // extract entry chunk module relation
    // this means `key_chunk` also referenced all entry module in value `vec`
    for (bits, modules) in pending_common_chunks {
      let item = Self::try_insert_into_exists_chunk(
        &bits.index_of_one().into_iter().map(ChunkIdx::from_raw).collect_vec(),
        &static_entry_chunk_reference,
        chunk_graph,
      );
      match item {
        CombineChunkRet::Entry(chunk_idx)
          if !matches!(
            chunk_graph.chunk_table[chunk_idx].preserve_entry_signature,
            Some(PreserveEntrySignatures::Strict)
          ) =>
        {
          for m in modules {
            chunk_graph.add_module_to_chunk(m, chunk_idx);
          }
        }
        CombineChunkRet::DynamicVec(_) | CombineChunkRet::None | CombineChunkRet::Entry(_) => {
          let mut chunk = Chunk::new(
            None,
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
            chunk_graph.add_module_to_chunk(module_idx, chunk_id);
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
        let chunk = &chunk_graph.chunk_table[chunk_idxs[0]];
        match chunk.kind {
          ChunkKind::EntryPoint { is_user_defined, .. } => {
            if is_user_defined {
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
        ChunkKind::EntryPoint { is_user_defined, module, .. } => {
          (!is_user_defined).then_some((module, idx))
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
