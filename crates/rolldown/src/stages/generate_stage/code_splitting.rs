use std::cmp::Ordering;

use itertools::Itertools;
use oxc::index::IndexVec;
use rolldown_common::{Chunk, ChunkIdx, ChunkKind, Module, ModuleIdx, OutputFormat};
use rolldown_utils::{rustc_hash::FxHashMapExt, BitSet};
use rustc_hash::FxHashMap;

use crate::{chunk_graph::ChunkGraph, type_alias::IndexChunks};

use super::GenerateStage;

impl<'a> GenerateStage<'a> {
  fn determine_reachable_modules_for_entry(
    &self,
    module_id: ModuleIdx,
    entry_index: u32,
    module_to_bits: &mut IndexVec<ModuleIdx, BitSet>,
  ) {
    let Module::Ecma(module) = &self.link_output.module_table.modules[module_id] else {
      return;
    };
    let meta = &self.link_output.metas[module_id];

    if !module.is_included {
      return;
    }

    if module_to_bits[module_id].has_bit(entry_index) {
      return;
    }

    module_to_bits[module_id].set_bit(entry_index);

    meta.dependencies.iter().copied().for_each(|dep_idx| {
      self.determine_reachable_modules_for_entry(dep_idx, entry_index, module_to_bits);
    });

    // Symbols from runtime are referenced by bundler not import statements.
    meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
      let canonical_ref = self.link_output.symbols.par_canonical_ref_for(*symbol_ref);
      self.determine_reachable_modules_for_entry(canonical_ref.owner, entry_index, module_to_bits);
    });

    module.stmt_infos.iter().for_each(|stmt_info| {
      if !stmt_info.is_included {
        return;
      }

      // We need this step to include the runtime module, if there are symbols of it.
      // TODO: Maybe we should push runtime module to `LinkingMetadata::dependencies` while pushing the runtime symbols.
      stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
        match reference_ref {
          rolldown_common::SymbolOrMemberExprRef::Symbol(sym_ref) => {
            let canonical_ref = self.link_output.symbols.par_canonical_ref_for(*sym_ref);
            self.determine_reachable_modules_for_entry(
              canonical_ref.owner,
              entry_index,
              module_to_bits,
            );
          }
          rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
            if let Some(sym_ref) = member_expr.resolved_symbol_ref(&meta.resolved_member_expr_refs)
            {
              let canonical_ref = self.link_output.symbols.par_canonical_ref_for(sym_ref);
              self.determine_reachable_modules_for_entry(
                canonical_ref.owner,
                entry_index,
                module_to_bits,
              );
            } else {
              // `None` means the member expression resolve to a ambiguous export, which means it actually resolve to nothing.
              // It would be rewrite to `undefined` in the final code, so we don't need to include anything to make `undefined` work.
            }
          }
        };
      });
    });
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub fn generate_chunks(&self) -> ChunkGraph {
    if matches!(self.options.format, OutputFormat::Iife) {
      let user_defined_entry_count =
        self.link_output.entries.iter().filter(|entry| entry.kind.is_user_defined()).count();
      debug_assert!(user_defined_entry_count == 1, "IIFE format only supports one entry point");
    }
    let entries_len: u32 =
      self.link_output.entries.len().try_into().expect("Too many entries, u32 overflowed.");
    // If we are in test environment, to make the runtime module always fall into a standalone chunk,
    // we create a facade entry point for it.

    let mut module_to_bits =
      oxc::index::index_vec![BitSet::new(entries_len); self.link_output.module_table.modules.len()];
    let mut bits_to_chunk = FxHashMap::with_capacity(self.link_output.entries.len());
    let mut chunks = IndexChunks::with_capacity(self.link_output.entries.len());
    let mut entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx> =
      FxHashMap::with_capacity(self.link_output.entries.len());
    // Create chunk for each static and dynamic entry
    for (entry_index, entry_point) in self.link_output.entries.iter().enumerate() {
      let count: u32 = entry_index.try_into().expect("Too many entries, u32 overflowed.");
      let mut bits = BitSet::new(entries_len);
      bits.set_bit(count);
      let Module::Ecma(module) = &self.link_output.module_table.modules[entry_point.id] else {
        continue;
      };
      let chunk = chunks.push(Chunk::new(
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
        &mut module_to_bits,
      );
    });

    let mut module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>> = oxc::index::index_vec![
      None;
      self.link_output.module_table.modules.len()
    ];

    // 1. Assign modules to corresponding chunks
    // 2. Create shared chunks to store modules that belong to multiple chunks.
    for normal_module in self.link_output.module_table.modules.iter().filter_map(Module::as_ecma) {
      if !normal_module.is_included {
        continue;
      }

      let bits = &module_to_bits[normal_module.idx];
      debug_assert!(
        !bits.is_empty(),
        "Empty bits means the module is not reachable, so it should bail out with `is_included: false` {:?}", normal_module.stable_id
      );
      if let Some(chunk_id) = bits_to_chunk.get(bits).copied() {
        chunks[chunk_id].modules.push(normal_module.idx);
        module_to_chunk[normal_module.idx] = Some(chunk_id);
      } else {
        let chunk = Chunk::new(None, bits.clone(), vec![normal_module.idx], ChunkKind::Common);
        let chunk_id = chunks.push(chunk);
        module_to_chunk[normal_module.idx] = Some(chunk_id);
        bits_to_chunk.insert(bits.clone(), chunk_id);
      }
    }

    // Sort modules in each chunk by execution order
    chunks.iter_mut().for_each(|chunk| {
      chunk.modules.sort_unstable_by_key(|module_id| {
        self.link_output.module_table.modules[*module_id].exec_order()
      });
    });

    chunks
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
    let sorted_chunk_idx_vec = chunks
      .indices()
      .sorted_unstable_by(|a, b| {
        let a_should_be_first = Ordering::Less;
        let b_should_be_first = Ordering::Greater;

        match (&chunks[*a].kind, &chunks[*b].kind) {
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
          _ => chunks[*a].exec_order.cmp(&chunks[*b].exec_order),
        }
      })
      .collect::<Vec<_>>();

    ChunkGraph { chunks, sorted_chunk_idx_vec, module_to_chunk, entry_module_to_entry_chunk }
  }
}
