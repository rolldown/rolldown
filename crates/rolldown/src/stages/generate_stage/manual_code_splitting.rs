use std::{
  cmp::{Ordering, Reverse},
  collections::BinaryHeap,
  path::Path,
  sync::Arc,
};

use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  Chunk, ChunkKind, ChunkingContext, EntryPoint, ManualCodeSplittingOptions, MatchGroup,
  MatchGroupTest, Module, ModuleIdx, ModuleTable, ModuleTagBitSet, ModuleTagRegistry,
};
use rolldown_error::BuildResult;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::{BitSet, IndexBitSet, xxhash::xxhash_with_base};
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

/// Unique identity for each module group, used for deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ModuleGroupId {
  match_group_index: usize,
  name: ArcStr,
}

/// Lightweight representation used during entries_aware subgroup merge.
/// Contains only the fields needed for bitset-based merge operations.
struct EntriesAwareSubgroup {
  bits: BitSet,
  modules: FxHashSet<ModuleIdx>,
  sizes: f64,
}

oxc_index::define_index_type! {
  struct ModuleGroupIdx = u32;
}

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
  /// Precomputed tag bitsets per match group (parallel to `match_groups`).
  /// `None` if the group has no `tags` filter.
  match_group_required_tags: Vec<Option<ModuleTagBitSet>>,
  plugin_driver: &'a SharedPluginDriver,
  input_base: &'a ArcStr,
  chunk_graph: &'a mut ChunkGraph,
  module_to_assigned: &'a mut IndexBitSet<ModuleIdx>,
  flattened_entries: Vec<&'a EntryPoint>,
}

impl ManualSplitter<'_> {
  async fn split(&mut self) -> BuildResult<()> {
    let (mut module_groups, entries_aware_groups) = self.build_module_groups().await?;

    if module_groups.iter().all(|group| group.modules.is_empty())
      && entries_aware_groups.iter().all(|group| group.modules.is_empty())
    {
      return Ok(());
    }

    self.process_entries_aware_groups(entries_aware_groups, &mut module_groups);

    let module_groups = self.into_priority_sorted_groups(module_groups);
    if module_groups.is_empty() {
      return Ok(());
    }

    self.convert_groups_to_chunks(module_groups);
    Ok(())
  }

  async fn build_module_groups(
    &self,
  ) -> BuildResult<(IndexVec<ModuleGroupIdx, ModuleGroup>, Vec<ModuleGroup>)> {
    let metas = &self.link_output.metas;
    let mut module_groups: IndexVec<ModuleGroupIdx, ModuleGroup> = IndexVec::default();
    let mut group_idx_by_id: FxHashMap<ModuleGroupId, ModuleGroupIdx> = FxHashMap::default();

    let mut entries_aware_groups: Vec<ModuleGroup> = Vec::new();
    let mut entries_aware_idx_by_id: FxHashMap<ModuleGroupId, usize> = FxHashMap::default();

    // Sort modules by `stable_id` before iterating so that user-supplied
    // `output.codeSplitting.groups[].name` functions are invoked in a deterministic order.
    // Without this, a stateful function would produce different chunk assignments across runs
    // because `ModuleIdx` is assigned in module-load completion order, which varies with
    // parallel `resolveId`/`load`.
    let sorted_normal_modules = self
      .link_output
      .module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .sorted_by(|a, b| a.stable_id.cmp(&b.stable_id));

    for normal_module in sorted_normal_modules {
      if !metas[normal_module.idx].is_included {
        continue;
      }

      if self.module_to_assigned.has_bit(normal_module.idx) {
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

        // Filter by module tags. See meta/design/module-tags.md
        if let Some(required_tags) = &self.match_group_required_tags[match_group_index] {
          if !splitting_info.tags_bit_set.contains_all(required_tags) {
            continue;
          }
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
        let module_group_id = ModuleGroupId { match_group_index, name: group_name.clone() };

        let include_dependencies_recursively = match_group
          .include_dependencies_recursively
          .or(self.chunking_options.include_dependencies_recursively)
          .unwrap_or(true);

        let group: &mut ModuleGroup = if entries_aware {
          let idx = match entries_aware_idx_by_id.entry(module_group_id) {
            std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
            std::collections::hash_map::Entry::Vacant(vacant) => {
              let idx = entries_aware_groups.len();
              entries_aware_groups.push(ModuleGroup {
                modules: FxHashSet::default(),
                match_group_index,
                priority: match_group.priority.unwrap_or(0),
                name: group_name,
                sizes: 0.0,
                entries_aware_bits: None,
              });
              *vacant.insert(idx)
            }
          };
          &mut entries_aware_groups[idx]
        } else {
          let idx = match group_idx_by_id.entry(module_group_id) {
            std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
            std::collections::hash_map::Entry::Vacant(vacant) => {
              let idx = module_groups.push(ModuleGroup {
                modules: FxHashSet::default(),
                match_group_index,
                priority: match_group.priority.unwrap_or(0),
                name: group_name,
                sizes: 0.0,
                entries_aware_bits: None,
              });
              *vacant.insert(idx)
            }
          };
          &mut module_groups[idx]
        };

        add_module_and_dependencies_to_group_recursively(
          group,
          normal_module.idx,
          &self.link_output.metas,
          &self.link_output.module_table,
          self.module_to_assigned,
          &mut FxHashSet::default(),
          include_dependencies_recursively,
        );
      }
    }

    Ok((module_groups, entries_aware_groups))
  }

  /// Post-process entries_aware groups: split each group's modules by bitset pattern,
  /// optionally merge small subgroups, then push finalized subgroups into module_groups.
  #[expect(clippy::cast_precision_loss)]
  fn process_entries_aware_groups(
    &self,
    entries_aware_groups: Vec<ModuleGroup>,
    module_groups: &mut IndexVec<ModuleGroupIdx, ModuleGroup>,
  ) {
    for group in entries_aware_groups {
      if group.modules.is_empty() {
        continue;
      }

      let match_group_index = group.match_group_index;
      let name = group.name.clone();
      let priority = group.priority;

      // Group modules by their bitset pattern into subgroups
      let mut bits_to_key: FxHashMap<BitSet, u32> = FxHashMap::default();
      let mut subgroups: FxHashMap<u32, EntriesAwareSubgroup> = FxHashMap::default();
      let mut next_key: u32 = 0;
      for module_idx in group.modules {
        let bits = &self.index_splitting_info[module_idx].bits;
        let key = match bits_to_key.entry(bits.clone()) {
          std::collections::hash_map::Entry::Occupied(occupied) => *occupied.get(),
          std::collections::hash_map::Entry::Vacant(vacant) => {
            let key = next_key;
            next_key = next_key.checked_add(1).expect("entries-aware subgroup key overflow");
            subgroups.insert(
              key,
              EntriesAwareSubgroup {
                bits: bits.clone(),
                modules: FxHashSet::default(),
                sizes: 0.0,
              },
            );
            *vacant.insert(key)
          }
        };
        let subgroup = subgroups.get_mut(&key).expect("subgroup key should exist");
        if subgroup.modules.insert(module_idx) {
          subgroup.sizes += self.link_output.module_table[module_idx].size() as f64;
        }
      }

      // Optionally merge small subgroups
      let merge_threshold =
        self.match_groups[match_group_index].entries_aware_merge_threshold.unwrap_or(0.0);
      if merge_threshold > 0.0 && subgroups.len() > 1 {
        let keys: Vec<u32> = subgroups.keys().copied().collect();
        merge_entries_aware_subgroups(
          &mut subgroups,
          &keys,
          merge_threshold,
          &self.link_output.module_table,
        );
      }

      // Convert each subgroup into a ModuleGroup and push into the IndexVec
      for (_, subgroup) in subgroups {
        if subgroup.modules.is_empty() {
          continue;
        }
        module_groups.push(ModuleGroup {
          name: name.clone(),
          match_group_index,
          modules: subgroup.modules,
          priority,
          sizes: subgroup.sizes,
          entries_aware_bits: Some(subgroup.bits),
        });
      }
    }
  }

  fn into_priority_sorted_groups(
    &self,
    module_groups: IndexVec<ModuleGroupIdx, ModuleGroup>,
  ) -> Vec<ModuleGroup> {
    let mut module_groups =
      module_groups.into_iter().filter(|group| !group.modules.is_empty()).collect::<Vec<_>>();
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
          if let Some(pieces) =
            self.partition_oversized_group(&group, allow_min_size, allow_max_size)
          {
            for piece in pieces {
              self.emit_chunk_from_group(&piece, &mut module_groups);
            }
            continue;
          }
        }
      }

      self.emit_chunk_from_group(&group, &mut module_groups);
    }
  }

  /// Partitions an oversized group into size-bounded pieces along stable-id relevance
  /// boundaries. The stable-id sort, prefix sizes, and adjacency similarities are
  /// computed once for the whole group, then a single recursive pass over index ranges
  /// selects split points — avoiding the per-level re-sorting and intermediate set
  /// allocations a worklist-driven bisection would incur. Boundary selection and the
  /// min/max feasibility rules are identical to a top-down bisection, so the resulting
  /// partition is the same contiguous one; only the work to compute it differs.
  ///
  /// Returns `None` when the group cannot be split (e.g. `minSize` forbids every cut),
  /// leaving the caller to emit it unchanged as an oversized chunk.
  fn partition_oversized_group(
    &self,
    group: &ModuleGroup,
    min_size: f64,
    max_size: f64,
  ) -> Option<Vec<ModuleGroup>> {
    let mut modules = group.modules.iter().copied().collect::<Vec<_>>();
    // Sort by lexical relevance first (stable module id), then by execution order.
    modules.sort_by(|lhs, rhs| {
      let lhs_module = &self.link_output.module_table[*lhs];
      let rhs_module = &self.link_output.module_table[*rhs];
      lhs_module
        .stable_id()
        .cmp(rhs_module.stable_id())
        .then(lhs_module.exec_order().cmp(&rhs_module.exec_order()))
    });

    let keys = modules
      .iter()
      .map(|module_idx| self.link_output.module_table[*module_idx].stable_id().as_str())
      .collect::<Vec<_>>();
    let prefix_sizes = prefix_group_sizes(&modules, &self.link_output.module_table);

    let mut ranges = Vec::new();
    collect_split_ranges(&keys, &prefix_sizes, 0, modules.len(), min_size, max_size, &mut ranges);

    if ranges.len() <= 1 {
      return None;
    }

    Some(
      ranges
        .into_iter()
        .map(|(lo, hi)| ModuleGroup {
          name: group.name.clone(),
          match_group_index: group.match_group_index,
          modules: modules[lo..hi].iter().copied().collect(),
          priority: group.priority,
          sizes: prefix_sizes[hi] - prefix_sizes[lo],
          entries_aware_bits: group.entries_aware_bits.clone(),
        })
        .collect(),
    )
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
      derive_entries_aware_chunk_name(
        &group.name,
        chunk_bits,
        &self.flattened_entries,
        self.link_output,
      )
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
      self.module_to_assigned.set_bit(module_idx);
    });
  }
}

impl GenerateStage<'_> {
  pub async fn apply_manual_code_splitting(
    &self,
    index_splitting_info: &IndexSplittingInfo,
    module_to_assigned: &mut IndexBitSet<ModuleIdx>,
    chunk_graph: &mut ChunkGraph,
    input_base: &ArcStr,
    tag_registry: &ModuleTagRegistry,
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

    let flattened_entries: Vec<&EntryPoint> =
      self.link_output.entries.iter().flat_map(|(_idx, entries)| entries.iter()).collect();
    let match_group_required_tags: Vec<Option<ModuleTagBitSet>> = match_groups
      .iter()
      .map(|group| group.tags.as_ref().map(|tags| tag_registry.compile_tags_to_bit_set(tags)))
      .collect();
    let mut splitter = ManualSplitter {
      link_output: self.link_output,
      index_splitting_info,
      options: self.options,
      chunking_options,
      match_groups,
      match_group_required_tags,
      plugin_driver: self.plugin_driver,
      input_base,
      chunk_graph,
      module_to_assigned,
      flattened_entries,
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
  subgroups: &mut FxHashMap<u32, EntriesAwareSubgroup>,
  group_keys: &[u32],
  threshold: f64,
  module_table: &ModuleTable,
) {
  let mut version_by_key: FxHashMap<u32, u32> = FxHashMap::default();
  let mut unqualified_heap: BinaryHeap<Reverse<(OrderedSize, u32, u32)>> = BinaryHeap::new();

  for &group_key in group_keys {
    let Some(group) = subgroups.get(&group_key) else {
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
    let Some(candidate_group) = subgroups.get(&candidate_key) else {
      continue;
    };
    if candidate_group.modules.is_empty()
      || candidate_version != current_version(&version_by_key, candidate_key)
      || !is_below_merge_threshold(candidate_group.sizes, threshold)
    {
      continue;
    }

    let candidate_bits = &candidate_group.bits;

    let mut best_target = None;
    for &target_key in group_keys {
      if target_key == candidate_key {
        continue;
      }

      let Some(target_group) = subgroups.get(&target_key) else {
        continue;
      };
      if target_group.modules.is_empty() {
        continue;
      }

      let score = (
        symmetric_difference_count(candidate_bits, &target_group.bits),
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

    merge_subgroups(subgroups, candidate_key, target_key, module_table);
    bump_version(&mut version_by_key, candidate_key);
    bump_version(&mut version_by_key, target_key);

    let Some(target_group) = subgroups.get(&target_key) else {
      continue;
    };
    if is_below_merge_threshold(target_group.sizes, threshold) && !target_group.modules.is_empty() {
      unqualified_heap.push(Reverse((
        OrderedSize(target_group.sizes),
        target_key,
        current_version(&version_by_key, target_key),
      )));
    }
  }
}

fn merge_subgroups(
  subgroups: &mut FxHashMap<u32, EntriesAwareSubgroup>,
  from_key: u32,
  to_key: u32,
  module_table: &ModuleTable,
) {
  if from_key == to_key {
    return;
  }

  let Some(mut from_group) = subgroups.remove(&from_key) else {
    return;
  };
  let Some(to_group) = subgroups.get_mut(&to_key) else {
    subgroups.insert(from_key, from_group);
    return;
  };

  to_group.modules.extend(from_group.modules.drain());
  to_group.sizes = sum_group_sizes(&to_group.modules, module_table);
  to_group.bits.union(&from_group.bits);
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

/// Similarity differences within this threshold are treated as insignificant ties,
/// allowing size-based criteria to decide. Value of 10 equals one character position's
/// max score, absorbing digit-level ASCII noise while preserving directory boundary signals.
const SIMILARITY_SIGNIFICANCE_THRESHOLD: i32 = 10;

fn stable_id_similarity(lhs: &str, rhs: &str) -> i32 {
  lhs.as_bytes().iter().zip(rhs.as_bytes()).fold(0, |acc, (lhs_char, rhs_char)| {
    acc + (10 - (i32::from(*lhs_char) - i32::from(*rhs_char)).abs()).max(0)
  })
}

#[expect(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here.
fn prefix_group_sizes(modules: &[ModuleIdx], module_table: &ModuleTable) -> Vec<f64> {
  let mut prefix_sizes = Vec::with_capacity(modules.len() + 1);
  prefix_sizes.push(0.0);
  for module_idx in modules {
    let next_size =
      prefix_sizes.last().copied().unwrap_or_default() + module_table[*module_idx].size() as f64;
    prefix_sizes.push(next_size);
  }
  prefix_sizes
}

/// Recursively partitions `[lo, hi)` into contiguous, size-bounded ranges, cutting at
/// the most relevant boundary (see [`pick_split_index_in_range`]) until each range fits
/// within `max_size`. Ranges that cannot be split without violating `min_size` are kept
/// whole, so an unsplittable oversized run is emitted as a single oversized chunk.
fn collect_split_ranges(
  keys: &[&str],
  prefix_sizes: &[f64],
  lo: usize,
  hi: usize,
  min_size: f64,
  max_size: f64,
  out: &mut Vec<(usize, usize)>,
) {
  let range_size = prefix_sizes[hi] - prefix_sizes[lo];
  if range_size <= max_size {
    out.push((lo, hi));
    return;
  }

  match pick_split_index_in_range(keys, prefix_sizes, lo, hi, min_size, max_size) {
    Some(split) => {
      collect_split_ranges(keys, prefix_sizes, lo, split, min_size, max_size, out);
      collect_split_ranges(keys, prefix_sizes, split, hi, min_size, max_size, out);
    }
    None => out.push((lo, hi)),
  }
}

/// Selects the best split point within `[lo, hi)` over a shared, group-wide
/// `prefix_sizes` array. Boundaries are ranked by stable-id similarity first (lower is a
/// stronger relevance boundary), with similarity differences within
/// [`SIMILARITY_SIGNIFICANCE_THRESHOLD`] treated as ties broken by fewer oversized sides
/// and then a smaller maximum side. Returns `None` when `min_size` forbids every cut.
fn pick_split_index_in_range(
  keys: &[&str],
  prefix_sizes: &[f64],
  lo: usize,
  hi: usize,
  min_size: f64,
  max_size: f64,
) -> Option<usize> {
  if hi - lo < 2 {
    return None;
  }

  let total_size = prefix_sizes[hi] - prefix_sizes[lo];

  // Find the leftmost split point that can satisfy min_size for the left group.
  let mut left_bound = lo + 1;
  while left_bound < hi && prefix_sizes[left_bound] - prefix_sizes[lo] < min_size {
    left_bound += 1;
  }

  // Find the rightmost split point that can satisfy min_size for the right group.
  let mut right_bound = hi - 1;
  while right_bound > lo && prefix_sizes[hi] - prefix_sizes[right_bound] < min_size {
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
    let left_size = prefix_sizes[split_index] - prefix_sizes[lo];
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

  best_split_index
}

fn derive_entries_aware_chunk_name(
  group_name: &str,
  bits: &BitSet,
  flattened_entries: &[&EntryPoint],
  link_output: &crate::stages::link_stage::LinkStageOutput,
) -> ArcStr {
  const MAX_CHUNK_NAME_LEN: usize = 100;
  const HASH_DISPLAY_LEN: usize = 8;
  const TRUNCATED_LEN: usize = MAX_CHUNK_NAME_LEN - HASH_DISPLAY_LEN - 1; // 1 for the `~` separator

  let entry_names: Vec<String> = bits
    .index_of_one()
    .filter_map(|index| {
      let idx = index as usize;
      if idx < flattened_entries.len() {
        let entry_point = flattened_entries[idx];
        Some(entry_point.name.as_ref().map(ArcStr::to_string).unwrap_or_else(|| {
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
  module_to_assigned: &IndexBitSet<ModuleIdx>,
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

  if module_to_assigned.has_bit(module_idx) {
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
        module_to_assigned,
        visited,
        recursively,
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{collect_split_ranges, pick_split_index_in_range, stable_id_similarity};

  /// Runs the full recursive partition and returns each piece as a list of keys.
  fn partition(keys: &[&str], sizes: &[f64], min_size: f64, max_size: f64) -> Vec<Vec<String>> {
    let prefix = prefix_sizes(sizes);
    let mut ranges = Vec::new();
    collect_split_ranges(keys, &prefix, 0, keys.len(), min_size, max_size, &mut ranges);
    ranges
      .iter()
      .map(|(lo, hi)| keys[*lo..*hi].iter().map(|key| (*key).to_string()).collect())
      .collect()
  }

  fn prefix_sizes(sizes: &[f64]) -> Vec<f64> {
    let mut prefix = Vec::with_capacity(sizes.len() + 1);
    prefix.push(0.0);
    for size in sizes {
      let next = prefix.last().copied().unwrap_or_default() + *size;
      prefix.push(next);
    }
    prefix
  }

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
    let prefix = prefix_sizes(case.sizes);
    let result = pick_split_index_in_range(
      case.keys,
      &prefix,
      0,
      case.keys.len(),
      case.min_size,
      case.max_size,
    );
    match (result, case.expected) {
      (Some(split), Some(expected)) => {
        let (left, right) = (&case.keys[..split], &case.keys[split..]);
        assert_eq!((left, right), expected, "similarities: {similarities:?}");
      }
      (None, None) => {}
      (Some(split), None) => {
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

  #[test]
  fn partition_keeps_each_fitting_package_together() {
    // Three vendor packages, each pair fits within max_size; cross-package boundaries
    // are the relevant cuts, so every package ends up in its own chunk.
    let pieces = partition(
      &[
        "node_modules/lodash/a.js",
        "node_modules/lodash/b.js",
        "node_modules/react/a.js",
        "node_modules/react/b.js",
        "node_modules/vue/a.js",
        "node_modules/vue/b.js",
      ],
      &[30.0, 30.0, 30.0, 30.0, 30.0, 30.0],
      0.0,
      70.0,
    );
    assert_eq!(
      pieces,
      vec![
        vec!["node_modules/lodash/a.js", "node_modules/lodash/b.js"],
        vec!["node_modules/react/a.js", "node_modules/react/b.js"],
        vec!["node_modules/vue/a.js", "node_modules/vue/b.js"],
      ],
    );
  }

  #[test]
  fn partition_groups_scoped_package_siblings() {
    // Scoped-package siblings share a long prefix, so they stay together and split away
    // from the unrelated package.
    let pieces = partition(
      &[
        "node_modules/@scope/pkg-a/index.js",
        "node_modules/@scope/pkg-b/index.js",
        "node_modules/zod/index.js",
      ],
      &[40.0, 40.0, 40.0],
      0.0,
      90.0,
    );
    assert_eq!(
      pieces,
      vec![
        vec!["node_modules/@scope/pkg-a/index.js", "node_modules/@scope/pkg-b/index.js"],
        vec!["node_modules/zod/index.js"],
      ],
    );
  }

  #[test]
  fn partition_groups_virtual_module_siblings() {
    // Virtual ids (leading NUL) must be scored without panicking and grouped by prefix.
    let pieces = partition(
      &["\0virtual:polyfill-a.js", "\0virtual:polyfill-b.js", "node_modules/react/index.js"],
      &[40.0, 40.0, 40.0],
      0.0,
      90.0,
    );
    assert_eq!(
      pieces,
      vec![
        vec!["\0virtual:polyfill-a.js", "\0virtual:polyfill-b.js"],
        vec!["node_modules/react/index.js"],
      ],
    );
  }

  #[test]
  fn partition_emits_unsplittable_oversized_module_as_singleton() {
    // `a.js` alone exceeds max_size and min_size forbids re-pairing it, so it is emitted
    // as an oversized singleton while the remainder is grouped normally.
    let pieces = partition(&["a.js", "b.js", "c.js"], &[100.0, 30.0, 30.0], 50.0, 80.0);
    assert_eq!(pieces, vec![vec!["a.js"], vec!["b.js", "c.js"]]);
  }

  #[test]
  fn partition_without_common_prefix_stays_within_size_bounds() {
    // Unrelated ids have no relevance signal; the partition must still be a valid,
    // exhaustive cover with every non-singleton piece within max_size.
    let keys = &["alpha.js", "beta.js", "gamma.js", "delta.js"];
    let sizes = &[30.0, 30.0, 30.0, 30.0];
    let pieces = partition(keys, sizes, 0.0, 70.0);

    let flattened: Vec<String> = pieces.iter().flatten().cloned().collect();
    let expected: Vec<String> = keys.iter().map(|key| (*key).to_string()).collect();
    assert_eq!(flattened, expected, "partition must be a contiguous, exhaustive cover");
    // Each module is 30.0 and max_size is 70.0, so any non-singleton piece holds <= 2.
    for piece in &pieces {
      assert!(piece.len() <= 2, "non-singleton piece {piece:?} exceeds max_size");
    }
  }
}
