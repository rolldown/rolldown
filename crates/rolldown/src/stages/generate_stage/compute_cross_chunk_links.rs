use std::borrow::Cow;
use std::cmp::Reverse;

use super::GenerateStage;
use crate::chunk_graph::ChunkGraph;
use crate::utils::chunk::normalize_preserve_entry_signature;
use itertools::{Itertools, multizip};
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ChunkIdx, ChunkKind, ChunkMeta, CrossChunkImportItem, EntryPointKind, ExportsKind, ImportKind,
  ImportRecordMeta, Module, ModuleIdx, NamedImport, OutputFormat, PreserveEntrySignatures,
  SymbolRef, WrapKind,
};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::concat_string;
use rolldown_utils::hash_placeholder::to_base64;
use rolldown_utils::indexmap::FxIndexSet;
use rolldown_utils::rayon::IntoParallelIterator;
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::{FxHashMap, FxHashSet};

type IndexChunkDependedSymbols = IndexVec<ChunkIdx, FxIndexSet<SymbolRef>>;
type IndexChunkImportsFromExternalModules =
  IndexVec<ChunkIdx, FxHashMap<ModuleIdx, Vec<NamedImport>>>;
type IndexChunkExportedSymbols = IndexVec<ChunkIdx, FxHashMap<SymbolRef, Vec<Rstr>>>;
type IndexCrossChunkImports = IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>;
type IndexCrossChunkDynamicImports = IndexVec<ChunkIdx, FxIndexSet<ChunkIdx>>;
type IndexImportsFromOtherChunks =
  IndexVec<ChunkIdx, FxHashMap<ChunkIdx, Vec<CrossChunkImportItem>>>;

impl GenerateStage<'_> {
  #[allow(clippy::too_many_lines)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn compute_cross_chunk_links(&mut self, chunk_graph: &mut ChunkGraph) {
    let mut index_chunk_depended_symbols: IndexChunkDependedSymbols =
      index_vec![FxIndexSet::<SymbolRef>::default(); chunk_graph.chunk_table.len()];
    let mut index_chunk_exported_symbols: IndexChunkExportedSymbols =
      index_vec![FxHashMap::<SymbolRef, Vec<Rstr>>::default(); chunk_graph.chunk_table.len()];
    let mut index_chunk_imports_from_external_modules: IndexChunkImportsFromExternalModules = index_vec![FxHashMap::<ModuleIdx, Vec<NamedImport>>::default(); chunk_graph.chunk_table.len()];

    let mut index_imports_from_other_chunks: IndexImportsFromOtherChunks = index_vec![FxHashMap::<ChunkIdx, Vec<CrossChunkImportItem>>::default(); chunk_graph.chunk_table.len()];
    let mut index_cross_chunk_imports: IndexCrossChunkImports =
      index_vec![FxHashSet::default(); chunk_graph.chunk_table.len()];
    let mut index_cross_chunk_dynamic_imports: IndexCrossChunkDynamicImports =
      index_vec![FxIndexSet::default(); chunk_graph.chunk_table.len()];

    self.collect_depended_symbols(
      chunk_graph,
      &mut index_chunk_depended_symbols,
      &mut index_chunk_imports_from_external_modules,
      &mut index_cross_chunk_dynamic_imports,
    );

    self.compute_chunk_imports(
      chunk_graph,
      &index_chunk_depended_symbols,
      &mut index_chunk_exported_symbols,
      &mut index_cross_chunk_imports,
      &mut index_imports_from_other_chunks,
    );
    self.deconflict_exported_names(chunk_graph, &index_chunk_exported_symbols);

    let index_sorted_cross_chunk_imports = index_cross_chunk_imports
      .into_par_iter()
      .map(|cross_chunk_imports| {
        let mut cross_chunk_imports = cross_chunk_imports.into_iter().collect::<Vec<_>>();
        cross_chunk_imports.sort_by_cached_key(|chunk_id| {
          let mut module_ids = chunk_graph.chunk_table[*chunk_id]
            .modules
            .iter()
            .map(|id| self.link_output.module_table[*id].id())
            .collect::<Vec<_>>();
          module_ids.sort_unstable();
          module_ids
        });
        cross_chunk_imports
      })
      .collect::<Vec<_>>();

    let index_sorted_imports_from_other_chunks = index_imports_from_other_chunks
      .into_iter()
      .collect_vec()
      .into_par_iter()
      .map(|importee_map| {
        importee_map
          .into_iter()
          .sorted_by_key(|(importee_chunk_id, _)| {
            chunk_graph.chunk_table[*importee_chunk_id].exec_order
          })
          .collect_vec()
      })
      .collect::<Vec<_>>();

    let index_sorted_imports_from_external_modules = index_chunk_imports_from_external_modules
      .into_iter()
      .map(|imports_from_external_modules| {
        imports_from_external_modules
          .into_iter()
          .sorted_by_key(|(external_module_id, _)| {
            self.link_output.module_table[*external_module_id].exec_order()
          })
          .collect_vec()
      })
      .collect::<Vec<_>>();

    multizip((
      chunk_graph.chunk_table.iter_mut(),
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
    chunk_graph: &ChunkGraph,
    index_chunk_depended_symbols: &mut IndexChunkDependedSymbols,
    index_chunk_imports_from_external_modules: &mut IndexChunkImportsFromExternalModules,
    index_cross_chunk_dynamic_imports: &mut IndexCrossChunkDynamicImports,
  ) {
    let symbols = &self.link_output.symbol_db;
    let chunk_id_to_symbols_vec = append_only_vec::AppendOnlyVec::new();

    let chunks_iter = multizip((
      chunk_graph.chunk_table.iter_enumerated(),
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
          let Module::Normal(module) = &self.link_output.module_table[module_id] else {
            return;
          };
          module
            .import_records
            .iter()
            .inspect(|rec| {
              if let Module::Normal(importee_module) =
                &self.link_output.module_table[rec.resolved_module]
              {
                // the the resolved module is not included in module graph, skip
                // TODO: Is that possible that the module of the record is a external module?
                if !importee_module.meta.is_included() {
                  return;
                }
                if matches!(rec.kind, ImportKind::DynamicImport) {
                  let importee_chunk = chunk_graph.module_to_chunk[importee_module.idx]
                    .expect("importee chunk should exist");
                  cross_chunk_dynamic_imports.insert(importee_chunk);
                }
              }
            })
            .filter(|rec| {
              matches!(rec.kind, ImportKind::Import)
                && !rec.meta.contains(ImportRecordMeta::IS_EXPORT_STAR)
            })
            .filter_map(|rec| self.link_output.module_table[rec.resolved_module].as_external())
            .for_each(|importee| {
              // Ensure the external module is imported in case it has side effects.
              imports_from_external_modules.entry(importee.idx).or_default();
            });

          module.named_imports.iter().for_each(|(_, import)| {
            let rec = &module.import_records[import.record_id];
            if let Module::External(importee) = &self.link_output.module_table[rec.resolved_module]
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
                  let mut canonical_ref = symbols.canonical_ref_for(*referenced);
                  if let Some(namespace_alias) = &symbols.get(canonical_ref).namespace_alias {
                    canonical_ref = namespace_alias.namespace_ref;
                  }
                  depended_symbols.insert(canonical_ref);
                }
                rolldown_common::SymbolOrMemberExprRef::MemberExpr(member_expr) => {
                  match member_expr.represent_symbol_ref(
                    &self.link_output.metas[module.idx].resolved_member_expr_refs,
                  ) {
                    Some(sym_ref) => {
                      let mut canonical_ref = self.link_output.symbol_db.canonical_ref_for(sym_ref);
                      let symbol = symbols.get(canonical_ref);
                      if let Some(ref ns_alias) = symbol.namespace_alias {
                        canonical_ref = ns_alias.namespace_ref;
                      }
                      depended_symbols.insert(canonical_ref);
                    }
                    _ => {
                      // `None` means the member expression resolve to a ambiguous export, which means it actually resolve to nothing.
                      // It would be rewrite to `undefined` in the final code, so we don't need to include anything to make `undefined` work.
                    }
                  }
                }
              }
            });
          });
        });

        if let Some(entry_id) = &chunk.entry_module_idx() {
          let entry = &self.link_output.module_table[*entry_id].as_normal().unwrap();
          let entry_meta = &self.link_output.metas[entry.idx];

          if !matches!(entry_meta.wrap_kind, WrapKind::Cjs) {
            for export_ref in entry_meta
              .resolved_exports
              .values()
              // A chunk should always consume a cjs export symbol by property access, so filter
              // out a exported symbol that came from a cjs module.
              .filter(|resolved_export| !resolved_export.came_from_cjs)
            {
              let mut canonical_ref = symbols.canonical_ref_for(export_ref.symbol_ref);
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
    let symbols = &mut self.link_output.symbol_db;
    for (chunk_id, symbol_list) in chunk_id_to_symbols_vec {
      for declared in symbol_list {
        let declared = declared.inner();
        if cfg!(debug_assertions) {
          let symbol_data = symbols.get(declared);
          debug_assert!(
            symbol_data.chunk_id.unwrap_or(chunk_id) == chunk_id,
            "Symbol: {:?}, {:?} in {:?} should only belong to one chunk. Existed {:?}, new {chunk_id:?}",
            declared.name(symbols),
            declared,
            self.link_output.module_table[declared.owner].id(),
            symbol_data.chunk_id,
          );
        }

        let symbol_data = symbols.get_mut(declared);
        symbol_data.chunk_id = Some(chunk_id);
      }
    }
  }

  /// - Filter out depended symbols to come from other chunks
  /// - Mark exports of importee chunks
  #[allow(clippy::too_many_lines)]
  fn compute_chunk_imports(
    &self,
    chunk_graph: &ChunkGraph,
    index_chunk_depended_symbols: &IndexChunkDependedSymbols,
    index_chunk_exported_symbols: &mut IndexChunkExportedSymbols,
    index_cross_chunk_imports: &mut IndexCrossChunkImports,
    index_imports_from_other_chunks: &mut IndexImportsFromOtherChunks,
  ) {
    chunk_graph.chunk_table.iter_enumerated().for_each(|(chunk_id, chunk)| {
      match chunk.kind {
        ChunkKind::EntryPoint { module: module_idx, meta, .. } => {
          let is_dynamic_imported = meta.contains(ChunkMeta::DynamicImported);
          let is_user_defined = meta.contains(ChunkMeta::UserDefinedEntry);

          let normalized_entry_signatures = normalize_preserve_entry_signature(
            &self.link_output.overrode_preserve_entry_signature_map,
            self.options,
            module_idx,
          );
          let needs_export_entry_signatures = if self.options.preserve_modules {
            if is_user_defined {
              !matches!(normalized_entry_signatures, PreserveEntrySignatures::False)
            } else {
              is_dynamic_imported
            }
          } else {
            is_dynamic_imported
              || !matches!(normalized_entry_signatures, PreserveEntrySignatures::False)
          };
          #[allow(clippy::nonminimal_bool)]
          if needs_export_entry_signatures {
            // If the entry point is external, we don't need to compute exports.
            let meta = &self.link_output.metas[module_idx];
            for (name, symbol) in meta
              .referenced_canonical_exports_symbols(
                module_idx,
                if is_user_defined {
                  EntryPointKind::UserDefined
                } else {
                  EntryPointKind::DynamicImport
                },
                &self.link_output.dynamic_import_exports_usage_map,
                false,
              )
              .map(|(name, export)| (name, export.symbol_ref))
            {
              index_chunk_exported_symbols[chunk_id].entry(symbol).or_default().push(name.clone());
            }
          }
        }
        ChunkKind::Common => {}
      }

      let chunk_meta_imports = &index_chunk_depended_symbols[chunk_id];
      for import_ref in chunk_meta_imports.iter().copied() {
        if !self.link_output.used_symbol_refs.contains(&import_ref) {
          continue;
        }
        // If the symbol from external, we don't need to include it.
        if self.link_output.module_table[import_ref.owner].is_external() {
          continue;
        }
        let import_symbol = self.link_output.symbol_db.get(import_ref);
        let importee_chunk_id = import_symbol.chunk_id.unwrap_or_else(|| {
          let symbol_owner = &self.link_output.module_table[import_ref.owner];
          let symbol_name = import_ref.name(&self.link_output.symbol_db);
          panic!("Symbol {:?} in {:?} should belong to a chunk", symbol_name, symbol_owner.id())
        });
        // Check if the import is from another chunk
        if chunk_id != importee_chunk_id {
          index_cross_chunk_imports[chunk_id].insert(importee_chunk_id);
          let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
          imports_from_other_chunks
            .entry(importee_chunk_id)
            .or_default()
            .push(CrossChunkImportItem { import_ref });
          index_chunk_exported_symbols[importee_chunk_id].entry(import_ref).or_default();
        }
      }

      // If this is an entry point, make sure we import all chunks belonging to this entry point, even if there are no imports. We need to make sure these chunks are evaluated for their side effects too.
      if let ChunkKind::EntryPoint { bit: importer_chunk_bit, .. } = &chunk.kind {
        if self.options.preserve_modules {
          let entry_module =
            chunk.entry_module(&self.link_output.module_table).expect("Should have entry module");
          entry_module
            .import_records
            .iter()
            .filter(|rec| rec.kind != ImportKind::DynamicImport)
            .for_each(|item| {
              if !self.link_output.module_table[item.resolved_module]
                .side_effects()
                .has_side_effects()
              {
                return;
              }
              let Some(importee_chunk_idx) = chunk_graph.module_to_chunk[item.resolved_module]
              else {
                return;
              };
              index_cross_chunk_imports[chunk_id].insert(importee_chunk_idx);
              let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
              imports_from_other_chunks.entry(importee_chunk_idx).or_default();
            });
        } else {
          chunk_graph
            .chunk_table
            .iter_enumerated()
            .filter(|(id, _)| *id != chunk_id)
            .filter(|(_, importee_chunk)| {
              if self.options.experimental.is_strict_execution_order_enabled() {
                // With strict_execution_order/wrapping, modules aren't executed in loading but on-demand.
                // So we don't need to do plain imports to address the side effects. It would be ensured
                // by those `init_xxx()` calls.
                false
              } else {
                importee_chunk.bits.has_bit(*importer_chunk_bit)
                  && importee_chunk.has_side_effect(self.link_output.runtime.id())
              }
            })
            .for_each(|(importee_chunk_id, _)| {
              index_cross_chunk_imports[chunk_id].insert(importee_chunk_id);
              let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
              imports_from_other_chunks.entry(importee_chunk_id).or_default();
            });
        }
      }
    });
  }

  fn deconflict_exported_names(
    &self,
    chunk_graph: &mut ChunkGraph,
    index_chunk_exported_symbols: &IndexChunkExportedSymbols,
  ) {
    let is_preserve_modules_enabled = self.options.preserve_modules;
    let allow_to_minify_internal_exports =
      !is_preserve_modules_enabled && self.options.minify_internal_exports;

    // Generate cross-chunk exports. These must be computed before cross-chunk
    // imports because of export alias renaming, which must consider all export
    // aliases simultaneously to avoid collisions.
    for (chunk_id, chunk) in chunk_graph.chunk_table.iter_mut_enumerated() {
      if allow_to_minify_internal_exports {
        // Reference: https://github.com/rollup/rollup/blob/f76339428586620ff3e4c32fce48f923e7be7b05/src/utils/exportNames.ts#L5
        let mut named_index = 0;
        let mut used_names = FxHashSet::default();

        let mut processed_entry_exports = FxHashSet::default();
        if let Some(entry_module_idx) = chunk.entry_module_idx() {
          // If this's an entry point, we need to make sure the entry modules' exports are not minified.
          let entry_module = &self.link_output.metas[entry_module_idx];
          entry_module.canonical_exports(false).for_each(|(name, export)| {
            let export_ref = self.link_output.symbol_db.canonical_ref_for(export.symbol_ref);
            if !self.link_output.used_symbol_refs.contains(&export_ref) {
              // Rolldown supports tree-shaking on dynamic entries, so not all exports are used.
              return;
            }
            used_names.insert(name.clone());
            chunk.exports_to_other_chunks.entry(export_ref).or_default().push(name.clone());
            processed_entry_exports.insert(export_ref);
          });
        }
        for (chunk_export, _predefined_names) in index_chunk_exported_symbols[chunk_id]
          .iter()
          .sorted_unstable_by_key(|(symbol_ref, _predefined_names)| {
            let symbol_owner = &self.link_output.module_table[symbol_ref.owner];
            let symbol_name = symbol_ref.name(&self.link_output.symbol_db);
            // `Reverse(symbol_owner.exec_order()` is used to follow the same deconflict order as in
            // https://github.com/rolldown/rolldown/blob/504ea76c00563eb7db7a49c2b6e04b2fbe61bdc1/crates/rolldown/src/utils/chunk/deconflict_chunk_symbols.rs?plain=1#L86-L102
            // Then we sort by the symbol name to ensure a stable order within the same module.
            (Reverse(symbol_owner.exec_order()), symbol_name)
          })
        {
          let export_ref = self.link_output.symbol_db.canonical_ref_for(*chunk_export);
          if processed_entry_exports.contains(&export_ref) {
            continue;
          }

          let mut export_name: Rstr;
          loop {
            named_index += 1;
            export_name = to_base64(named_index).into();
            if export_name.starts_with('1') {
              named_index += 9 * 64u32.pow(u32::try_from(export_name.len() - 1).unwrap());
              continue;
            }
            if !used_names.contains(&export_name) {
              break;
            }
          }
          used_names.insert(export_name.clone());
          chunk.exports_to_other_chunks.entry(export_ref).or_default().push(export_name);
        }

        continue;
      }

      let mut name_count = FxHashMap::with_capacity(index_chunk_exported_symbols[chunk_id].len());
      for (chunk_export, predefined_names) in index_chunk_exported_symbols[chunk_id]
        .iter()
        .sorted_by_cached_key(|(symbol_ref, _predefined_names)| {
          // same deconflict order in deconflict_chunk_symbols.rs
          // https://github.com/rolldown/rolldown/blob/504ea76c00563eb7db7a49c2b6e04b2fbe61bdc1/crates/rolldown/src/utils/chunk/deconflict_chunk_symbols.rs?plain=1#L86-L102
          Reverse::<u32>(self.link_output.module_table[symbol_ref.owner].exec_order())
        })
      {
        let original_name: rolldown_rstr::Rstr = match predefined_names.as_slice() {
          [] => chunk_export.name(&self.link_output.symbol_db).to_rstr(),
          lst => {
            for item in lst {
              name_count.insert(Cow::Borrowed(item), 0);
            }

            chunk.exports_to_other_chunks.entry(*chunk_export).or_default().extend_from_slice(lst);
            continue;
          }
        };
        // A special case for `default` export when setting `preserve_modules`.
        // When `preserve_modules` is enabled, we need to ensure that the default export is
        // correctly named as `default`.
        // Otherwise, we just use the default_export_ref representative name
        let mut candidate_name = if self.options.preserve_modules {
          let module = chunk.entry_module(&self.link_output.module_table).unwrap();
          // If `preserve_modules` is enabled, there should have only one default export per chunk.
          if module.default_export_ref == *chunk_export {
            "default".into()
          } else {
            original_name.clone()
          }
        } else {
          original_name.clone()
        };
        loop {
          let key: Cow<'_, Rstr> = Cow::Owned(candidate_name.clone());
          match name_count.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut occ) => {
              let next_conflict_index = *occ.get() + 1;
              *occ.get_mut() = next_conflict_index;
              candidate_name =
                concat_string!(original_name, "$", itoa::Buffer::new().format(next_conflict_index))
                  .into();
            }
            std::collections::hash_map::Entry::Vacant(vac) => {
              vac.insert(0);
              break;
            }
          }
        }
        chunk.exports_to_other_chunks.entry(*chunk_export).or_default().push(candidate_name);
      }
    }
  }
}
