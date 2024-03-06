use std::{borrow::Cow, ptr::addr_of};

use crate::{
  OutputFormat,
  {
    chunk::chunk::{ChunkKind, CrossChunkImportItem},
    chunk_graph::ChunkGraph,
    utils::is_in_rust_test_mode,
  },
};

use super::BundleStage;
use index_vec::{index_vec, IndexVec};
use rayon::iter::{ParallelBridge, ParallelIterator};
use rolldown_common::{
  ChunkId, ExportsKind, ExternalModuleId, ImportKind, ModuleId, NamedImport, SymbolRef,
};
use rolldown_rstr::{Rstr, ToRstr};
use rustc_hash::{FxHashMap, FxHashSet};

type ChunkMetaImports = IndexVec<ChunkId, FxHashSet<SymbolRef>>;
type ChunkMetaImportsForExternalModules =
  IndexVec<ChunkId, FxHashMap<ExternalModuleId, Vec<NamedImport>>>;
type ChunkMetaExports = IndexVec<ChunkId, FxHashSet<SymbolRef>>;

impl<'a> BundleStage<'a> {
  /// - Assign each symbol to the chunk it belongs to
  /// - Collect all referenced symbols and consider them potential imports
  fn collect_potential_chunk_imports(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    chunk_meta_imports: &mut ChunkMetaImports,
    chunk_meta_imports_from_external_modules: &mut ChunkMetaImportsForExternalModules,
  ) {
    let symbols = &self.link_output.symbols;

    chunk_graph
      .chunks
      .iter_enumerated()
      .zip(chunk_meta_imports.iter_mut().zip(chunk_meta_imports_from_external_modules.iter_mut()))
      .par_bridge()
      .for_each(|((chunk_id, chunk), (chunk_meta_imports, imports_from_external_modules))| {
        chunk.modules.iter().copied().for_each(|module_id| {
          let module = &self.link_output.module_table.normal_modules[module_id];
          module
            .import_records
            .iter()
            .filter(|rec| matches!(rec.kind, ImportKind::Import))
            .filter_map(|rec| {
              rec
                .resolved_module
                .as_external()
                .map(|id| &self.link_output.module_table.external_modules[id])
            })
            .for_each(|importee| {
              // Ensure the external module is imported in case it has side effects.
              imports_from_external_modules.entry(importee.id).or_default();
            });

          module.named_imports.iter().for_each(|(_, import)| {
            let rec = &module.import_records[import.record_id];
            if let ModuleId::External(importee_id) = rec.resolved_module {
              imports_from_external_modules.entry(importee_id).or_default().push(import.clone());
            }
          });

          module.stmt_infos.iter().for_each(|stmt_info| {
            if !stmt_info.is_included {
              return;
            }

            stmt_info.declared_symbols.iter().for_each(|declared| {
              let symbol = symbols.get(*declared);
              debug_assert!(
                symbol.chunk_id.unwrap_or(chunk_id) == chunk_id,
                "Symbol: {:?}, {:?} in {:?} should only belong to one chunk",
                symbol.name,
                declared,
                module.resource_id,
              );

              // safety: No two threads are ever writing to the same location
              unsafe {
                (*addr_of!(*symbols).cast_mut()).get_mut(*declared).chunk_id = Some(chunk_id);
              }
            });

            stmt_info.referenced_symbols.iter().for_each(|referenced| {
              let mut canonical_ref = self.link_output.symbols.par_canonical_ref_for(*referenced);
              if let Some(namespace_alias) = &symbols.get(canonical_ref).namespace_alias {
                canonical_ref = namespace_alias.namespace_ref;
              }
              chunk_meta_imports.insert(canonical_ref);
            });
          });
        });

        if let ChunkKind::EntryPoint { module: entry_module_id, .. } = &chunk.kind {
          let entry_module = &self.link_output.module_table.normal_modules[*entry_module_id];
          let entry_linking_info = &self.link_output.metas[entry_module.id];
          if matches!(entry_module.exports_kind, ExportsKind::CommonJs)
            && matches!(self.output_options.format, OutputFormat::Esm)
          {
            chunk_meta_imports
              .insert(entry_linking_info.wrapper_ref.expect("cjs should be wrapped in esm output"));
          }
          for export_ref in entry_linking_info.resolved_exports.values() {
            let mut canonical_ref =
              self.link_output.symbols.par_canonical_ref_for(export_ref.symbol_ref);
            let symbol = symbols.get(canonical_ref);
            if let Some(ns_alias) = &symbol.namespace_alias {
              canonical_ref = ns_alias.namespace_ref;
            }
            chunk_meta_imports.insert(canonical_ref);
          }
        }
      });
  }

  pub fn compute_cross_chunk_links(&mut self, chunk_graph: &mut ChunkGraph) {
    let mut chunk_meta_imports_vec: ChunkMetaImports =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut chunk_meta_exports_vec: ChunkMetaExports =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut chunk_meta_imports_from_external_modules_vec: ChunkMetaImportsForExternalModules = index_vec![FxHashMap::<ExternalModuleId, Vec<NamedImport>>::default(); chunk_graph.chunks.len()];

    self.collect_potential_chunk_imports(
      chunk_graph,
      &mut chunk_meta_imports_vec,
      &mut chunk_meta_imports_from_external_modules_vec,
    );

    // - Find out what imports are actually come from other chunks
    chunk_graph.chunks.iter_enumerated().for_each(|(chunk_id, chunk)| {
      let chunk_meta_imports = &chunk_meta_imports_vec[chunk_id];
      for import_ref in chunk_meta_imports.iter().copied() {
        let import_symbol = self.link_output.symbols.get(import_ref);

        let importee_chunk_id = import_symbol.chunk_id.unwrap_or_else(|| {
          let symbol_owner = &self.link_output.module_table.normal_modules[import_ref.owner];
          let symbol_name = self.link_output.symbols.get_original_name(import_ref);
          panic!(
            "Symbol {:?} in {:?} should belong to a chunk",
            symbol_name, symbol_owner.resource_id
          )
        });
        // Check if the import is from another chunk
        if chunk_id != importee_chunk_id {
          let imports_from_other_chunks =
          // Safety: No race condition here:
          // - This loop is executed sequentially in a single thread
            unsafe { &mut (*addr_of!(chunk.imports_from_other_chunks).cast_mut()) };
          imports_from_other_chunks
            .entry(importee_chunk_id)
            .or_default()
            .push(CrossChunkImportItem { import_ref, export_alias: None });
          chunk_meta_exports_vec[importee_chunk_id].insert(import_ref);
        }
      }

      // If this is an entry point, make sure we import all chunks belonging to
      // this entry point, even if there are no imports. We need to make sure
      // these chunks are evaluated for their side effects too.
      if let ChunkKind::EntryPoint { bit: importer_chunk_bit, .. } = &chunk.kind {
        chunk_graph
          .chunks
          .iter_enumerated()
          .filter(|(id, _)| *id != chunk_id)
          .filter(|(_, importee_chunk)| importee_chunk.bits.has_bit(*importer_chunk_bit))
          .filter(|(_, importee_chunk)| {
            // If we are in test environment, to get a cleaner output in snapshot, no need to import
            // the runtime chunk as it for sure has no side effects.
            is_in_rust_test_mode()
              && importee_chunk.modules.first().copied() != Some(self.link_output.runtime.id())
          })
          .for_each(|(importee_chunk_id, _)| {
            let imports_from_other_chunks =
              unsafe { &mut (*addr_of!(chunk.imports_from_other_chunks).cast_mut()) };
            imports_from_other_chunks.entry(importee_chunk_id).or_default();
          });
      }
    });

    // Generate cross-chunk exports. These must be computed before cross-chunk
    // imports because of export alias renaming, which must consider all export
    // aliases simultaneously to avoid collisions.
    let mut name_count = FxHashMap::default();
    for (chunk_id, chunk) in chunk_graph.chunks.iter_mut_enumerated() {
      for export in chunk_meta_exports_vec[chunk_id].iter().copied() {
        let original_name: rolldown_rstr::Rstr =
          self.link_output.symbols.get_original_name(export).to_rstr();
        let key: Cow<'_, Rstr> = Cow::Owned(original_name.clone());
        let count = name_count.entry(key).or_insert(0u32);
        let alias = if *count == 0 {
          original_name.clone()
        } else {
          format!("{original_name}${count}").into()
        };
        chunk.exports_to_other_chunks.insert(export, alias.clone());
        *count += 1;
      }
    }
    for chunk_id in chunk_graph.chunks.indices() {
      for (importee_chunk_id, import_items) in
        &chunk_graph.chunks[chunk_id].imports_from_other_chunks
      {
        for item in import_items {
          if let Some(alias) =
            chunk_graph.chunks[*importee_chunk_id].exports_to_other_chunks.get(&item.import_ref)
          {
            // safety: no other mutable reference to `item` exists
            unsafe {
              let item = (item as *const CrossChunkImportItem).cast_mut();
              (*item).export_alias = Some(alias.clone().into());
            }
          }
        }
      }
    }

    chunk_meta_imports_from_external_modules_vec.into_iter_enumerated().for_each(
      |(chunk_id, imports_from_external_modules)| {
        chunk_graph.chunks[chunk_id].imports_from_external_modules = imports_from_external_modules;
      },
    );
  }
}
