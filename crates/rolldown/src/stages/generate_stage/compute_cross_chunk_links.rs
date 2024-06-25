use std::hash::BuildHasherDefault;
use std::{borrow::Cow, sync::Mutex};

use super::GenerateStage;
use crate::chunk_graph::ChunkGraph;
use indexmap::IndexSet;
use itertools::{multizip, Itertools};
use oxc::index::{index_vec, IndexVec};
use rolldown_common::{
  ChunkId, ChunkKind, CrossChunkImportItem, ExportsKind, ExternalModuleId, ImportKind, ModuleId,
  NamedImport, OutputFormat, SymbolRef, WrapKind,
};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::rayon::IntoParallelIterator;
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};

type IndexChunkDependedSymbols = IndexVec<ChunkId, FxHashSet<SymbolRef>>;
type IndexChunkImportsFromExternalModules =
  IndexVec<ChunkId, FxHashMap<ExternalModuleId, Vec<NamedImport>>>;
type IndexChunkExportedSymbols = IndexVec<ChunkId, FxHashSet<SymbolRef>>;
type IndexCrossChunkImports = IndexVec<ChunkId, FxHashSet<ChunkId>>;
type IndexCrossChunkDynamicImports =
  IndexVec<ChunkId, IndexSet<ChunkId, BuildHasherDefault<FxHasher>>>;
type IndexImportsFromOtherChunks = IndexVec<ChunkId, FxHashMap<ChunkId, Vec<CrossChunkImportItem>>>;

impl<'a> GenerateStage<'a> {
  #[allow(clippy::too_many_lines)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn compute_cross_chunk_links(&mut self, chunk_graph: &mut ChunkGraph) {
    let mut index_chunk_depended_symbols: IndexChunkDependedSymbols =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut index_chunk_exported_symbols: IndexChunkExportedSymbols =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut index_chunk_imports_from_external_modules: IndexChunkImportsFromExternalModules = index_vec![FxHashMap::<ExternalModuleId, Vec<NamedImport>>::default(); chunk_graph.chunks.len()];

    let mut index_imports_from_other_chunks: IndexImportsFromOtherChunks = index_vec![FxHashMap::<ChunkId, Vec<CrossChunkImportItem>>::default(); chunk_graph.chunks.len()];
    let mut index_cross_chunk_imports: IndexCrossChunkImports =
      index_vec![FxHashSet::default(); chunk_graph.chunks.len()];
    let mut index_cross_chunk_dynamic_imports: IndexCrossChunkDynamicImports =
      index_vec![IndexSet::default(); chunk_graph.chunks.len()];

    self.collect_depended_symbols(
      chunk_graph,
      &mut index_chunk_depended_symbols,
      &mut index_chunk_imports_from_external_modules,
      &mut index_cross_chunk_dynamic_imports,
    );

    self.compute_chunk_imports(
      chunk_graph,
      &mut index_chunk_depended_symbols,
      &mut index_chunk_exported_symbols,
      &mut index_cross_chunk_imports,
      &mut index_imports_from_other_chunks,
    );

    self.deconflict_exported_names(
      chunk_graph,
      &index_chunk_exported_symbols,
      &mut index_imports_from_other_chunks,
    );

    let index_sorted_cross_chunk_imports = index_cross_chunk_imports
      .into_iter()
      // FIXME: Extra traversing. This is a workaround due to `par_bridge` doesn't ensure order https://github.com/rayon-rs/rayon/issues/551#issuecomment-882069261
      .collect::<Vec<_>>()
      .into_par_iter()
      .map(|cross_chunk_imports| {
        let mut cross_chunk_imports = cross_chunk_imports.into_iter().collect::<Vec<_>>();
        cross_chunk_imports.sort_by_cached_key(|chunk_id| {
          let mut resource_ids = chunk_graph.chunks[*chunk_id]
            .modules
            .iter()
            .map(|id| self.link_output.module_table.normal_modules[*id].resource_id.as_str())
            .collect::<Vec<_>>();
          resource_ids.sort_unstable();
          resource_ids
        });
        cross_chunk_imports
      })
      .collect::<Vec<_>>();

    let index_sorted_imports_from_other_chunks = index_imports_from_other_chunks
      .into_iter_enumerated()
      .collect_vec()
      .into_par_iter()
      .map(|(_chunk_id, importee_map)| {
        importee_map
          .into_iter()
          .sorted_by_key(|(importee_chunk_id, _)| chunk_graph.chunks[*importee_chunk_id].exec_order)
          .collect_vec()
      })
      .collect::<Vec<_>>();

    let index_sorted_imports_from_external_modules = index_chunk_imports_from_external_modules
      .into_iter()
      .map(|imports_from_external_modules| {
        imports_from_external_modules
          .into_iter()
          .sorted_by_key(|(external_module_id, _)| {
            self.link_output.module_table.external_modules[*external_module_id].exec_order
          })
          .collect_vec()
      })
      .collect::<Vec<_>>();

    multizip((
      chunk_graph.chunks.iter_mut(),
      index_sorted_imports_from_other_chunks,
      index_sorted_imports_from_external_modules,
      index_sorted_cross_chunk_imports,
      index_cross_chunk_dynamic_imports,
    ))
    .par_bridge()
    .for_each(
      |(
        chunk,
        sorted_imports_from_other_chunks,
        imports_from_external_modules,
        cross_chunk_imports,
        cross_chunk_dynamic_imports,
      )| {
        chunk.imports_from_other_chunks = sorted_imports_from_other_chunks;
        chunk.imports_from_external_modules = imports_from_external_modules;
        chunk.cross_chunk_imports = cross_chunk_imports;
        chunk.cross_chunk_dynamic_imports =
          cross_chunk_dynamic_imports.into_iter().collect::<Vec<_>>();
      },
    );
  }

  /// - Assign each symbol to the chunk it belongs to
  /// - Collect all referenced symbols and consider them potential imports
  #[allow(clippy::too_many_lines)]
  fn collect_depended_symbols(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    index_chunk_depended_symbols: &mut IndexChunkDependedSymbols,
    index_chunk_imports_from_external_modules: &mut IndexChunkImportsFromExternalModules,
    index_cross_chunk_dynamic_imports: &mut IndexCrossChunkDynamicImports,
  ) {
    let symbols = &Mutex::new(&mut self.link_output.symbols);

    let chunks_iter = multizip((
      chunk_graph.chunks.iter_enumerated(),
      index_chunk_depended_symbols.iter_mut(),
      index_chunk_imports_from_external_modules.iter_mut(),
      index_cross_chunk_dynamic_imports.iter_mut(),
    ));

    chunks_iter.par_bridge().for_each(
      |(
        (chunk_id, chunk),
        depended_symbols,
        imports_from_external_modules,
        cross_chunk_dynamic_imports,
      )| {
        chunk.modules.iter().copied().for_each(|module_id| {
          let module = &self.link_output.module_table.normal_modules[module_id];
          module
            .import_records
            .iter()
            .inspect(|rec| {
              if let ModuleId::Normal(importee_id) = rec.resolved_module {
                let importee_module = &self.link_output.module_table.normal_modules[importee_id];
                // the the resolved module is not included in module graph, skip
                // TODO: Is that possible that the module of the record is a external module?
                if !importee_module.is_included {
                  return;
                }
                if matches!(rec.kind, ImportKind::DynamicImport) {
                  let importee_chunk =
                    chunk_graph.module_to_chunk[importee_id].expect("importee chunk should exist");
                  cross_chunk_dynamic_imports.insert(importee_chunk);
                }
              }
            })
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
            let mut symbols = symbols.lock().expect("ignore poison error");
            stmt_info.declared_symbols.iter().for_each(|declared| {
              let symbol = symbols.get_mut(*declared);
              debug_assert!(
                symbol.chunk_id.unwrap_or(chunk_id) == chunk_id,
                "Symbol: {:?}, {:?} in {:?} should only belong to one chunk",
                symbol.name,
                declared,
                module.resource_id,
              );

              symbol.chunk_id = Some(chunk_id);
            });

            stmt_info.referenced_symbols.iter().for_each(|referenced| {
              let referenced = referenced.symbol_ref();
              let mut canonical_ref = symbols.par_canonical_ref_for(*referenced);
              if let Some(namespace_alias) = &symbols.get(canonical_ref).namespace_alias {
                canonical_ref = namespace_alias.namespace_ref;
              }
              depended_symbols.insert(canonical_ref);
            });
          });
        });

        if let ChunkKind::EntryPoint { module: entry_id, .. } = &chunk.kind {
          let entry = &self.link_output.module_table.normal_modules[*entry_id];
          let entry_meta = &self.link_output.metas[entry.id];

          if !matches!(entry_meta.wrap_kind, WrapKind::Cjs) {
            let symbols = symbols.lock().expect("ignore poison error");

            for export_ref in entry_meta.resolved_exports.values() {
              let mut canonical_ref = symbols.par_canonical_ref_for(export_ref.symbol_ref);
              let symbol = symbols.get(canonical_ref);
              if let Some(ns_alias) = &symbol.namespace_alias {
                canonical_ref = ns_alias.namespace_ref;
              }
              depended_symbols.insert(canonical_ref);
            }
          }

          if !matches!(entry_meta.wrap_kind, WrapKind::None) {
            depended_symbols
              .insert(entry_meta.wrapper_ref.expect("cjs should be wrapped in esm output"));
          }

          if matches!(self.options.format, OutputFormat::Cjs)
            && matches!(entry.exports_kind, ExportsKind::Esm)
          {
            depended_symbols.insert(self.link_output.runtime.resolve_symbol("__toCommonJS"));
            depended_symbols.insert(entry.namespace_object_ref);
          }
        }
      },
    );
  }

  /// - Filter out depended symbols to come from other chunks
  /// - Mark exports of importee chunks
  fn compute_chunk_imports(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    index_chunk_depended_symbols: &mut IndexChunkDependedSymbols,
    index_chunk_exported_symbols: &mut IndexChunkExportedSymbols,
    index_cross_chunk_imports: &mut IndexCrossChunkImports,
    index_imports_from_other_chunks: &mut IndexImportsFromOtherChunks,
  ) {
    chunk_graph.chunks.iter_enumerated().for_each(|(chunk_id, chunk)| {
      let chunk_meta_imports = &index_chunk_depended_symbols[chunk_id];
      for import_ref in chunk_meta_imports.iter().copied() {
        if !self.link_output.used_symbol_refs.contains(&import_ref) {
          continue;
        }
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
          index_cross_chunk_imports[chunk_id].insert(importee_chunk_id);
          let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
          imports_from_other_chunks
            .entry(importee_chunk_id)
            .or_default()
            .push(CrossChunkImportItem { import_ref, export_alias: None });
          index_chunk_exported_symbols[importee_chunk_id].insert(import_ref);
        }
      }

      // If this is an entry point, make sure we import all chunks belonging to this entry point, even if there are no imports. We need to make sure these chunks are evaluated for their side effects too.
      if let ChunkKind::EntryPoint { bit: importer_chunk_bit, .. } = &chunk.kind {
        chunk_graph
          .chunks
          .iter_enumerated()
          .filter(|(id, _)| *id != chunk_id)
          .filter(|(_, importee_chunk)| {
            importee_chunk.bits.has_bit(*importer_chunk_bit)
              && importee_chunk.has_side_effect(self.link_output.runtime.id())
          })
          .for_each(|(importee_chunk_id, _)| {
            let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
            imports_from_other_chunks.entry(importee_chunk_id).or_default();
          });
      }
    });
  }

  fn deconflict_exported_names(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    index_chunk_exported_symbols: &IndexChunkExportedSymbols,
    index_imports_from_other_chunks: &mut IndexImportsFromOtherChunks,
  ) {
    // Generate cross-chunk exports. These must be computed before cross-chunk
    // imports because of export alias renaming, which must consider all export
    // aliases simultaneously to avoid collisions.
    let mut name_count =
      FxHashMap::with_capacity(index_chunk_exported_symbols.iter().map(FxHashSet::len).sum());

    for (chunk_id, chunk) in chunk_graph.chunks.iter_mut_enumerated() {
      for chunk_export in index_chunk_exported_symbols[chunk_id].iter().copied() {
        let original_name: rolldown_rstr::Rstr =
          self.link_output.symbols.get_original_name(chunk_export).to_rstr();
        let key: Cow<'_, Rstr> = Cow::Owned(original_name.clone());
        let count = name_count.entry(key).or_insert(0u32);
        let alias = if *count == 0 {
          original_name.clone()
        } else {
          format!("{original_name}${count}").into()
        };
        chunk.exports_to_other_chunks.insert(chunk_export, alias.clone());
        *count += 1;
      }
    }

    for chunk_id in chunk_graph.chunks.indices() {
      for (importee_chunk_id, import_items) in &mut index_imports_from_other_chunks[chunk_id] {
        for item in import_items {
          if let Some(alias) =
            chunk_graph.chunks[*importee_chunk_id].exports_to_other_chunks.get(&item.import_ref)
          {
            item.export_alias = Some(alias.clone().into());
          }
        }
      }
    }
  }
}
