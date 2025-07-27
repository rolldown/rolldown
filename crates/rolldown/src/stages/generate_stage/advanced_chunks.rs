use std::{cmp::Reverse, sync::Arc};

use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::{
  Chunk, ChunkKind, ChunkingContext, MatchGroupTest, Module, ModuleIdx, ModuleTable,
};
use rolldown_error::BuildResult;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{chunk_graph::ChunkGraph, types::linking_metadata::LinkingMetadataVec};

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
}

oxc_index::define_index_type! {
  pub struct ModuleGroupIdx = u32;
}

impl ModuleGroup {
  #[allow(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here
  pub fn add_module(&mut self, module_idx: ModuleIdx, module_table: &ModuleTable) {
    if self.modules.insert(module_idx) {
      self.sizes += module_table[module_idx].size() as f64;
    }
  }

  #[allow(clippy::cast_precision_loss)] // We consider `usize` to `f64` is safe here
  pub fn remove_module(&mut self, module_idx: ModuleIdx, module_table: &ModuleTable) {
    if self.modules.remove(&module_idx) {
      self.sizes -= module_table[module_idx].size() as f64;
      self.sizes = f64::max(self.sizes, 0.0);
    }
  }
}

impl GenerateStage<'_> {
  #[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
  )] // TODO(hyf0): refactor
  pub async fn apply_advanced_chunks(
    &self,
    index_splitting_info: &IndexSplittingInfo,
    module_to_assigned: &mut IndexVec<ModuleIdx, bool>,
    chunk_graph: &mut ChunkGraph,
    input_base: &ArcStr,
  ) -> BuildResult<()> {
    let Some(chunking_options) = &self.options.advanced_chunks else {
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

    let mut index_module_groups: IndexVec<ModuleGroupIdx, ModuleGroup> = IndexVec::new();
    let mut name_to_module_group: FxHashMap<(usize, ArcStr), ModuleGroupIdx> = FxHashMap::default();

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
          match_group.min_module_size.map_or(chunking_options.min_module_size, Some);
        let allow_max_module_size =
          match_group.max_module_size.map_or(chunking_options.max_module_size, Some);

        let is_min_module_size_satisfied = allow_min_module_size
          .is_none_or(|min_module_size| normal_module.size() >= min_module_size);
        let is_max_module_size_satisfied = allow_max_module_size
          .is_none_or(|max_module_size| normal_module.size() <= max_module_size);

        if !is_min_module_size_satisfied || !is_max_module_size_satisfied {
          continue;
        }

        if let Some(allow_min_share_count) =
          match_group.min_share_count.map_or(chunking_options.min_share_count, Some)
        {
          if splitting_info.share_count < allow_min_share_count {
            continue;
          }
        }

        let ctx = ChunkingContext::new(Arc::clone(&self.plugin_driver.modules));

        let Some(group_name) = match_group.name.value(&ctx, &normal_module.id).await? else {
          // Group which doesn't have a name will be ignored.
          continue;
        };
        let group_name = ArcStr::from(group_name);

        let unique_key = (match_group_index, group_name.clone());

        let module_group_idx = name_to_module_group.entry(unique_key).or_insert_with(|| {
          index_module_groups.push(ModuleGroup {
            modules: FxHashSet::default(),
            match_group_index,
            priority: match_group.priority.unwrap_or(0),
            name: group_name.clone(),
            sizes: 0.0,
          })
        });

        let include_dependencies_recursively =
          chunking_options.include_dependencies_recursively.unwrap_or(true);

        add_module_and_dependencies_to_group_recursively(
          &mut index_module_groups[*module_group_idx],
          normal_module.idx,
          &self.link_output.metas,
          &self.link_output.module_table,
          &mut FxHashSet::default(),
          include_dependencies_recursively,
        );
      }
    }

    let mut module_groups = index_module_groups.raw;
    module_groups.sort_by_cached_key(|item| {
      Reverse((Reverse(item.priority), item.match_group_index, item.name.clone()))
    });
    // - Higher priority group goes first.
    // - If two groups have the same priority, the one with the lower index goes first.
    // - If two groups have the same priority and index, we use dictionary order to sort them.
    // Outer `Reverse` is due to we're gonna use `pop` consume the vector.

    module_groups.retain(|group| !group.modules.is_empty());
    if module_groups.is_empty() {
      // If no module group is found, we just return instead of creating a unnecessary runtime chunk.
      return Ok(());
    }

    // Manually pull out the module `rolldown:runtime` into a standalone chunk.
    let runtime_module_idx = self.link_output.runtime.id();
    let Module::Normal(runtime_module) = &self.link_output.module_table[runtime_module_idx] else {
      unreachable!("`rolldown:runtime` is always a normal module");
    };

    if runtime_module.meta.is_included() {
      let runtime_chunk = Chunk::new(
        Some("rolldown-runtime".into()),
        None,
        index_splitting_info[runtime_module_idx].bits.clone(),
        vec![],
        ChunkKind::Common,
        input_base.clone(),
        None,
      );
      let chunk_idx = chunk_graph.add_chunk(runtime_chunk);
      module_groups.iter_mut().for_each(|group| {
        group.remove_module(runtime_module_idx, &self.link_output.module_table);
      });
      chunk_graph.chunk_table[chunk_idx].bits.union(&index_splitting_info[runtime_module_idx].bits);
      chunk_graph.add_module_to_chunk(
        runtime_module_idx,
        chunk_idx,
        self.link_output.metas[runtime_module_idx].depended_runtime_helper,
      );
      module_to_assigned[runtime_module_idx] = true;
    }

    while let Some(this_module_group) = module_groups.pop() {
      if this_module_group.modules.is_empty() {
        continue;
      }

      let allow_min_size = match_groups[this_module_group.match_group_index]
        .min_size
        .map_or(chunking_options.min_size, Some)
        .unwrap_or(0.0);

      if this_module_group.sizes < allow_min_size {
        continue;
      }

      if let Some(allow_max_size) = match_groups[this_module_group.match_group_index]
        .max_size
        .map_or(chunking_options.max_size, Some)
      {
        if this_module_group.sizes > allow_max_size {
          // If the size of the group is larger than the max size, we should split the group into smaller groups.
          let mut modules = this_module_group.modules.iter().copied().collect::<Vec<_>>();
          modules.sort_by_key(|module_idx| {
            (
              // smaller size goes first
              self.link_output.module_table[*module_idx].size(),
              self.link_output.module_table[*module_idx].stable_id(),
              self.link_output.module_table[*module_idx].exec_order(),
            )
          });
          // Make sure we sort the modules based on size in the end. Since we compute new group size from left to right, if a giant
          // module is at the most left, it may cause a split-able group can't be split.

          let mut left_size = 0f64;
          let mut next_left_index = 0isize;
          let mut right_size = 0f64;
          let mut next_right_index = (modules.len() - 1) as isize;
          let modules_len = modules.len() as isize;

          while left_size < allow_min_size && next_left_index < modules_len {
            left_size +=
              self.link_output.module_table[modules[next_left_index as usize]].size() as f64;
            next_left_index += 1;
          }

          while right_size < allow_min_size && next_right_index >= 0 {
            right_size +=
              self.link_output.module_table[modules[next_right_index as usize]].size() as f64;
            next_right_index -= 1;
          }
          if next_right_index + 1 < next_left_index {
            // For example:
            // [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            //          r^    l^
            // Left contains [0, 1, 2, 3, 4].
            // Right contains [4, 5, 6, 7, 8, 9].
            // There's a overlap [4] in both groups.
            // That is the group can't be split into two groups with both satisfied the min size requirement.
            // In this case, we just ignore the max size requirement and keep the group as a whole.
          } else {
            // TODO: Though, [0..next_left_index] is a valid group, we want to find a best split index that makes files in left group are in the same disk location.
            let mut split_size = left_size;
            loop {
              if next_left_index <= next_right_index && split_size < allow_max_size {
                split_size += self.link_output.module_table.modules
                  [modules[next_left_index as usize]]
                  .size() as f64;
                next_left_index += 1;
              } else {
                break;
              }
            }
            while next_left_index <= next_right_index && next_right_index >= 0 {
              right_size += self.link_output.module_table.modules
                [modules[next_right_index as usize]]
                .size() as f64;
              next_right_index -= 1;
            }

            if next_right_index != -1 && next_left_index != modules_len {
              // - next_right_index == -1
              // - next_left_index == modules.len()
              // They mean that either left or right group is empty, which is not allowed.
              module_groups.push(ModuleGroup {
                name: this_module_group.name.clone(),
                match_group_index: this_module_group.match_group_index,
                modules: modules[..next_left_index as usize].iter().copied().collect(),
                priority: this_module_group.priority,
                sizes: split_size,
              });
              module_groups.push(ModuleGroup {
                name: this_module_group.name.clone(),
                match_group_index: this_module_group.match_group_index,
                modules: modules[next_left_index as usize..].iter().copied().collect(),
                priority: this_module_group.priority,
                sizes: right_size,
              });
              continue;
            }
          }
        }
      }
      let mut chunk = Chunk::new(
        Some(this_module_group.name.clone()),
        None,
        index_splitting_info
          [this_module_group.modules.iter().next().copied().expect("must have one")]
        .bits
        .clone(),
        vec![],
        ChunkKind::Common,
        input_base.clone(),
        None,
      );
      chunk.add_creation_reason(
        ChunkCreationReason::AdvancedChunkGroup(
          &this_module_group.name,
          this_module_group.match_group_index.try_into().unwrap(),
        ),
        self.options,
      );

      let chunk_idx = chunk_graph.add_chunk(chunk);

      this_module_group.modules.iter().copied().for_each(|module_idx| {
        module_groups.iter_mut().for_each(|group| {
          group.remove_module(module_idx, &self.link_output.module_table);
        });
        chunk_graph.chunk_table[chunk_idx].bits.union(&index_splitting_info[module_idx].bits);
        chunk_graph.add_module_to_chunk(
          module_idx,
          chunk_idx,
          self.link_output.metas[module_idx].depended_runtime_helper,
        );
        module_to_assigned[module_idx] = true;
      });
    }
    Ok(())
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

  let Module::Normal(module) = &module_table[module_idx] else {
    return;
  };

  if !module.ecma_view.meta.is_included() {
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
