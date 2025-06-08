use arcstr::ArcStr;
use itertools::Itertools;
use rolldown_common::{Chunk, NormalizedBundlerOptions};
use rolldown_utils::BitSet;

use crate::stages::link_stage::LinkStageOutput;
pub trait ChunkDebugExt {
  fn add_creation_reason(
    &mut self,
    reason: ChunkCreationReason,
    options: &NormalizedBundlerOptions,
  );
}
pub enum ChunkCreationReason<'a> {
  AdvancedChunkGroup(&'a str),
  PreserveModules { is_user_defined_entry: bool, module_stable_id: &'a str },
  Entry { is_user_defined_entry: bool, entry_module_id: &'a str, name: Option<&'a ArcStr> },
  CommonChunk { bits: &'a BitSet, link_output: &'a LinkStageOutput },
}

impl ChunkDebugExt for Chunk {
  fn add_creation_reason(
    &mut self,
    reason: ChunkCreationReason,
    options: &NormalizedBundlerOptions,
  ) {
    if !options.experimental.is_attach_debug_info_enabled() {
      return;
    }

    let reason = match reason {
      ChunkCreationReason::AdvancedChunkGroup(name) => {
        format!("AdvancedChunks: [Group-Name: {name}]")
      }
      ChunkCreationReason::PreserveModules { is_user_defined_entry, module_stable_id } => {
        format!(
          "Enabling Preserve Module: [User-defined: {is_user_defined_entry}] [Module-Id: {module_stable_id}]",
        )
      }
      ChunkCreationReason::Entry {
        is_user_defined_entry,
        entry_module_id: debug_id,
        name: entry_point_name,
      } => {
        if is_user_defined_entry {
          format!("User-defined Entry: [Entry-Module-Id: {debug_id}] [Name: {entry_point_name:?}]",)
        } else {
          format!("Dynamic Entry: [Entry-Module-Id: {debug_id}] [Name: {entry_point_name:?}]",)
        }
      }
      ChunkCreationReason::CommonChunk { bits, link_output } => {
        let entries = link_output
          .entries
          .iter()
          .enumerate()
          .filter_map(|(index, entry_point)| {
            if bits.has_bit(index.try_into().unwrap()) {
              let entry_module = &link_output.module_table[entry_point.id];
              Some(entry_module.stable_id().to_string())
            } else {
              None
            }
          })
          .join(", ");
        format!("Common Chunk: [Shared-By: {entries}]")
      }
    };
    self.create_reasons.push(reason);
  }
}
