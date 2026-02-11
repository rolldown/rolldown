use arcstr::ArcStr;
use itertools::Itertools;
use rolldown_common::{Chunk, ChunkDebugInfo, ChunkReasonType, NormalizedBundlerOptions};
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
  ManualCodeSplittingGroup {
    name: &'a str,
    group_index: u32,
    bits: Option<&'a BitSet>,
    link_output: &'a LinkStageOutput,
  },
  PreserveModules {
    is_user_defined_entry: bool,
    module_stable_id: &'a str,
  },
  Entry {
    is_user_defined_entry: bool,
    entry_module_id: &'a str,
    name: Option<&'a ArcStr>,
  },
  CommonChunk {
    bits: &'a BitSet,
    link_output: &'a LinkStageOutput,
  },
}

impl ChunkDebugExt for Chunk {
  fn add_creation_reason(
    &mut self,
    reason: ChunkCreationReason,
    options: &NormalizedBundlerOptions,
  ) {
    match reason {
      ChunkCreationReason::ManualCodeSplittingGroup { group_index, .. } => {
        *self.chunk_reason_type = ChunkReasonType::ManualCodeSplitting { group_index };
      }
      ChunkCreationReason::PreserveModules { .. } => {
        *self.chunk_reason_type = ChunkReasonType::PreserveModules;
      }
      ChunkCreationReason::Entry { .. } => {
        *self.chunk_reason_type = ChunkReasonType::Entry;
      }
      ChunkCreationReason::CommonChunk { .. } => {
        *self.chunk_reason_type = ChunkReasonType::Common;
      }
    }

    if !options.experimental.is_attach_debug_info_full() && !options.devtools {
      return;
    }

    let reason = match reason {
      ChunkCreationReason::ManualCodeSplittingGroup { name, bits, link_output, .. } => {
        let entries_info = bits
          .map(|bits| {
            let entries = resolve_bits_to_entry_names(bits, link_output);
            format!(" [Entries: {entries}]")
          })
          .unwrap_or_default();
        format!("ManualCodeSplitting: [Group-Name: {name}]{entries_info}")
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
        let entries = resolve_bits_to_entry_names(bits, link_output);
        format!("Common Chunk: [Shared-By: {entries}]")
      }
    };

    self.debug_info.push(ChunkDebugInfo::CreateReason(reason));
  }
}

fn resolve_bits_to_entry_names(bits: &BitSet, link_output: &LinkStageOutput) -> String {
  link_output
    .entries
    .iter()
    .flat_map(|(idx, entries)| entries.iter().map(move |_| idx))
    .enumerate()
    .filter_map(|(index, &module_idx)| {
      if bits.has_bit(index.try_into().unwrap()) {
        let entry_module = &link_output.module_table[module_idx];
        Some(entry_module.stable_id().to_string())
      } else {
        None
      }
    })
    .join(", ")
}
