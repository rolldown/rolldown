use std::{
  borrow::Cow,
  cmp::{Ordering, Reverse},
  collections::BinaryHeap,
  path::Path,
  sync::Arc,
};

use arcstr::ArcStr;
use itertools::{Either, Itertools};
use oxc_index::IndexVec;
use rolldown_common::{
  Chunk, ChunkKind, ChunkingContext, EntryPoint, EntryPointKind, ManualCodeSplittingOptions,
  MatchGroup, MatchGroupName, Module, ModuleIdx, ModuleTable, ModuleTagBitSet, ModuleTagRegistry,
  NormalModule,
};
use rolldown_error::BuildResult;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::{BitSet, IndexBitSet, concat_string, xxhash::xxhash_with_base};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::utils::module_id_matcher::match_module_ids;
use crate::{
  SharedOptions, chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput,
  types::linking_metadata::LinkingMetadataVec,
  utils::topology_config::{DependencyBoundary, TopologyConfig},
};

use super::{
  GenerateStage,
  chunk_ext::{ChunkCreationReason, ChunkDebugExt},
  code_splitting::{
    BUILTIN_APP_TAG_BIT, BUILTIN_ASYNC_TAG_BIT, BUILTIN_SHARED_TAG_BIT, BUILTIN_VENDOR_TAG_BIT,
    IndexSplittingInfo,
  },
};
use rolldown_common::ModuleTag;
use sugar_path::SugarPath as _;

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
  topology_config: Option<Arc<TopologyConfig>>,
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

  /// Runs the checks that sit between a group's `test` call and its `name` call.
  ///
  /// They read data that only Rust holds. That is why `test` and `name` need two calls.
  fn passes_static_filters(
    &self,
    match_group_index: usize,
    match_group: &MatchGroup,
    module: &NormalModule,
  ) -> bool {
    let splitting_info = &self.index_splitting_info[module.idx];

    // Filter by module tags. See internal-docs/module-tags/implementation.md
    if let Some(required_tags) = &self.match_group_required_tags[match_group_index] {
      if !splitting_info.tags_bit_set.contains_all(required_tags) {
        return false;
      }
    }

    let allow_min_module_size =
      match_group.min_module_size.map_or(self.chunking_options.min_module_size, Some);
    let allow_max_module_size =
      match_group.max_module_size.map_or(self.chunking_options.max_module_size, Some);

    let is_min_module_size_satisfied =
      allow_min_module_size.is_none_or(|min_module_size| module.size() >= min_module_size);
    let is_max_module_size_satisfied =
      allow_max_module_size.is_none_or(|max_module_size| module.size() <= max_module_size);

    if !is_min_module_size_satisfied || !is_max_module_size_satisfied {
      return false;
    }

    if let Some(allow_min_share_count) =
      match_group.min_share_count.map_or(self.chunking_options.min_share_count, Some)
    {
      if splitting_info.share_count < allow_min_share_count {
        return false;
      }
    }

    true
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
    let candidate_modules = self
      .link_output
      .module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .filter(|module| {
        metas[module.idx].is_included && !self.module_to_assigned.has_bit(module.idx)
      })
      .sorted_unstable_by(|a, b| a.stable_id.cmp(&b.stable_id))
      .collect_vec();

    // `test` and `name` are JS callbacks, so one call per module would cross the napi boundary
    // M * G times. One batched call per group replaces that.
    //
    // A group cannot narrow `candidate_modules` to what earlier groups left behind. Priority
    // resolves the overlap later, in `emit_chunk_from_group`. If this loop dropped matched
    // modules, a module would go to the first group in the list, not to the group with the
    // highest priority.
    let ctx = ChunkingContext::new(Arc::clone(&self.plugin_driver.module_infos));

    for (match_group_index, match_group) in self.match_groups.iter().copied().enumerate() {
      let group_rule = self
        .topology_config
        .as_ref()
        .and_then(|cfg| cfg.group_rules.get(match_group_index));

      let matched = if let Some(rule) = group_rule
        && (!rule.packages.is_empty() || !rule.paths.is_empty())
      {
        candidate_modules
          .iter()
          .map(|module| matches_group_rule(module, rule, &self.options.cwd))
          .collect_vec()
      } else {
        match &match_group.test {
          None => vec![true; candidate_modules.len()],
          Some(test) => {
            let ids = candidate_modules.iter().map(|module| module.id.as_str()).collect_vec();
            match_module_ids(test, &ids, "a `codeSplitting` group `test` function").await?
          }
        }
      };

      let captured = candidate_modules
        .iter()
        .copied()
        .zip(matched)
        .filter(|(module, is_matched)| {
          *is_matched && self.passes_static_filters(match_group_index, match_group, module)
        })
        .map(|(module, _)| module)
        .collect_vec();

      if captured.is_empty() {
        continue;
      }

      let names = if let Some(rule) = group_rule
        && let Some(naming) = &rule.naming
      {
        captured
          .iter()
          .map(|module| resolve_group_naming(module, naming, &self.options.cwd).map(Cow::Owned))
          .collect_vec()
      } else {
        match &match_group.name {
          MatchGroupName::Static(name) => vec![Some(Cow::Borrowed(name.as_str())); captured.len()],
          MatchGroupName::Dynamic(_) => {
            let module_ids = captured.iter().map(|module| module.id.to_string()).collect_vec();
            match_group.name.values(&ctx, module_ids).await?
          }
        }
      };

      for (normal_module, group_name) in captured.into_iter().zip(names) {
        let Some(group_name) = group_name else {
          // Group which doesn't have a name will be ignored.
          continue;
        };
        let group_name = ArcStr::from(group_name.as_ref());

        let entries_aware = match_group.entries_aware.unwrap_or(false);
        let module_group_id = ModuleGroupId { match_group_index, name: group_name.clone() };

        let include_dependencies_recursively = match_group
          .include_dependencies_recursively
          .or(self.chunking_options.include_dependencies_recursively)
          .unwrap_or(true);
        let boundary = self
          .topology_config
          .as_ref()
          .and_then(|cfg| cfg.group_boundaries.get(match_group_index).copied())
          .unwrap_or(if include_dependencies_recursively {
            DependencyBoundary::All
          } else {
            DependencyBoundary::None
          });
        let is_mf_shared_group = self
          .topology_config
          .as_ref()
          .and_then(|cfg| cfg.group_is_mf_shared.get(match_group_index).copied())
          .unwrap_or(false);
        let mf_shared_packages = self
          .topology_config
          .as_ref()
          .map_or(&[][..], |cfg| cfg.mf_shared_packages.as_slice());

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
          normal_module.idx,
          &self.link_output.metas,
          &self.link_output.module_table,
          self.module_to_assigned,
          &mut FxHashSet::default(),
          boundary,
          is_mf_shared_group,
          mf_shared_packages,
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
      let group_overfetch_budget = self
        .topology_config
        .as_ref()
        .and_then(|cfg| cfg.group_overfetch_budget_bytes.get(match_group_index).copied().flatten())
        .or_else(|| self.topology_config.as_ref().and_then(|cfg| cfg.overfetch_budget_bytes))
        .map(f64::from);
      let raw_merge_threshold =
        self.match_groups[match_group_index].entries_aware_merge_threshold.unwrap_or(0.0);
      let merge_threshold = match (raw_merge_threshold > 0.0, group_overfetch_budget) {
        (true, Some(budget)) => raw_merge_threshold.min(budget),
        (true, None) => raw_merge_threshold,
        (false, Some(budget)) => budget,
        (false, None) => 0.0,
      };
      let allow_max_size = self.match_groups[match_group_index]
        .max_size
        .map_or(self.chunking_options.max_size, Some)
        .or_else(|| {
          self.topology_config.as_ref().and_then(|cfg| cfg.chunk_size.map(|c| f64::from(c.max_bytes)))
        });
      let group_modules = group.modules.into_iter();
      // Merge ties use subgroup keys, so assign them in execution order.
      // See internal-docs/manual-code-splitting/implementation.md.
      let group_modules = if merge_threshold > 0.0 {
        Either::Left(
          group_modules
            .sorted_unstable_by_key(|idx| self.link_output.module_table[*idx].exec_order()),
        )
      } else {
        Either::Right(group_modules)
      };

      // Group modules by their bitset pattern into subgroups
      let mut bits_to_key: FxHashMap<BitSet, u32> = FxHashMap::default();
      let mut subgroups: FxHashMap<u32, EntriesAwareSubgroup> = FxHashMap::default();
      let mut next_key: u32 = 0;
      for module_idx in group_modules {
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
      if merge_threshold > 0.0 && subgroups.len() > 1 {
        let mut user_defined_entry_bits = BitSet::new(self.flattened_entries.len() as u32);
        for (entry_idx, entry_point) in self.flattened_entries.iter().enumerate() {
          if matches!(
            entry_point.kind,
            EntryPointKind::UserDefined | EntryPointKind::EmittedUserDefined
          ) {
            user_defined_entry_bits.set_bit(entry_idx as u32);
          }
        }
        let mut keys: Vec<u32> = subgroups.keys().copied().collect();
        keys.sort_unstable();
        merge_entries_aware_subgroups(
          &mut subgroups,
          &keys,
          merge_threshold,
          allow_max_size,
          &user_defined_entry_bits,
          self.index_splitting_info,
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

      let is_mf_shared = self
        .topology_config
        .as_ref()
        .and_then(|cfg| cfg.group_is_mf_shared.get(group.match_group_index).copied())
        .unwrap_or(false);

      let allow_min_size = if is_mf_shared {
        0.0
      } else {
        self.match_groups[group.match_group_index]
          .min_size
          .map_or(self.chunking_options.min_size, Some)
          .or_else(|| {
            self
              .topology_config
              .as_ref()
              .and_then(|cfg| cfg.chunk_size.map(|c| f64::from(c.min_bytes)))
          })
          .unwrap_or(0.0)
      };

      if group.sizes < allow_min_size {
        continue;
      }

      if !is_mf_shared
        && let Some(allow_max_size) = self.match_groups[group.match_group_index]
          .max_size
          .map_or(self.chunking_options.max_size, Some)
          .or_else(|| {
            self
              .topology_config
              .as_ref()
              .and_then(|cfg| cfg.chunk_size.map(|c| f64::from(c.max_bytes)))
          })
      {
        if group.sizes > allow_max_size {
          let target_size = self
            .topology_config
            .as_ref()
            .and_then(|cfg| {
              cfg
                .group_target_bytes
                .get(group.match_group_index)
                .copied()
                .flatten()
                .or_else(|| cfg.chunk_size.and_then(|c| c.target_bytes))
            })
            .map(f64::from);
          if let Some((left, right)) =
            self.try_split_oversized_group(&group, allow_min_size, allow_max_size, target_size)
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
    target_size: Option<f64>,
  ) -> Option<(ModuleGroup, ModuleGroup)> {
    let mut modules = group.modules.iter().copied().collect::<Vec<_>>();
    // Split by lexical relevance first (using pkg:// key for node_modules so .pnpm/ prefixes do not
    // distort package similarity), then by size constraints.
    modules.sort_unstable_by(|lhs, rhs| {
      let lhs_module = &self.link_output.module_table[*lhs];
      let rhs_module = &self.link_output.module_table[*rhs];
      let lhs_key = module_similarity_key(lhs_module);
      let rhs_key = module_similarity_key(rhs_module);
      lhs_key
        .as_ref()
        .cmp(rhs_key.as_ref())
        .then_with(|| lhs_module.stable_id().cmp(rhs_module.stable_id()))
        .then(lhs_module.exec_order().cmp(&rhs_module.exec_order()))
    });

    let (split_index, left_size, right_size) = find_relevance_split_index(
      &modules,
      &self.link_output.module_table,
      min_size,
      max_size,
      target_size,
    )?;

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

    let _ = tag_registry;
    let flattened_entries: Vec<&EntryPoint> =
      self.link_output.entries.iter().flat_map(|(_idx, entries)| entries.iter()).collect();
    let match_group_required_tags: Vec<Option<ModuleTagBitSet>> = match_groups
      .iter()
      .map(|group| group.tags.as_ref().map(|tags| compile_builtin_tags_to_bit_set(tags)))
      .collect();
    let mut splitter = ManualSplitter {
      link_output: self.link_output,
      index_splitting_info,
      options: self.options,
      chunking_options,
      topology_config: crate::utils::topology_config::get(self.options),
      match_groups,
      match_group_required_tags,
      plugin_driver: self.plugin_driver,
      input_base,
      chunk_graph,
      module_to_assigned,
      flattened_entries,
    };
    let result = splitter.split().await;
    // Release the JS module-info cache on both success and failure.
    // See internal-docs/manual-code-splitting/implementation.md.
    if let Some(invalidate_module_info_cache) =
      &chunking_options.internal_invalidate_module_info_cache
    {
      return result.and(invalidate_module_info_cache.call().await.map_err(Into::into));
    }
    result
  }
}

fn compile_builtin_tags_to_bit_set(tags: &[ModuleTag]) -> ModuleTagBitSet {
  let mut bits = ModuleTagBitSet::default();
  for tag in tags {
    match tag {
      ModuleTag::Initial => bits.set_bit(ModuleTag::INITIAL_BIT),
      ModuleTag::Custom(name) => match name.as_str() {
        "$initial" => bits.set_bit(ModuleTag::INITIAL_BIT),
        "$async" => bits.set_bit(BUILTIN_ASYNC_TAG_BIT),
        "$vendor" => bits.set_bit(BUILTIN_VENDOR_TAG_BIT),
        "$app" => bits.set_bit(BUILTIN_APP_TAG_BIT),
        "$shared" => bits.set_bit(BUILTIN_SHARED_TAG_BIT),
        _ => {}
      },
    }
  }
  bits
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

fn has_user_defined_entry(bits: &BitSet, user_defined_entry_bits: &BitSet) -> bool {
  bits.index_of_one().any(|bit| user_defined_entry_bits.has_bit(bit))
}

fn combined_overfetch_exceeds_budget(
  candidate: &EntriesAwareSubgroup,
  target: &EntriesAwareSubgroup,
  threshold: f64,
  index_splitting_info: &IndexSplittingInfo,
  module_table: &ModuleTable,
) -> bool {
  // Check directional overfetch for every entry in candidate_bits ^ target_bits (and any entry
  // in the combined bitset that already has overfetched modules from prior merges).
  let mut union_bits = candidate.bits.clone();
  union_bits.union(&target.bits);

  // Fast directional check on the two subgroups' total sizes:
  // If candidate has an entry not in target, that entry is forced to load target.sizes.
  let candidate_has_extra =
    candidate.bits.index_of_one().any(|bit| !target.bits.has_bit(bit));
  if candidate_has_extra && !is_below_merge_threshold(target.sizes, threshold) {
    return true;
  }
  // If target has an entry not in candidate, that entry is forced to load candidate.sizes.
  let target_has_extra =
    target.bits.index_of_one().any(|bit| !candidate.bits.has_bit(bit));
  if target_has_extra && !is_below_merge_threshold(candidate.sizes, threshold) {
    return true;
  }

  // Exact per-entry overfetch check across all modules in the merged subgroup:
  for entry_bit in union_bits.index_of_one() {
    let mut overfetch_bytes = 0.0;
    for &module_idx in candidate.modules.iter().chain(target.modules.iter()) {
      if !index_splitting_info[module_idx].bits.has_bit(entry_bit) {
        overfetch_bytes += module_size(module_idx, module_table);
        if overfetch_bytes >= threshold {
          return true;
        }
      }
    }
  }
  false
}

fn merge_entries_aware_subgroups(
  subgroups: &mut FxHashMap<u32, EntriesAwareSubgroup>,
  group_keys: &[u32],
  threshold: f64,
  max_size: Option<f64>,
  user_defined_entry_bits: &BitSet,
  index_splitting_info: &IndexSplittingInfo,
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
    let candidate_is_initial = has_user_defined_entry(candidate_bits, user_defined_entry_bits);

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

      // Prevent merging an async-only subgroup (no UserDefinedEntry bits) with an initial subgroup.
      let target_is_initial = has_user_defined_entry(&target_group.bits, user_defined_entry_bits);
      if candidate_is_initial != target_is_initial {
        continue;
      }

      // Respect max_size if configured.
      if let Some(max_size) = max_size
        && candidate_group.sizes + target_group.sizes > max_size
      {
        continue;
      }

      // Ensure directional overfetch imposed on any entry does not exceed threshold / overfetch_budget_bytes.
      if combined_overfetch_exceeds_budget(
        candidate_group,
        target_group,
        threshold,
        index_splitting_info,
        module_table,
      ) {
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

fn is_vendor_module(module: &NormalModule) -> bool {
  let id = module.id.as_str();
  id.contains("/node_modules/")
    || id.contains("\\node_modules\\")
    || id.starts_with("node_modules/")
    || id.starts_with("node_modules\\")
    || id.contains("/.store/")
    || id.contains("\\.store\\")
}

/// Extract the resolved package name of a `node_modules` module from `originative_resolved_id.package_json`
/// or by parsing its `node_modules` path segment.
pub fn module_package_name(module: &NormalModule) -> Option<Cow<'_, str>> {
  if !is_vendor_module(module) {
    return None;
  }
  if let Some(pkg_json) = module.originative_resolved_id.package_json.as_ref()
    && let Some(name) = pkg_json.name()
    && !name.is_empty()
  {
    return Some(Cow::Borrowed(name));
  }
  let normalized = module.id.as_str().replace('\\', "/");
  let (_, after_nm) = normalized.rsplit_once("node_modules/")?;
  let mut parts = after_nm.split('/');
  let first = parts.next()?;
  if first.starts_with('@') {
    let second = parts.next()?;
    Some(Cow::Owned(format!("{first}/{second}")))
  } else {
    Some(Cow::Owned(first.to_string()))
  }
}

/// Extract the resolved package version of a `node_modules` module from `originative_resolved_id.package_json`
/// or from a `.pnpm/<pkg>@<version>` path segment.
pub fn module_package_version(module: &NormalModule) -> Option<Cow<'_, str>> {
  if !is_vendor_module(module) {
    return None;
  }
  if let Some(pkg_json) = module.originative_resolved_id.package_json.as_ref()
    && let Some(version) = pkg_json.version()
    && !version.is_empty()
  {
    return Some(Cow::Borrowed(version));
  }
  let normalized = module.id.as_str().replace('\\', "/");
  if let Some((_, after_pnpm)) = normalized.split_once(".pnpm/")
    && let Some(first_seg) = after_pnpm.split('/').next()
    && let Some((_, ver_rest)) = first_seg.rsplit_once('@')
  {
    let ver = ver_rest.split('_').next().unwrap_or(ver_rest);
    if !ver.is_empty() {
      return Some(Cow::Owned(ver.to_string()));
    }
  }
  None
}

pub fn package_pattern_matches(pattern: &str, pkg_name: &str) -> bool {
  if pattern == "*" {
    return true;
  }
  if let Some(scope_prefix) = pattern.strip_suffix("/*") {
    return pkg_name
      .strip_prefix(scope_prefix)
      .is_some_and(|rest| rest.starts_with('/') && !rest[1..].is_empty() && !rest[1..].contains('/'));
  }
  if let Some(prefix) = pattern.strip_suffix('*') {
    return pkg_name.starts_with(prefix);
  }
  pkg_name == pattern
}

pub fn module_root_relative_path<'a>(module: &'a NormalModule, cwd: &Path) -> Cow<'a, str> {
  let stable = module.stable_id.as_str();
  if !Path::new(stable).is_absolute() && !stable.starts_with('/') {
    return Cow::Borrowed(stable);
  }
  let id_slash = module.id.as_str().replace('\\', "/");
  let cwd_slash = cwd.to_slash_lossy();
  let cwd_trimmed = cwd_slash.trim_end_matches('/');
  if !cwd_trimmed.is_empty()
    && let Some(rest) = id_slash.strip_prefix(cwd_trimmed)
    && let Some(rel) = rest.strip_prefix('/')
  {
    return Cow::Owned(rel.to_string());
  }
  if let Some((_, rel)) = id_slash.split_once("__native_inputs__/") {
    return Cow::Owned(rel.to_string());
  }
  Cow::Owned(stable.trim_start_matches('/').to_string())
}

fn matches_group_rule(
  module: &NormalModule,
  rule: &crate::utils::topology_config::GroupRuleConfig,
  cwd: &Path,
) -> bool {
  if !rule.packages.is_empty()
    && let Some(pkg_name) = module_package_name(module)
    && rule.packages.iter().any(|pattern| package_pattern_matches(pattern, pkg_name.as_ref()))
  {
    return true;
  }
  if !rule.paths.is_empty() && !is_vendor_module(module) {
    let rel_path = module_root_relative_path(module, cwd);
    let rel_str = rel_path.as_ref().trim_start_matches("./");
    if rule.paths.iter().any(|prefix| {
      let clean = prefix.trim_start_matches("./").trim_end_matches('/');
      clean.is_empty()
        || rel_str == clean
        || rel_str
          .strip_prefix(clean)
          .is_some_and(|rest| rest.starts_with('/'))
    }) {
      return true;
    }
  }
  false
}

fn sanitize_chunk_segment(raw: &str) -> String {
  raw
    .trim_start_matches('@')
    .chars()
    .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' { ch } else { '-' })
    .collect::<String>()
    .trim_matches('-')
    .to_string()
}

fn resolve_group_naming(
  module: &NormalModule,
  naming: &crate::utils::topology_config::GroupNamingPolicy,
  cwd: &Path,
) -> Option<String> {
  use crate::utils::topology_config::GroupNamingPolicy;
  match naming {
    GroupNamingPolicy::Static(name) => Some(name.clone()),
    GroupNamingPolicy::ByPackage { prefix, disambiguate_version } => {
      let pkg = module_package_name(module)?;
      let pkg_slug = sanitize_chunk_segment(pkg.as_ref());
      if pkg_slug.is_empty() {
        return Some(prefix.clone());
      }
      if *disambiguate_version
        && let Some(ver) = module_package_version(module)
      {
        let ver_slug = sanitize_chunk_segment(ver.as_ref());
        if !ver_slug.is_empty() {
          return Some(format!("{prefix}-{pkg_slug}-{ver_slug}"));
        }
      }
      Some(format!("{prefix}-{pkg_slug}"))
    }
    GroupNamingPolicy::ByDirectory { base_name, prefix, depth } => {
      let rel = module_root_relative_path(module, cwd);
      let rel_str = rel.as_ref().trim_start_matches("./");
      let scoped = if let Some(prefix) = prefix {
        let clean = prefix.trim_start_matches("./").trim_end_matches('/');
        rel_str
          .strip_prefix(clean)
          .and_then(|rest| rest.strip_prefix('/'))
          .unwrap_or(rel_str)
      } else {
        rel_str
      };
      let mut parts: Vec<&str> = scoped.split('/').filter(|s| !s.is_empty()).collect();
      if parts.len() > 1 {
        parts.pop(); // drop filename
      }
      let take_n = usize::from(*depth).max(1).min(parts.len());
      let slug = parts[..take_n]
        .iter()
        .map(|s| sanitize_chunk_segment(s))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
      if slug.is_empty() {
        Some(base_name.clone())
      } else {
        Some(format!("{base_name}-{slug}"))
      }
    }
    GroupNamingPolicy::ByEcosystem { prefix } => {
      let Some(pkg) = module_package_name(module) else {
        return Some(prefix.clone());
      };
      let p = pkg.as_ref();
      let bucket = if matches!(
        p,
        "react"
          | "react-dom"
          | "scheduler"
          | "react-router"
          | "react-router-dom"
          | "use-sync-external-store"
      ) || p.starts_with("@remix-run/")
      {
        "react"
      } else if matches!(p, "zustand" | "redux" | "immer" | "recoil" | "jotai")
        || p.starts_with("@tanstack/")
        || p.starts_with("@reduxjs/")
        || p.starts_with("mobx")
      {
        "state"
      } else if matches!(
        p,
        "lucide-react"
          | "clsx"
          | "tailwind-merge"
          | "class-variance-authority"
          | "framer-motion"
      ) || p.starts_with("@radix-ui/")
        || p.starts_with("@fluentui/")
        || p.starts_with("@mui/")
        || p.starts_with("@emotion/")
      {
        "ui"
      } else {
        return Some(prefix.clone());
      };
      Some(format!("{prefix}-{bucket}"))
    }
  }
}

/// Build a canonical similarity key (`pkg://<name>@<version>/<rel_path>`) for `node_modules`
/// modules so `.pnpm/` prefixes do not distort `stable_id_similarity`.
pub fn module_similarity_key(module: &Module) -> Cow<'_, str> {
  let Some(normal) = module.as_normal() else {
    return Cow::Borrowed(module.stable_id().as_str());
  };
  if !is_vendor_module(normal) {
    return Cow::Borrowed(module.stable_id().as_str());
  }
  if let Some(pkg_json) = normal.originative_resolved_id.package_json.as_ref()
    && let Some(name) = pkg_json.name()
    && !name.is_empty()
  {
    let version = pkg_json.version().unwrap_or("0.0.0");
    let pkg_root = pkg_json
      .realpath()
      .parent()
      .map(|p| p.to_slash_lossy().into_owned())
      .unwrap_or_default();
    let id_slash = normal.id.as_str().replace('\\', "/");
    let rel_path = if !pkg_root.is_empty()
      && let Some(rest) = id_slash.strip_prefix(&pkg_root)
    {
      rest.trim_start_matches('/')
    } else if let Some((_, after_nm)) = id_slash.rsplit_once("node_modules/")
      && let Some(rest) = after_nm.strip_prefix(name)
    {
      rest.trim_start_matches('/')
    } else {
      module.stable_id().as_str()
    };
    return Cow::Owned(format!("pkg://{name}@{version}/{rel_path}"));
  }
  let id_slash = module.stable_id().as_str().replace('\\', "/");
  if let Some((_, after_nm)) = id_slash.rsplit_once("node_modules/") {
    let mut parts = after_nm.splitn(3, '/');
    if let Some(first) = parts.next() {
      if first.starts_with('@') {
        if let Some(second) = parts.next() {
          let rel = parts.next().unwrap_or("");
          return Cow::Owned(format!("pkg://{first}/{second}@0.0.0/{rel}"));
        }
      } else {
        let rest = after_nm.strip_prefix(first).unwrap_or("").trim_start_matches('/');
        return Cow::Owned(format!("pkg://{first}@0.0.0/{rest}"));
      }
    }
  }
  Cow::Borrowed(module.stable_id().as_str())
}

fn find_relevance_split_index(
  modules: &[ModuleIdx],
  module_table: &ModuleTable,
  min_size: f64,
  max_size: f64,
  target_size: Option<f64>,
) -> Option<(usize, f64, f64)> {
  if modules.len() < 2 {
    return None;
  }

  let owned_keys: Vec<Cow<'_, str>> =
    modules.iter().map(|module_idx| module_similarity_key(&module_table[*module_idx])).collect();
  let keys: Vec<&str> = owned_keys.iter().map(AsRef::as_ref).collect();
  let sizes =
    modules.iter().map(|module_idx| module_size(*module_idx, module_table)).collect::<Vec<_>>();
  pick_relevance_split_index_with_target(&keys, &sizes, min_size, max_size, target_size)
}

#[cfg(test)]
fn pick_relevance_split_index(
  keys: &[&str],
  sizes: &[f64],
  min_size: f64,
  max_size: f64,
) -> Option<(usize, f64, f64)> {
  pick_relevance_split_index_with_target(keys, sizes, min_size, max_size, None)
}

pub(crate) fn pick_relevance_split_index_with_target(
  keys: &[&str],
  sizes: &[f64],
  min_size: f64,
  max_size: f64,
  target_size: Option<f64>,
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
  let mut best_target_distance = f64::INFINITY;
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
    let target_distance = target_size.map_or(0.0, |target| {
      (left_size - target).abs().min((right_size - target).abs())
    });

    let is_better = if (best_similarity - similarity).abs() > SIMILARITY_SIGNIFICANCE_THRESHOLD {
      similarity < best_similarity
    } else if oversized_side_count != best_oversized_side_count {
      oversized_side_count < best_oversized_side_count
    } else if target_size.is_some() && (target_distance - best_target_distance).abs() > f64::EPSILON {
      target_distance < best_target_distance
    } else {
      max_side_size < best_max_side_size
    };

    if is_better {
      best_split_index = Some(split_index);
      best_similarity = similarity;
      best_oversized_side_count = oversized_side_count;
      best_target_distance = target_distance;
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
            .and_then(|s| s.to_str())
            .map(ToString::to_string)
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
    concat_string!(group_name, "~", entry_names.join("~"))
  };

  if full_name.len() > MAX_CHUNK_NAME_LEN {
    let hash = xxhash_with_base(full_name.as_bytes(), 36);
    let mut truncate_at = TRUNCATED_LEN;
    while !full_name.is_char_boundary(truncate_at) {
      truncate_at -= 1;
    }
    let truncated = &full_name[..truncate_at];
    ArcStr::from(concat_string!(truncated, "~", &hash[..HASH_DISPLAY_LEN]))
  } else {
    ArcStr::from(full_name)
  }
}

/// `visited` guards one walk against cycles and repeated paths, and nothing more. A set shared
/// across matched modules gave no measured gain. It would also have to belong to one
/// `ModuleGroup`, because a dynamic `name` splits one match group across several groups.
#[expect(clippy::too_many_arguments)]
fn add_module_and_dependencies_to_group_recursively(
  module_group: &mut ModuleGroup,
  seed_module_idx: ModuleIdx,
  module_idx: ModuleIdx,
  module_metas: &LinkingMetadataVec,
  module_table: &ModuleTable,
  module_to_assigned: &IndexBitSet<ModuleIdx>,
  visited: &mut FxHashSet<ModuleIdx>,
  boundary: DependencyBoundary,
  is_mf_shared_group: bool,
  mf_shared_packages: &[String],
) {
  let is_visited = !visited.insert(module_idx);

  if is_visited {
    return;
  }

  let Some(normal_module) = module_table[module_idx].as_normal() else {
    return;
  };

  if module_to_assigned.has_bit(module_idx) {
    return;
  }

  if !module_metas[module_idx].is_included {
    return;
  }

  // Hard Module Federation shared-singleton barrier:
  // - Non-shared groups must never capture or recursively pull modules belonging to federated `shared` packages.
  // - A shared group (`is_mf_shared_group`) must never recursively pull a *different* federated `shared` package.
  if !mf_shared_packages.is_empty()
    && let Some(pkg_name) = module_package_name(normal_module)
    && mf_shared_packages.iter().any(|shared| shared == pkg_name.as_ref())
  {
    if !is_mf_shared_group {
      return;
    }
    if module_idx != seed_module_idx {
      let seed_pkg = module_table[seed_module_idx]
        .as_normal()
        .and_then(module_package_name);
      if seed_pkg.as_deref() != Some(pkg_name.as_ref()) {
        return;
      }
    }
  }

  if module_idx != seed_module_idx {
    match boundary {
      DependencyBoundary::None => return,
      DependencyBoundary::All => {}
      DependencyBoundary::SameOriginKind => {
        let Some(seed_normal) = module_table[seed_module_idx].as_normal() else {
          return;
        };
        if is_vendor_module(seed_normal) != is_vendor_module(normal_module) {
          return;
        }
      }
      DependencyBoundary::SamePackage => {
        let Some(seed_normal) = module_table[seed_module_idx].as_normal() else {
          return;
        };
        let seed_pkg = module_package_name(seed_normal);
        let cur_pkg = module_package_name(normal_module);
        match (seed_pkg, cur_pkg) {
          (Some(a), Some(b)) if a == b => {}
          (None, None) if !is_vendor_module(seed_normal) && !is_vendor_module(normal_module) => {}
          _ => return,
        }
      }
    }
  }

  module_group.add_module(module_idx, module_table);
  if boundary != DependencyBoundary::None {
    for dep in &module_metas[module_idx].dependencies {
      add_module_and_dependencies_to_group_recursively(
        module_group,
        seed_module_idx,
        *dep,
        module_metas,
        module_table,
        module_to_assigned,
        visited,
        boundary,
        is_mf_shared_group,
        mf_shared_packages,
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
        assert_eq!((left, right), expected, "similarities: {similarities:?}");
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

  #[test]
  fn target_size_prefers_split_closer_to_target_within_similarity_tier() {
    let keys = &["pkg://foo@1.0.0/a.js", "pkg://foo@1.0.0/b.js", "pkg://foo@1.0.0/c.js"];
    let sizes = &[20.0, 15.0, 35.0];
    // Without target_size, both splits (index 1: 20/50, index 2: 35/35) have 0 oversized sides
    // at max_size=60, and min(max_side) picks index 2 (35 vs 35).
    // With target_size = Some(20.0), index 1 has left_size = 20.0 (distance 0 from target_size),
    // so it prefers index 1!
    let (split_idx, left_size, right_size) =
      super::pick_relevance_split_index_with_target(keys, sizes, 10.0, 60.0, Some(20.0)).unwrap();
    assert_eq!(split_idx, 1);
    assert_eq!((left_size, right_size), (20.0, 50.0));
  }

  #[test]
  fn pnpm_pkg_similarity_key_splits_at_package_boundary() {
    // With raw `.pnpm/` prefixes, all modules share a long `node_modules/.pnpm/` prefix that
    // obscures the package boundary. With `pkg://` canonical keys, the boundary between
    // `pkg://alpha@1.0.0/...` and `pkg://zeta@2.0.0/...` has similarity 0 and wins cleanly.
    let keys = &[
      "pkg://alpha@1.0.0/dist/a.js",
      "pkg://alpha@1.0.0/dist/b.js",
      "pkg://zeta@2.0.0/dist/c.js",
    ];
    let sizes = &[10.0, 30.0, 20.0];
    let (split_idx, _, _) =
      super::pick_relevance_split_index_with_target(keys, sizes, 5.0, 45.0, None).unwrap();
    assert_eq!(split_idx, 2);
  }

  #[test]
  fn package_pattern_matching_supports_exact_scoped_wildcard_prefix_and_star() {
    use super::package_pattern_matches;
    assert!(package_pattern_matches("*", "react"));
    assert!(package_pattern_matches("*", "@radix-ui/react-dialog"));
    assert!(package_pattern_matches("react", "react"));
    assert!(!package_pattern_matches("react", "react-dom"));
    assert!(package_pattern_matches("react*", "react"));
    assert!(package_pattern_matches("react*", "react-dom"));
    assert!(package_pattern_matches("@radix-ui/*", "@radix-ui/react-dialog"));
    assert!(!package_pattern_matches("@radix-ui/*", "@radix-ui/react-dialog/sub"));
    assert!(!package_pattern_matches("@radix-ui/*", "@fluentui/react-components"));
  }
}
