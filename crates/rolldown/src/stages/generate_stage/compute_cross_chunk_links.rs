use std::borrow::Cow;
use std::hash::BuildHasherDefault;

use super::GenerateStage;
use crate::chunk_graph::ChunkGraph;
use indexmap::IndexSet;
use itertools::{multizip, Itertools};
use oxc::index::{index_vec, IndexVec};
use rolldown_common::{
  ChunkIdx, ChunkKind, CrossChunkImportItem, ExportsKind, ImportKind, Module, ModuleIdx,
  NamedImport, OutputFormat, SymbolRef, WrapKind,
};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::rayon::IntoParallelIterator;
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::{FxHashMap, FxHashSet, FxHasher};

type IndexChunkDependedSymbols = IndexVec<ChunkIdx, FxHashSet<SymbolRef>>;
type IndexChunkImportsFromExternalModules =
  IndexVec<ChunkIdx, FxHashMap<ModuleIdx, Vec<NamedImport>>>;
type IndexChunkExportedSymbols = IndexVec<ChunkIdx, FxHashSet<SymbolRef>>;
type IndexCrossChunkImports = IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>;
type IndexCrossChunkDynamicImports =
  IndexVec<ChunkIdx, IndexSet<ChunkIdx, BuildHasherDefault<FxHasher>>>;
type IndexImportsFromOtherChunks =
  IndexVec<ChunkIdx, FxHashMap<ChunkIdx, Vec<CrossChunkImportItem>>>;

impl<'a> GenerateStage<'a> {
  #[allow(clippy::too_many_lines)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn compute_cross_chunk_links(&mut self, chunk_graph: &mut ChunkGraph) {
    let mut index_chunk_depended_symbols: IndexChunkDependedSymbols =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut index_chunk_exported_symbols: IndexChunkExportedSymbols =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut index_chunk_imports_from_external_modules: IndexChunkImportsFromExternalModules =
      index_vec![FxHashMap::<ModuleIdx, Vec<NamedImport>>::default(); chunk_graph.chunks.len()];

    let mut index_imports_from_other_chunks: IndexImportsFromOtherChunks = index_vec![FxHashMap::<ChunkIdx, Vec<CrossChunkImportItem>>::default(); chunk_graph.chunks.len()];
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

    // TODO: support rayon trait in `oxc_index`
    let index_sorted_cross_chunk_imports = index_cross_chunk_imports
      .raw
      .into_par_iter()
      .map(|cross_chunk_imports| {
        let mut cross_chunk_imports = cross_chunk_imports.into_iter().collect::<Vec<_>>();
        cross_chunk_imports.sort_by_cached_key(|chunk_id| {
          let mut module_ids = chunk_graph.chunks[*chunk_id]
            .modules
            .iter()
            .map(|id| self.link_output.module_table.modules[*id].id())
            .collect::<Vec<_>>();
          module_ids.sort_unstable();
          module_ids
        });
        cross_chunk_imports
      })
      .collect::<Vec<_>>();

    index_imports_from_other_chunks.iter_enumerated().for_each(|(chunk_id, importee_map)| {
      chunk_graph.chunks[chunk_id].require_binding_names_for_other_chunks = importee_map
        .keys()
        .map(|id| {
          let chunk = &chunk_graph.chunks[*id];
          (*id, chunk.name.clone().unwrap().to_string())
        })
        .collect();
    });

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
            self.link_output.module_table.modules[*external_module_id].exec_order()
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
    let symbols = &self.link_output.symbols;
    let chunk_id_to_symbols_vec = append_only_vec::AppendOnlyVec::new();

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
        let mut symbol_needs_to_assign = vec![];
        chunk.modules.iter().copied().for_each(|module_id| {
          let Module::Ecma(module) = &self.link_output.module_table.modules[module_id] else {
            return;
          };
          module
            .import_records
            .iter()
            .inspect(|rec| {
              if let Module::Ecma(importee_module) =
                &self.link_output.module_table.modules[rec.resolved_module]
              {
                // the the resolved module is not included in module graph, skip
                // TODO: Is that possible that the module of the record is a external module?
                if !importee_module.is_included {
                  return;
                }
                if matches!(rec.kind, ImportKind::DynamicImport) {
                  let importee_chunk = chunk_graph.module_to_chunk[importee_module.idx]
                    .expect("importee chunk should exist");
                  cross_chunk_dynamic_imports.insert(importee_chunk);
                }
              }
            })
            .filter(|rec| matches!(rec.kind, ImportKind::Import))
            .filter_map(|rec| {
              self.link_output.module_table.modules[rec.resolved_module].as_external()
            })
            .for_each(|importee| {
              // Ensure the external module is imported in case it has side effects.
              imports_from_external_modules.entry(importee.idx).or_default();
            });

          module.named_imports.iter().for_each(|(_, import)| {
            let rec = &module.import_records[import.record_id];
            if let Module::External(importee) =
              &self.link_output.module_table.modules[rec.resolved_module]
            {
              imports_from_external_modules.entry(importee.idx).or_default().push(import.clone());
            }
          });

          module.stmt_infos.iter().for_each(|stmt_info| {
            if !stmt_info.is_included {
              return;
            }
            stmt_info.declared_symbols.iter().for_each(|declared| {
              symbol_needs_to_assign.push(*declared);
            });

            stmt_info.referenced_symbols.iter().for_each(|reference_ref| {
              match reference_ref {
                rolldown_common::SymbolOrMemberExprRef::Symbol(referenced) => {
                  let mut canonical_ref = symbols.par_canonical_ref_for(*referenced);
                  if let Some(namespace_alias) = &symbols.get(canonical_ref).namespace_alias {
                    canonical_ref = namespace_alias.namespace_ref;
                  }
                  depended_symbols.insert(canonical_ref);
                }
                rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
                  if let Some(sym_ref) = member_expr.resolved_symbol_ref(
                    &self.link_output.metas[module.idx].resolved_member_expr_refs,
                  ) {
                    let canonical_ref = self.link_output.symbols.par_canonical_ref_for(sym_ref);
                    depended_symbols.insert(canonical_ref);
                  } else {
                    // `None` means the member expression resolve to a ambiguous export, which means it actually resolve to nothing.
                    // It would be rewrite to `undefined` in the final code, so we don't need to include anything to make `undefined` work.
                  }
                }
              };
            });
          });
        });

        if let ChunkKind::EntryPoint { module: entry_id, .. } = &chunk.kind {
          let entry = &self.link_output.module_table.modules[*entry_id].as_ecma().unwrap();
          let entry_meta = &self.link_output.metas[entry.idx];

          if !matches!(entry_meta.wrap_kind, WrapKind::Cjs) {
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
        chunk_id_to_symbols_vec.push((chunk_id, symbol_needs_to_assign));
      },
    );
    // shadowing previous immutable borrow
    let symbols = &mut self.link_output.symbols;
    for (chunk_id, symbol_list) in chunk_id_to_symbols_vec {
      for declared in symbol_list {
        let symbol = symbols.get_mut(declared);
        debug_assert!(
          symbol.chunk_id.unwrap_or(chunk_id) == chunk_id,
          "Symbol: {:?}, {:?} in {:?} should only belong to one chunk",
          symbol.name,
          declared,
          self.link_output.module_table.modules[declared.owner].id(),
        );
        symbol.chunk_id = Some(chunk_id);
      }
    }
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
          let symbol_owner = &self.link_output.module_table.modules[import_ref.owner];
          let symbol_name = self.link_output.symbols.get_original_name(import_ref);
          panic!("Symbol {:?} in {:?} should belong to a chunk", symbol_name, symbol_owner.id())
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
            index_cross_chunk_imports[chunk_id].insert(importee_chunk_id);
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
