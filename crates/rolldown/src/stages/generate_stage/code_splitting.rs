use std::cmp::{Ordering, Reverse};

use crate::{chunk_graph::ChunkGraph, types::linking_metadata::LinkingMetadataVec};
use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{Chunk, ChunkIdx, ChunkKind, Module, ModuleIdx, ModuleTable, OutputFormat};
use rolldown_utils::{rustc_hash::FxHashMapExt, BitSet};
use rustc_hash::{FxHashMap, FxHashSet};

use super::GenerateStage;

#[derive(Clone)]
pub struct SplittingInfo {
  bits: BitSet,
  share_count: u32,
}

pub type IndexSplittingInfo = IndexVec<ModuleIdx, SplittingInfo>;

impl<'a> GenerateStage<'a> {
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
        bits.clone(),
        vec![],
        ChunkKind::EntryPoint {
          is_user_defined: module.is_user_defined_entry,
          bit: count,
          module: entry_point.id,
        },
      ));
      bits_to_chunk.insert(bits, chunk);
      entry_module_to_entry_chunk.insert(entry_point.id, chunk);
    }

    // Determine which modules belong to which chunk. A module could belong to multiple chunks.
    self.link_output.entries.iter().enumerate().for_each(|(i, entry_point)| {
      self.determine_reachable_modules_for_entry(
        entry_point.id,
        i.try_into().expect("Too many entries, u32 overflowed."),
        &mut index_splitting_info,
      );
    });

    let mut module_to_assigned: IndexVec<ModuleIdx, bool> =
      oxc_index::index_vec![false; self.link_output.module_table.modules.len()];

    self.apply_advanced_chunks(&index_splitting_info, &mut module_to_assigned, &mut chunk_graph);

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
        "Empty bits means the module is not reachable, so it should bail out with `is_included: false` {:?}", normal_module.stable_id
      );

      if let Some(chunk_id) = bits_to_chunk.get(bits).copied() {
        chunk_graph.add_module_to_chunk(normal_module.idx, chunk_id);
      } else {
        let chunk = Chunk::new(None, bits.clone(), vec![], ChunkKind::Common);
        let chunk_id = chunk_graph.add_chunk(chunk);
        chunk_graph.add_module_to_chunk(normal_module.idx, chunk_id);
        bits_to_chunk.insert(bits.clone(), chunk_id);
      }
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
    let sorted_chunk_idx_vec = chunk_graph
      .chunk_table
      .iter_enumerated()
      .sorted_unstable_by(|(index_a, a), (index_b, b)| {
        let a_should_be_first = Ordering::Less;
        let b_should_be_first = Ordering::Greater;

        match (&a.kind, &b.kind) {
          (ChunkKind::EntryPoint { is_user_defined, .. }, ChunkKind::Common) => {
            if *is_user_defined {
              a_should_be_first
            } else {
              b_should_be_first
            }
          }
          (ChunkKind::Common, ChunkKind::EntryPoint { is_user_defined, .. }) => {
            if *is_user_defined {
              b_should_be_first
            } else {
              a_should_be_first
            }
          }
          (
            ChunkKind::EntryPoint { is_user_defined: a_is_user_defined, .. },
            ChunkKind::EntryPoint { is_user_defined: b_is_user_defined, .. },
          ) => {
            if *a_is_user_defined && *b_is_user_defined {
              // Using user specific order of entry
              index_a.cmp(index_b)
            } else {
              a.exec_order.cmp(&b.exec_order)
            }
          }
          _ => a.exec_order.cmp(&b.exec_order),
        }
      })
      .map(|(idx, _)| idx)
      .collect::<Vec<_>>();

    chunk_graph.sorted_chunk_idx_vec = sorted_chunk_idx_vec;
    chunk_graph.entry_module_to_entry_chunk = entry_module_to_entry_chunk;

    chunk_graph
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

  #[allow(clippy::too_many_lines)] // TODO(hyf0): refactor
  fn apply_advanced_chunks(
    &mut self,
    index_splitting_info: &IndexSplittingInfo,
    module_to_assigned: &mut IndexVec<ModuleIdx, bool>,
    chunk_graph: &mut ChunkGraph,
  ) {
    fn add_module_and_dependencies_to_group_recursively(
      module_group: &mut ModuleGroup,
      module_idx: ModuleIdx,
      module_metas: &LinkingMetadataVec,
      module_table: &ModuleTable,
      visited: &mut FxHashSet<ModuleIdx>,
    ) {
      let is_visited = !visited.insert(module_idx);

      if is_visited {
        return;
      }

      let Module::Normal(module) = &module_table.modules[module_idx] else {
        return;
      };

      if !module.ecma_view.meta.is_included() {
        return;
      }

      visited.insert(module_idx);

      module_group.add_module(module_idx, module_table);

      for dep in &module_metas[module_idx].dependencies {
        add_module_and_dependencies_to_group_recursively(
          module_group,
          *dep,
          module_metas,
          module_table,
          visited,
        );
      }
    }
    // `ModuleGroup` is a temporary representation of `Chunk`. A valid `ModuleGroup` would be converted to a `Chunk` in the end.
    struct ModuleGroup {
      name: ArcStr,
      match_group_index: usize,
      modules: FxHashSet<ModuleIdx>,
      priority: u32,
      sizes: f64,
    }

    oxc_index::define_index_type! {
      pub struct ModuleGroupIdx = u32;
    }

    impl ModuleGroup {
      #[allow(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here
      pub fn add_module(&mut self, module_idx: ModuleIdx, module_table: &ModuleTable) {
        if self.modules.insert(module_idx) {
          self.sizes += module_table.modules[module_idx].size() as f64;
        }
      }

      #[allow(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here
      pub fn remove_module(&mut self, module_idx: ModuleIdx, module_table: &ModuleTable) {
        if self.modules.remove(&module_idx) {
          self.sizes -= module_table.modules[module_idx].size() as f64;
          self.sizes = f64::max(self.sizes, 0.0);
        }
      }
    }

    let Some(chunking_options) = &self.options.advanced_chunks else {
      return;
    };

    let Some(match_groups) =
      chunking_options.groups.as_ref().map(|inner| inner.iter().collect::<Vec<_>>())
    else {
      return;
    };

    if match_groups.is_empty() {
      return;
    }

    let mut index_module_groups: IndexVec<ModuleGroupIdx, ModuleGroup> = IndexVec::new();
    let mut name_to_module_group: FxHashMap<ArcStr, ModuleGroupIdx> = FxHashMap::default();

    for normal_module in self.link_output.module_table.modules.iter().filter_map(Module::as_normal)
    {
      if !normal_module.meta.is_included() {
        continue;
      }

      if module_to_assigned[normal_module.idx] {
        continue;
      }

      let splitting_info = &index_splitting_info[normal_module.idx];

      for (match_group_index, match_group) in match_groups.iter().copied().enumerate() {
        let is_matched =
          match_group.test.as_ref().map_or(true, |test| test.matches(&normal_module.id));

        if !is_matched {
          continue;
        }

        if let Some(allow_min_share_count) =
          match_group.min_share_count.map_or(chunking_options.min_share_count, Some)
        {
          if splitting_info.share_count < allow_min_share_count {
            continue;
          }
        }

        let group_name = ArcStr::from(&match_group.name);

        let module_group_idx =
          name_to_module_group.entry(group_name.clone()).or_insert_with(|| {
            index_module_groups.push(ModuleGroup {
              modules: FxHashSet::default(),
              match_group_index,
              priority: match_group.priority.unwrap_or(0),
              name: group_name.clone(),
              sizes: 0.0,
            })
          });

        add_module_and_dependencies_to_group_recursively(
          &mut index_module_groups[*module_group_idx],
          normal_module.idx,
          &self.link_output.metas,
          &self.link_output.module_table,
          &mut FxHashSet::default(),
        );
      }
    }

    let mut module_groups = index_module_groups.raw;
    module_groups.sort_unstable_by_key(|item| item.match_group_index);
    module_groups.sort_by_key(|item| Reverse(item.priority));
    module_groups.reverse();
    // These two sort ensure higher priority group goes first. If two groups have the same priority, the one with the lower index goes first.

    while let Some(this_module_group) = module_groups.pop() {
      if this_module_group.modules.is_empty() {
        continue;
      }

      if let Some(allow_min_size) = match_groups[this_module_group.match_group_index]
        .min_size
        .map_or(chunking_options.min_size, Some)
      {
        if this_module_group.sizes < allow_min_size {
          continue;
        }
      }

      let chunk = Chunk::new(
        Some(this_module_group.name.clone()),
        index_splitting_info
          [this_module_group.modules.iter().next().copied().expect("must have one")]
        .bits
        .clone(),
        vec![],
        ChunkKind::Common,
      );

      let chunk_idx = chunk_graph.add_chunk(chunk);

      this_module_group.modules.iter().copied().for_each(|module_idx| {
        module_groups.iter_mut().for_each(|group| {
          group.remove_module(module_idx, &self.link_output.module_table);
        });
        chunk_graph.chunk_table[chunk_idx].bits.union(&index_splitting_info[module_idx].bits);
        chunk_graph.add_module_to_chunk(module_idx, chunk_idx);
        module_to_assigned[module_idx] = true;
      });
    }
  }
}
