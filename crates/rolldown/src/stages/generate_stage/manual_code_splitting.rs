use std::{
  cmp::{Ordering, Reverse},
  collections::BinaryHeap,
  path::Path,
  sync::Arc,
};

use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::{
  Chunk, ChunkKind, ChunkingContext, ManualCodeSplittingOptions, MatchGroup, MatchGroupTest,
  Module, ModuleIdx, ModuleTable,
};
use rolldown_error::BuildResult;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::{BitSet, xxhash::xxhash_with_base};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions, chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput,
  types::linking_metadata::LinkingMetadataVec,
};

use super::{
  GenerateStage,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  code_splitting::IndexSplittingInfo,
};

// `ModuleGroup` is a temporary representation of `Chunk`. A valid `ModuleGroup` would be converted to a `Chunk` in the end.
#[derive(Debug)]
struct ModuleGroup {
  name: ArcStr,
  match_group_index: usize,
  modules: FxHashSet<ModuleIdx>,
  priority: u32,
  sizes: f64,
  entries_aware_bits: Option<BitSet>,
}

/// Used to track what created the module group
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ModuleGroupOrigin {
  match_group_index: usize,
  name: ArcStr,
}

/// Unique for each module group
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ModuleGroupId(ModuleGroupOrigin, Option<BitSet>);

impl ModuleGroup {
  #[expect(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here
  pub fn add_module(&mut self, module_idx: ModuleIdx, module_table: &ModuleTable) {
    if self.modules.insert(module_idx) {
      self.sizes += module_table[module_idx].size() as f64;
    }
  }

  #[expect(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here
  pub fn remove_module(&mut self, module_idx: ModuleIdx, module_table: &ModuleTable) {
    if self.modules.remove(&module_idx) {
      self.sizes -= module_table[module_idx].size() as f64;
      self.sizes = f64::max(self.sizes, 0.0);
    }
  }
}

struct ManualSplitter<'a> {
  link_output: &'a LinkStageOutput,
  index_splitting_info: &'a IndexSplittingInfo,
  options: &'a SharedOptions,
  chunking_options: &'a ManualCodeSplittingOptions,
  match_groups: Vec<&'a MatchGroup>,
  plugin_driver: &'a SharedPluginDriver,
  input_base: &'a ArcStr,
  chunk_graph: &'a mut ChunkGraph,
  module_to_assigned: &'a mut IndexVec<ModuleIdx, bool>,
}

impl ManualSplitter<'_> {
  async fn split(&mut self) -> BuildResult<()> {
    let (mut module_groups, entries_aware_groups_by_origin) = self.build_module_groups().await?;

    if module_groups.values().all(|group| group.modules.is_empty()) {
      return Ok(());
    }

    self.extract_runtime_chunk(&mut module_groups);

    if !entries_aware_groups_by_origin.is_empty() {
      merge_entries_aware_subgroups(
        &mut module_groups,
        &entries_aware_groups_by_origin,
        &self.match_groups,
        &self.link_output.module_table,
      );
    }

    let module_groups = self.into_priority_sorted_groups(module_groups);
    if module_groups.is_empty() {
      return Ok(());
    }

    self.convert_groups_to_chunks(module_groups);
    Ok(())
  }

  async fn build_module_groups(
    &self,
  ) -> BuildResult<(FxHashMap<u32, ModuleGroup>, FxHashMap<ModuleGroupOrigin, Vec<u32>>)> {
    let metas = &self.link_output.metas;
    let mut module_groups: FxHashMap<u32, ModuleGroup> = FxHashMap::default();
    let mut group_key_by_id: FxHashMap<ModuleGroupId, u32> = FxHashMap::default();
    let mut entries_aware_groups_by_origin: FxHashMap<ModuleGroupOrigin, Vec<u32>> =
      FxHashMap::default();
    let mut next_group_key: u32 = 0;

    for normal_module in self.link_output.module_table.modules.iter().filter_map(Module::as_normal)
    {
      if !metas[normal_module.idx].is_included {
        continue;
      }

      if self.module_to_assigned[normal_module.idx] {
        continue;
      }

      let splitting_info = &self.index_splitting_info[normal_module.idx];

      for (match_group_index, match_group) in self.match_groups.iter().copied().enumerate() {
        let is_matched = match &match_group.test {
          None => true,
          Some(MatchGroupTest::Regex(reg)) => reg.matches(&normal_module.id),
          Some(MatchGroupTest::Function(func)) => {
            func(&normal_module.id).await?.unwrap_or_default()
          }
        };

        if !is_matched {
          continue;
        }

        let allow_min_module_size =
          match_group.min_module_size.map_or(self.chunking_options.min_module_size, Some);
        let allow_max_module_size =
          match_group.max_module_size.map_or(self.chunking_options.max_module_size, Some);

        let is_min_module_size_satisfied = allow_min_module_size
          .is_none_or(|min_module_size| normal_module.size() >= min_module_size);
        let is_max_module_size_satisfied = allow_max_module_size
          .is_none_or(|max_module_size| normal_module.size() <= max_module_size);

        if !is_min_module_size_satisfied || !is_max_module_size_satisfied {
          continue;
        }

        if let Some(allow_min_share_count) =
          match_group.min_share_count.map_or(self.chunking_options.min_share_count, Some)
        {
          if splitting_info.share_count < allow_min_share_count {
            continue;
          }
        }

        let ctx = ChunkingContext::new(Arc::clone(&self.plugin_driver.module_infos));

        let Some(group_name) = match_group.name.value(&ctx, &normal_module.id).await? else {
          // Group which doesn't have a name will be ignored.
          continue;
        };
        let group_name = ArcStr::from(group_name);

        let entries_aware = match_group.entries_aware.unwrap_or(false);
        let entries_aware_merge_threshold =
          match_group.entries_aware_merge_threshold.unwrap_or(0.0);
        let entries_aware_bits =
          if entries_aware { Some(splitting_info.bits.clone()) } else { None };
        let module_group_origin = ModuleGroupOrigin { match_group_index, name: group_name.clone() };
        let module_group_id =
          ModuleGroupId(module_group_origin.clone(), entries_aware_bits.clone());
        let module_group_key = match group_key_by_id.entry(module_group_id) {
          std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
          std::collections::hash_map::Entry::Vacant(vacant) => {
            let module_group_key = next_group_key;
            next_group_key = next_group_key.checked_add(1).expect("too many module groups");
            module_groups.insert(
              module_group_key,
              ModuleGroup {
                modules: FxHashSet::default(),
                match_group_index: module_group_origin.match_group_index,
                priority: match_group.priority.unwrap_or(0),
                name: module_group_origin.name.clone(),
                sizes: 0.0,
                entries_aware_bits: entries_aware_bits.clone(),
              },
            );
            if entries_aware && entries_aware_merge_threshold > 0.0 {
              entries_aware_groups_by_origin
                .entry(module_group_origin.clone())
                .or_default()
                .push(module_group_key);
            }
            *vacant.insert(module_group_key)
          }
        };

        let include_dependencies_recursively =
          self.chunking_options.include_dependencies_recursively.unwrap_or(true);

        add_module_and_dependencies_to_group_recursively(
          module_groups.get_mut(&module_group_key).expect("group key should exist"),
          normal_module.idx,
          &self.link_output.metas,
          &self.link_output.module_table,
          &mut FxHashSet::default(),
          include_dependencies_recursively,
        );
      }
    }

    Ok((module_groups, entries_aware_groups_by_origin))
  }

  fn extract_runtime_chunk(&mut self, module_groups: &mut FxHashMap<u32, ModuleGroup>) {
    // Manually pull out the runtime module into a standalone chunk.
    let metas = &self.link_output.metas;
    let runtime_module_idx = self.link_output.runtime.id();
    assert!(
      matches!(&self.link_output.module_table[runtime_module_idx], Module::Normal(_)),
      "rolldown runtime is always a normal module"
    );

    if metas[runtime_module_idx].is_included {
      let runtime_chunk = Chunk::new(
        Some("rolldown-runtime".into()),
        None,
        self.index_splitting_info[runtime_module_idx].bits.clone(),
        vec![],
        ChunkKind::Common,
        self.input_base.clone(),
        None,
      );
      let chunk_idx = self.chunk_graph.add_chunk(runtime_chunk);
      module_groups.values_mut().for_each(|group| {
        group.remove_module(runtime_module_idx, &self.link_output.module_table);
      });
      self.chunk_graph.chunk_table[chunk_idx]
        .bits
        .union(&self.index_splitting_info[runtime_module_idx].bits);
      self.chunk_graph.add_module_to_chunk(
        runtime_module_idx,
        chunk_idx,
        self.link_output.metas[runtime_module_idx].depended_runtime_helper,
      );
      self.module_to_assigned[runtime_module_idx] = true;
    }
  }

  fn into_priority_sorted_groups(
    &self,
    module_groups: FxHashMap<u32, ModuleGroup>,
  ) -> Vec<ModuleGroup> {
    let mut module_groups =
      module_groups.into_values().filter(|group| !group.modules.is_empty()).collect::<Vec<_>>();
    if module_groups.is_empty() {
      return module_groups;
    }

    // - Higher priority group goes first.
    // - If two groups have the same priority, the one with the lower index goes first.
    // - If two groups have the same priority and index, we use dictionary order to sort them.
    // Outer `Reverse` is due to we're gonna use `pop` consume the vector.
    module_groups.sort_by_cached_key(|item| {
      Reverse((Reverse(item.priority), item.match_group_index, item.name.clone()))
    });

    module_groups
  }

  fn convert_groups_to_chunks(&mut self, mut module_groups: Vec<ModuleGroup>) {
    while let Some(group) = module_groups.pop() {
      if group.modules.is_empty() {
        continue;
      }

      let allow_min_size = self.match_groups[group.match_group_index]
        .min_size
        .map_or(self.chunking_options.min_size, Some)
        .unwrap_or(0.0);

      if group.sizes < allow_min_size {
        continue;
      }

      if let Some(allow_max_size) = self.match_groups[group.match_group_index]
        .max_size
        .map_or(self.chunking_options.max_size, Some)
      {
        if group.sizes > allow_max_size {
          if let Some((left, right)) =
            self.try_split_oversized_group(&group, allow_min_size, allow_max_size)
          {
            module_groups.push(left);
            module_groups.push(right);
            continue;
          }
        }
      }

      self.emit_chunk_from_group(&group, &mut module_groups);
    }
  }

  fn try_split_oversized_group(
    &self,
    group: &ModuleGroup,
    min_size: f64,
    max_size: f64,
  ) -> Option<(ModuleGroup, ModuleGroup)> {
    let mut modules = group.modules.iter().copied().collect::<Vec<_>>();
    // Split by lexical relevance first (stable module id), then by size constraints.
    modules.sort_by(|lhs, rhs| {
      let lhs_module = &self.link_output.module_table[*lhs];
      let rhs_module = &self.link_output.module_table[*rhs];
      lhs_module
        .stable_id()
        .cmp(rhs_module.stable_id())
        .then(lhs_module.exec_order().cmp(&rhs_module.exec_order()))
    });

    let (split_index, left_size, right_size) =
      find_relevance_split_index(&modules, &self.link_output.module_table, min_size, max_size)?;

    Some((
      ModuleGroup {
        name: group.name.clone(),
        match_group_index: group.match_group_index,
        modules: modules[..split_index].iter().copied().collect(),
        priority: group.priority,
        sizes: left_size,
        entries_aware_bits: group.entries_aware_bits.clone(),
      },
      ModuleGroup {
        name: group.name.clone(),
        match_group_index: group.match_group_index,
        modules: modules[split_index..].iter().copied().collect(),
        priority: group.priority,
        sizes: right_size,
        entries_aware_bits: group.entries_aware_bits.clone(),
      },
    ))
  }

  fn emit_chunk_from_group(&mut self, group: &ModuleGroup, remaining_groups: &mut [ModuleGroup]) {
    let first_module_bits =
      &self.index_splitting_info[group.modules.iter().next().copied().expect("must have one")].bits;

    let entries_aware = self.match_groups[group.match_group_index].entries_aware.unwrap_or(false);
    let chunk_bits = if entries_aware {
      group.entries_aware_bits.as_ref().unwrap_or(first_module_bits)
    } else {
      first_module_bits
    };

    let chunk_name = if entries_aware {
      derive_entries_aware_chunk_name(&group.name, chunk_bits, self.link_output)
    } else {
      group.name.clone()
    };

    let mut chunk = Chunk::new(
      Some(chunk_name),
      None,
      chunk_bits.clone(),
      vec![],
      ChunkKind::Common,
      self.input_base.clone(),
      None,
    );
    chunk.add_creation_reason(
      ChunkCreationReason::ManualCodeSplittingGroup {
        name: &group.name,
        group_index: group.match_group_index.try_into().unwrap(),
        bits: if entries_aware { Some(chunk_bits) } else { None },
        link_output: self.link_output,
      },
      self.options,
    );

    let chunk_idx = self.chunk_graph.add_chunk(chunk);

    group.modules.iter().copied().for_each(|module_idx| {
      remaining_groups.iter_mut().for_each(|remaining| {
        remaining.remove_module(module_idx, &self.link_output.module_table);
      });
      self.chunk_graph.chunk_table[chunk_idx]
        .bits
        .union(&self.index_splitting_info[module_idx].bits);
      self.chunk_graph.add_module_to_chunk(
        module_idx,
        chunk_idx,
        self.link_output.metas[module_idx].depended_runtime_helper,
      );
      self.module_to_assigned[module_idx] = true;
    });
  }
}

impl GenerateStage<'_> {
  pub async fn apply_manual_code_splitting(
    &self,
    index_splitting_info: &IndexSplittingInfo,
    module_to_assigned: &mut IndexVec<ModuleIdx, bool>,
    chunk_graph: &mut ChunkGraph,
    input_base: &ArcStr,
  ) -> BuildResult<()> {
    let Some(chunking_options) = &self.options.manual_code_splitting else {
      return Ok(());
    };

    let Some(match_groups) =
      chunking_options.groups.as_ref().map(|inner| inner.iter().collect::<Vec<_>>())
    else {
      return Ok(());
    };

    if match_groups.is_empty() {
      return Ok(());
    }

    let mut splitter = ManualSplitter {
      link_output: self.link_output,
      index_splitting_info,
      options: self.options,
      chunking_options,
      match_groups,
      plugin_driver: self.plugin_driver,
      input_base,
      chunk_graph,
      module_to_assigned,
    };
    splitter.split().await
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedSize(f64);

impl Eq for OrderedSize {}

impl PartialOrd for OrderedSize {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for OrderedSize {
  fn cmp(&self, other: &Self) -> Ordering {
    self.0.total_cmp(&other.0)
  }
}

fn merge_entries_aware_subgroups(
  module_groups: &mut FxHashMap<u32, ModuleGroup>,
  entries_aware_groups_by_origin: &FxHashMap<ModuleGroupOrigin, Vec<u32>>,
  match_groups: &[&MatchGroup],
  module_table: &ModuleTable,
) {
  let mut version_by_key: FxHashMap<u32, u32> = FxHashMap::default();

  for (origin, group_keys) in entries_aware_groups_by_origin {
    let threshold =
      match_groups[origin.match_group_index].entries_aware_merge_threshold.unwrap_or(0.0);
    if threshold <= 0.0 {
      continue;
    }

    let mut unqualified_heap: BinaryHeap<Reverse<(OrderedSize, u32, u32)>> = BinaryHeap::new();

    for &group_key in group_keys {
      let Some(group) = module_groups.get(&group_key) else {
        continue;
      };
      if is_below_merge_threshold(group.sizes, threshold) && !group.modules.is_empty() {
        unqualified_heap.push(Reverse((
          OrderedSize(group.sizes),
          group_key,
          current_version(&version_by_key, group_key),
        )));
      }
    }

    while let Some(Reverse((_size, candidate_key, candidate_version))) = unqualified_heap.pop() {
      let Some(candidate_group) = module_groups.get(&candidate_key) else {
        continue;
      };
      if candidate_group.modules.is_empty()
        || candidate_version != current_version(&version_by_key, candidate_key)
        || !is_below_merge_threshold(candidate_group.sizes, threshold)
      {
        continue;
      }

      let Some(candidate_bits) = candidate_group.entries_aware_bits.as_ref() else {
        continue;
      };

      let mut best_target = None;
      for &target_key in group_keys {
        if target_key == candidate_key {
          continue;
        }

        let Some(target_group) = module_groups.get(&target_key) else {
          continue;
        };
        if target_group.modules.is_empty() {
          continue;
        }

        let Some(target_bits) = target_group.entries_aware_bits.as_ref() else {
          continue;
        };

        let score = (
          symmetric_difference_count(candidate_bits, target_bits),
          OrderedSize(target_group.sizes),
          target_key,
        );
        if best_target.is_none_or(|best| score < best) {
          best_target = Some(score);
        }
      }

      let Some((_extra_count, _target_size, target_key)) = best_target else {
        continue;
      };

      merge_module_groups(module_groups, candidate_key, target_key, module_table);
      bump_version(&mut version_by_key, candidate_key);
      bump_version(&mut version_by_key, target_key);

      let Some(target_group) = module_groups.get(&target_key) else {
        continue;
      };
      if is_below_merge_threshold(target_group.sizes, threshold) && !target_group.modules.is_empty()
      {
        unqualified_heap.push(Reverse((
          OrderedSize(target_group.sizes),
          target_key,
          current_version(&version_by_key, target_key),
        )));
      }
    }
  }
}

fn merge_module_groups(
  module_groups: &mut FxHashMap<u32, ModuleGroup>,
  from_key: u32,
  to_key: u32,
  module_table: &ModuleTable,
) {
  if from_key == to_key {
    return;
  }

  let Some(mut from_group) = module_groups.remove(&from_key) else {
    return;
  };
  let Some(to_group) = module_groups.get_mut(&to_key) else {
    module_groups.insert(from_key, from_group);
    return;
  };

  to_group.modules.extend(from_group.modules.drain());
  to_group.sizes = sum_group_sizes(&to_group.modules, module_table);
  if let Some(from_bits) = from_group.entries_aware_bits.take() {
    match &mut to_group.entries_aware_bits {
      Some(to_bits) => to_bits.union(&from_bits),
      None => to_group.entries_aware_bits = Some(from_bits),
    }
  }
}

fn symmetric_difference_count(lhs: &BitSet, rhs: &BitSet) -> usize {
  let lhs_extra = lhs.index_of_one().filter(|bit| !rhs.has_bit(*bit)).count();
  let rhs_extra = rhs.index_of_one().filter(|bit| !lhs.has_bit(*bit)).count();
  lhs_extra + rhs_extra
}

fn is_below_merge_threshold(size: f64, threshold: f64) -> bool {
  size > 0.0 && size < threshold
}

fn current_version(version_by_key: &FxHashMap<u32, u32>, key: u32) -> u32 {
  version_by_key.get(&key).copied().unwrap_or(0)
}

fn bump_version(version_by_key: &mut FxHashMap<u32, u32>, key: u32) {
  let next_version = current_version(version_by_key, key).wrapping_add(1);
  version_by_key.insert(key, next_version);
}

#[expect(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here.
fn sum_group_sizes(modules: &FxHashSet<ModuleIdx>, module_table: &ModuleTable) -> f64 {
  modules.iter().map(|module_idx| module_table[*module_idx].size() as f64).sum()
}

#[expect(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here.
fn module_size(module_idx: ModuleIdx, module_table: &ModuleTable) -> f64 {
  module_table[module_idx].size() as f64
}

/// Similarity differences within this threshold are treated as insignificant ties,
/// allowing size-based criteria to decide. Value of 10 equals one character position's
/// max score, absorbing digit-level ASCII noise while preserving directory boundary signals.
const SIMILARITY_SIGNIFICANCE_THRESHOLD: i32 = 10;

fn stable_id_similarity(lhs: &str, rhs: &str) -> i32 {
  lhs.as_bytes().iter().zip(rhs.as_bytes()).fold(0, |acc, (lhs_char, rhs_char)| {
    acc + (10 - (i32::from(*lhs_char) - i32::from(*rhs_char)).abs()).max(0)
  })
}

fn find_relevance_split_index(
  modules: &[ModuleIdx],
  module_table: &ModuleTable,
  min_size: f64,
  max_size: f64,
) -> Option<(usize, f64, f64)> {
  if modules.len() < 2 {
    return None;
  }

  let keys = modules
    .iter()
    .map(|module_idx| module_table[*module_idx].stable_id().as_str())
    .collect::<Vec<_>>();
  let sizes =
    modules.iter().map(|module_idx| module_size(*module_idx, module_table)).collect::<Vec<_>>();
  pick_relevance_split_index(&keys, &sizes, min_size, max_size)
}

fn pick_relevance_split_index(
  keys: &[&str],
  sizes: &[f64],
  min_size: f64,
  max_size: f64,
) -> Option<(usize, f64, f64)> {
  debug_assert_eq!(keys.len(), sizes.len());
  if keys.len() < 2 || sizes.len() < 2 {
    return None;
  }

  let mut prefix_sizes = Vec::with_capacity(sizes.len() + 1);
  prefix_sizes.push(0.0);
  for size in sizes {
    let next_size = prefix_sizes.last().copied().unwrap_or_default() + *size;
    prefix_sizes.push(next_size);
  }

  let total_size = prefix_sizes.last().copied().unwrap_or_default();

  // Find the leftmost split point that can satisfy min_size for the left group.
  let mut left_bound = 1;
  while left_bound < sizes.len() && prefix_sizes[left_bound] < min_size {
    left_bound += 1;
  }

  // Find the rightmost split point that can satisfy min_size for the right group.
  let mut right_bound = sizes.len() - 1;
  while right_bound > 0 && total_size - prefix_sizes[right_bound] < min_size {
    right_bound -= 1;
  }

  if left_bound > right_bound {
    return None;
  }

  let mut best_split_index = None;
  let mut best_similarity = i32::MAX;
  let mut best_oversized_side_count = usize::MAX;
  let mut best_max_side_size = f64::INFINITY;

  for split_index in left_bound..=right_bound {
    let left_size = prefix_sizes[split_index];
    let right_size = total_size - left_size;
    if left_size < min_size || right_size < min_size {
      continue;
    }

    let similarity = stable_id_similarity(keys[split_index - 1], keys[split_index]);
    let oversized_side_count =
      usize::from(left_size > max_size) + usize::from(right_size > max_size);
    let max_side_size = left_size.max(right_size);

    let is_better = if (best_similarity - similarity).abs() > SIMILARITY_SIGNIFICANCE_THRESHOLD {
      similarity < best_similarity
    } else if oversized_side_count != best_oversized_side_count {
      oversized_side_count < best_oversized_side_count
    } else {
      max_side_size < best_max_side_size
    };

    if is_better {
      best_split_index = Some(split_index);
      best_similarity = similarity;
      best_oversized_side_count = oversized_side_count;
      best_max_side_size = max_side_size;
    }
  }

  let split_index = best_split_index?;
  let left_size = prefix_sizes[split_index];
  let right_size = total_size - left_size;
  Some((split_index, left_size, right_size))
}

fn derive_entries_aware_chunk_name(
  group_name: &str,
  bits: &BitSet,
  link_output: &crate::stages::link_stage::LinkStageOutput,
) -> ArcStr {
  const MAX_CHUNK_NAME_LEN: usize = 100;
  const HASH_DISPLAY_LEN: usize = 8;
  const TRUNCATED_LEN: usize = MAX_CHUNK_NAME_LEN - HASH_DISPLAY_LEN - 1; // 1 for the `~` separator

  let entry_names: Vec<String> = link_output
    .entries
    .iter()
    .flat_map(|(_idx, entries)| entries.iter())
    .enumerate()
    .filter_map(|(index, entry_point)| {
      if bits.has_bit(index.try_into().unwrap()) {
        Some(entry_point.name.as_ref().map(ArcStr::to_string).unwrap_or_else(|| {
          // Fall back to file stem of the entry module's stable_id
          let module = &link_output.module_table[entry_point.idx];
          Path::new(module.stable_id().as_str())
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| module.stable_id().to_string())
        }))
      } else {
        None
      }
    })
    .collect();

  let full_name = if entry_names.is_empty() {
    group_name.to_string()
  } else {
    format!("{}~{}", group_name, entry_names.join("~"))
  };

  if full_name.len() > MAX_CHUNK_NAME_LEN {
    let hash = xxhash_with_base(full_name.as_bytes(), 36);
    let mut truncate_at = TRUNCATED_LEN;
    while !full_name.is_char_boundary(truncate_at) {
      truncate_at -= 1;
    }
    let truncated = &full_name[..truncate_at];
    ArcStr::from(format!("{truncated}~{}", &hash[..HASH_DISPLAY_LEN]))
  } else {
    ArcStr::from(full_name)
  }
}

fn add_module_and_dependencies_to_group_recursively(
  module_group: &mut ModuleGroup,
  module_idx: ModuleIdx,
  module_metas: &LinkingMetadataVec,
  module_table: &ModuleTable,
  visited: &mut FxHashSet<ModuleIdx>,
  recursively: bool,
) {
  let is_visited = !visited.insert(module_idx);

  if is_visited {
    return;
  }

  if !module_table[module_idx].is_normal() {
    return;
  }

  if !module_metas[module_idx].is_included {
    return;
  }

  visited.insert(module_idx);

  module_group.add_module(module_idx, module_table);
  if recursively {
    for dep in &module_metas[module_idx].dependencies {
      add_module_and_dependencies_to_group_recursively(
        module_group,
        *dep,
        module_metas,
        module_table,
        visited,
        recursively,
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{pick_relevance_split_index, stable_id_similarity};

  struct SplitCase<'a> {
    keys: &'a [&'a str],
    sizes: &'a [f64],
    min_size: f64,
    max_size: f64,
    /// Expected (left_group, right_group) after splitting, or None if no valid split.
    expected: Option<(&'a [&'a str], &'a [&'a str])>,
  }

  fn assert_split(case: &SplitCase) {
    let similarities: Vec<i32> =
      case.keys.windows(2).map(|w| stable_id_similarity(w[0], w[1])).collect();
    let result = pick_relevance_split_index(case.keys, case.sizes, case.min_size, case.max_size);
    match (result, case.expected) {
      (Some((split, _, _)), Some(expected)) => {
        let (left, right) = (&case.keys[..split], &case.keys[split..]);
        assert_eq!((left, right), expected, "similarities: {similarities:?}",);
      }
      (None, None) => {}
      (Some((split, _, _)), None) => {
        panic!(
          "expected no split, but got split at {split}: ({:?}, {:?})\nsimilarities: {similarities:?}",
          &case.keys[..split],
          &case.keys[split..],
        );
      }
      (None, Some(expected)) => {
        panic!(
          "expected split ({:?}, {:?}), but got None\nsimilarities: {similarities:?}",
          expected.0, expected.1,
        );
      }
    }
  }

  #[test]
  fn similarity_prefers_low_stable_id_boundary() {
    assert_split(&SplitCase {
      keys: &[
        "src/components/button.js",
        "src/components/modal.js",
        "node_modules/react/index.js",
        "node_modules/react-dom/index.js",
      ],
      sizes: &[10.0, 10.0, 10.0, 10.0],
      min_size: 10.0,
      max_size: 100.0,
      expected: Some((
        &["src/components/button.js", "src/components/modal.js"],
        &["node_modules/react/index.js", "node_modules/react-dom/index.js"],
      )),
    });
  }

  #[test]
  fn threshold_prefers_size_over_insignificant_similarity_difference() {
    // Digit noise: gap=3 (<10), tie → size picks smaller max side.
    assert_split(&SplitCase {
      keys: &["size-15.js", "size-20.js", "size-41.js"],
      sizes: &[15.0, 20.0, 41.0],
      min_size: 0.0,
      max_size: 40.0,
      expected: Some((&["size-15.js", "size-20.js"], &["size-41.js"])),
    });
    // Lowest similarity at index 1, but gap=1 (<10) → size wins.
    assert_split(&SplitCase {
      keys: &["ab0.js", "aa9.js", "aa0.js"],
      sizes: &[10.0, 10.0, 30.0],
      min_size: 0.0,
      max_size: 25.0,
      expected: Some((&["ab0.js", "aa9.js"], &["aa0.js"])),
    });
  }

  #[test]
  fn significant_similarity_gap_still_wins_over_size() {
    // Directory boundary: gap=17 (>10), similarity wins despite worse size.
    assert_split(&SplitCase {
      keys: &["src/a.js", "lib/b.js", "lib/c.js"],
      sizes: &[10.0, 10.0, 30.0],
      min_size: 0.0,
      max_size: 25.0,
      expected: Some((&["src/a.js"], &["lib/b.js", "lib/c.js"])),
    });
  }

  #[test]
  fn min_size_prevents_split() {
    // Clear directory boundary exists, but any split would create a side < min_size.
    assert_split(&SplitCase {
      keys: &["src/a.js", "lib/b.js", "lib/c.js"],
      sizes: &[3.0, 3.0, 3.0],
      min_size: 5.0,
      max_size: 10.0,
      expected: None,
    });
  }
}
