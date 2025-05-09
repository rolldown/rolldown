use std::{cmp::Ordering, path::Path};

use crate::chunk_graph::ChunkGraph;
use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{Chunk, ChunkIdx, ChunkKind, Module, ModuleIdx, OutputFormat};
use rolldown_utils::{BitSet, commondir, rustc_hash::FxHashMapExt};
use rustc_hash::FxHashMap;

use super::GenerateStage;

#[derive(Clone)]
pub struct SplittingInfo {
  pub bits: BitSet,
  pub share_count: u32,
}

pub type IndexSplittingInfo = IndexVec<ModuleIdx, SplittingInfo>;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn generate_chunks(&mut self) -> ChunkGraph {
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

        let count = idx.raw();
        let mut bits = BitSet::new(modules_len);
        bits.set_bit(count);
        let chunk = chunk_graph.add_chunk(Chunk::new(
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
          module.is_included(),
          input_base.clone(),
        ));
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
      self.split_chunks(
        &mut index_splitting_info,
        &mut chunk_graph,
        &mut bits_to_chunk,
        &input_base,
      );
    }

    // Sort modules in each chunk by execution order
    chunk_graph.chunk_table.iter_mut().for_each(|chunk| {
      chunk.modules.sort_unstable_by_key(|module_id| {
        self.link_output.module_table.modules[*module_id].exec_order()
      });
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
          ) => self.link_output.module_table.modules[*a_module_id]
            .exec_order()
            .cmp(&self.link_output.module_table.modules[*b_module_id].exec_order()),
          (ChunkKind::EntryPoint { module: a_module_id, .. }, ChunkKind::Common) => {
            let a_module_exec_order =
              self.link_output.module_table.modules[*a_module_id].exec_order();
            let b_chunk_first_module_exec_order =
              self.link_output.module_table.modules[b.modules[0]].exec_order();
            if a_module_exec_order == b_chunk_first_module_exec_order {
              a_should_be_first
            } else {
              a_module_exec_order.cmp(&b_chunk_first_module_exec_order)
            }
          }
          (ChunkKind::Common, ChunkKind::EntryPoint { module: b_module_id, .. }) => {
            let b_module_exec_order =
              self.link_output.module_table.modules[*b_module_id].exec_order();
            let a_chunk_first_module_exec_order =
              self.link_output.module_table.modules[a.modules[0]].exec_order();
            if a_chunk_first_module_exec_order == b_module_exec_order {
              b_should_be_first
            } else {
              a_chunk_first_module_exec_order.cmp(&b_module_exec_order)
            }
          }
          (ChunkKind::Common, ChunkKind::Common) => {
            let a_chunk_first_module_exec_order =
              self.link_output.module_table.modules[a.modules[0]].exec_order();
            let b_chunk_first_module_exec_order =
              self.link_output.module_table.modules[b.modules[0]].exec_order();
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

    chunk_graph
  }

  pub fn get_common_dir_of_all_modules(&self, modules: &[Module]) -> Option<String> {
    let mut ret: Option<String> = None;
    let iter = modules.iter().filter_map(|m| {
      m.as_normal().and_then(|item| {
        if !item.is_included() {
          return None;
        }
        if self.options.preserve_modules || item.is_user_defined_entry {
          Path::new(m.id()).is_absolute().then_some(m.id())
        } else {
          None
        }
      })
    });
    for id in iter {
      if let Some(ref mut ret_id) = ret {
        *ret_id = commondir::extract_longest_common_path(ret_id.as_str(), id);
      } else {
        ret = Some(id.to_string());
      }
    }
    ret
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
      let Module::Normal(module) = &self.link_output.module_table.modules[entry_point.id] else {
        continue;
      };
      let chunk = chunk_graph.add_chunk(Chunk::new(
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
        self.link_output.lived_entry_points.contains(&entry_point.id),
        input_base.clone(),
      ));
      bits_to_chunk.insert(bits, chunk);
      entry_module_to_entry_chunk.insert(entry_point.id, chunk);
    }
  }
  fn split_chunks(
    &self,
    index_splitting_info: &mut IndexSplittingInfo,
    chunk_graph: &mut ChunkGraph,
    bits_to_chunk: &mut FxHashMap<BitSet, ChunkIdx>,
    input_base: &ArcStr,
  ) {
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

    self.apply_advanced_chunks(
      index_splitting_info,
      &mut module_to_assigned,
      chunk_graph,
      input_base,
    );

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
      } else {
        let chunk = Chunk::new(
          None,
          None,
          None,
          bits.clone(),
          vec![],
          ChunkKind::Common,
          true,
          input_base.clone(),
        );
        let chunk_id = chunk_graph.add_chunk(chunk);
        chunk_graph.add_module_to_chunk(normal_module.idx, chunk_id);
        bits_to_chunk.insert(bits.clone(), chunk_id);
      }
    }
  }

  fn determine_reachable_modules_for_entry(
    &self,
    module_id: ModuleIdx,
    entry_index: u32,
    index_splitting_info: &mut IndexSplittingInfo,
  ) {
    let Module::Normal(module) = &self.link_output.module_table.modules[module_id] else {
      return;
    };
    let meta = &self.link_output.metas[module_id];

    if !module.meta.is_included() {
      return;
    }

    if index_splitting_info[module_id].bits.has_bit(entry_index) {
      return;
    }

    index_splitting_info[module_id].bits.set_bit(entry_index);
    index_splitting_info[module_id].share_count += 1;

    meta.dependencies.iter().copied().for_each(|dep_idx| {
      self.determine_reachable_modules_for_entry(dep_idx, entry_index, index_splitting_info);
    });
  }
}
