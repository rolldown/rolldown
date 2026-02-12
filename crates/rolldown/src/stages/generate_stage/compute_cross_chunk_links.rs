use std::borrow::Cow;
use std::cmp::Reverse;

use super::GenerateStage;
use crate::chunk_graph::ChunkGraph;
use crate::utils::chunk::normalize_preserve_entry_signature;
use itertools::{Itertools, multizip};
use oxc::semantic::SymbolId;
use oxc::span::CompactStr;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ChunkIdx, ChunkKind, ChunkMeta, CrossChunkImportItem, EntryPointKind, ExportsKind, ImportKind,
  ImportRecordMeta, Module, ModuleIdx, NamedImport, OutputFormat, PostChunkOptimizationOperation,
  PreserveEntrySignatures, RUNTIME_HELPER_NAMES, SymbolIdExt, SymbolRef, WrapKind,
};
use rolldown_utils::concat_string;
use rolldown_utils::index_vec_ext::IndexVecRefExt as _;
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::{FxHashMap, FxHashSet};

type IndexChunkDependedSymbols = IndexVec<ChunkIdx, FxIndexSet<SymbolRef>>;
type IndexChunkImportsFromExternalModules =
  IndexVec<ChunkIdx, FxHashMap<ModuleIdx, Vec<(ModuleIdx, NamedImport)>>>;
type IndexChunkAllImportsFromExternalModules = IndexVec<ChunkIdx, FxIndexSet<ModuleIdx>>;
type IndexChunkExportedSymbols = IndexVec<ChunkIdx, FxHashMap<SymbolRef, Vec<CompactStr>>>;
type IndexCrossChunkImports = IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>;
type IndexCrossChunkDynamicImports = IndexVec<ChunkIdx, FxIndexSet<ChunkIdx>>;
type IndexImportsFromOtherChunks =
  IndexVec<ChunkIdx, FxHashMap<ChunkIdx, Vec<CrossChunkImportItem>>>;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn compute_cross_chunk_links(&mut self, chunk_graph: &mut ChunkGraph) {
    let mut index_chunk_depended_symbols: IndexChunkDependedSymbols =
      index_vec![FxIndexSet::<SymbolRef>::default(); chunk_graph.chunk_table.len()];
    let mut index_chunk_exported_symbols: IndexChunkExportedSymbols =
      index_vec![FxHashMap::<SymbolRef, Vec<CompactStr>>::default(); chunk_graph.chunk_table.len()];
    let mut index_chunk_direct_imports_from_external_modules: IndexChunkImportsFromExternalModules = index_vec![FxHashMap::<ModuleIdx, Vec<(ModuleIdx, NamedImport)>>::default(); chunk_graph.chunk_table.len()];
    // Used for cjs,umd,iife only
    let mut index_chunk_indirect_imports_from_external_modules: IndexChunkAllImportsFromExternalModules =
      index_vec![FxIndexSet::<ModuleIdx>::default(); chunk_graph.chunk_table.len()];

    let mut index_imports_from_other_chunks: IndexImportsFromOtherChunks = index_vec![FxHashMap::<ChunkIdx, Vec<CrossChunkImportItem>>::default(); chunk_graph.chunk_table.len()];
    let mut index_cross_chunk_imports: IndexCrossChunkImports =
      index_vec![FxHashSet::default(); chunk_graph.chunk_table.len()];
    let mut index_cross_chunk_dynamic_imports: IndexCrossChunkDynamicImports =
      index_vec![FxIndexSet::default(); chunk_graph.chunk_table.len()];

    self.collect_depended_symbols(
      chunk_graph,
      &mut index_chunk_depended_symbols,
      &mut index_chunk_direct_imports_from_external_modules,
      &mut index_cross_chunk_dynamic_imports,
    );

    self.compute_chunk_imports(
      chunk_graph,
      &index_chunk_depended_symbols,
      &mut index_chunk_exported_symbols,
      &mut index_cross_chunk_imports,
      &mut index_imports_from_other_chunks,
      &mut index_chunk_indirect_imports_from_external_modules,
    );
    self.deconflict_exported_names(chunk_graph, &index_chunk_exported_symbols);

    let index_sorted_cross_chunk_imports = index_cross_chunk_imports
      .par_iter_enumerated()
      .map(|(chunk_idx, cross_chunk_imports)| {
        // Include imports from `imports_from_other_chunks` which may have been
        // added during chunk merging optimization (PR #7194).
        // See: https://github.com/rolldown/rolldown/issues/7297
        let mut cross_chunk_imports = cross_chunk_imports
          .iter()
          .copied()
          .chain(chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks.keys().copied())
          .collect::<Vec<_>>();
        cross_chunk_imports.sort_by_cached_key(|chunk_id| {
          let mut module_ids = chunk_graph.chunk_table[*chunk_id]
            .modules
            .iter()
            .map(|id| self.link_output.module_table[*id].id().as_str())
            .collect::<Vec<_>>();
          module_ids.sort_unstable();
          module_ids
        });
        cross_chunk_imports
      })
      .collect::<Vec<_>>();

    let index_sorted_imports_from_other_chunks = index_imports_from_other_chunks
      .into_iter_enumerated()
      .map(|(chunk_idx, mut importee_map)| {
        for (idx, items) in &chunk_graph.chunk_table[chunk_idx].imports_from_other_chunks {
          importee_map.entry(*idx).or_default().extend_from_slice(items);
        }
        importee_map
          .into_iter()
          .sorted_by_key(|(importee_chunk_id, _)| {
            chunk_graph.chunk_table[*importee_chunk_id].exec_order
          })
          .collect::<FxIndexMap<_, _>>()
      })
      .collect::<Vec<_>>();

    let index_sorted_imports_from_external_modules =
      index_chunk_direct_imports_from_external_modules
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
      index_chunk_indirect_imports_from_external_modules,
    ))
    .par_bridge()
    .for_each(
      |(
        chunk,
        sorted_imports_from_other_chunks,
        imports_from_external_modules,
        cross_chunk_imports,
        cross_chunk_dynamic_imports,
        mut chunk_indirect_imports_from_external_modules,
      )| {
        // deduplicated
        for (module_idx, _) in &imports_from_external_modules {
          chunk_indirect_imports_from_external_modules.shift_remove(module_idx);
        }
        chunk.imports_from_other_chunks = sorted_imports_from_other_chunks;
        chunk.direct_imports_from_external_modules = imports_from_external_modules;
        chunk.cross_chunk_imports = cross_chunk_imports;
        chunk.cross_chunk_dynamic_imports =
          cross_chunk_dynamic_imports.into_iter().collect::<Vec<_>>();
        chunk.import_symbol_from_external_modules = chunk_indirect_imports_from_external_modules;
      },
    );
  }

  /// - Assign each symbol to the chunk it belongs to
  /// - Collect all referenced symbols and consider them potential imports
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
            .filter_map(|rec| rec.resolved_module.map(|module_idx| (rec, module_idx)))
            .for_each(|(rec, module_idx)| {
              match &self.link_output.module_table[module_idx] {
                Module::Normal(_) => {
                  // The the resolved module is not included in module graph, skip it.
                  if !self.link_output.metas[module_idx].is_included {
                    return;
                  }
                  if matches!(rec.kind, ImportKind::DynamicImport) {
                    let importee_chunk =
                      chunk_graph.module_to_chunk[module_idx].expect("importee chunk should exist");
                    cross_chunk_dynamic_imports.insert(importee_chunk);
                  }
                }
                Module::External(_) => {
                  // Ensure the external module is imported in case it has side effects.
                  if matches!(rec.kind, ImportKind::Import)
                    && !rec.meta.contains(ImportRecordMeta::IsExportStar)
                  {
                    imports_from_external_modules.entry(module_idx).or_default();
                  }
                }
              }
            });

          module
            .named_imports
            .iter()
            .filter_map(|(_, import)| {
              module.import_records[import.record_idx]
                .resolved_module
                .map(|module_idx| (import, module_idx))
            })
            .for_each(|(import, module_idx)| {
              if let Module::External(importee) = &self.link_output.module_table[module_idx] {
                imports_from_external_modules
                  .entry(importee.idx)
                  .or_default()
                  .push((module.idx, import.clone()));
              }
            });
          module.stmt_infos.iter_enumerated().for_each(|(stmt_info_idx, stmt_info)| {
            if !self.link_output.metas[module.idx].stmt_info_included[stmt_info_idx] {
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

          if !matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
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

          if !matches!(entry_meta.wrap_kind(), WrapKind::None) {
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

        // Depending runtime helpers
        for helper in chunk.depended_runtime_helper {
          let index = helper.bits().trailing_zeros() as usize;
          let name = RUNTIME_HELPER_NAMES[index];
          depended_symbols.insert(self.link_output.runtime.resolve_symbol(name));
        }

        chunk_id_to_symbols_vec.push((chunk_id, symbol_needs_to_assign));
      },
    );
    // shadowing previous immutable borrow
    let symbols = &mut self.link_output.symbol_db;
    for (chunk_idx, symbol_list) in chunk_id_to_symbols_vec {
      for declared in symbol_list {
        let declared = declared.inner();
        if cfg!(debug_assertions) {
          let symbol_data = symbols.get(declared);
          debug_assert!(
            symbol_data.chunk_idx.unwrap_or(chunk_idx) == chunk_idx,
            "Symbol: {:?}, {:?} in {:?} should only belong to one chunk. Existed {:?}, new {chunk_idx:?}",
            declared.name(symbols),
            declared,
            self.link_output.module_table[declared.owner].id().as_str(),
            symbol_data.chunk_idx,
          );
        }

        let symbol_data = symbols.get_mut(declared);
        symbol_data.chunk_idx = Some(chunk_idx);
      }
    }
  }

  /// - Filter out depended symbols to come from other chunks
  /// - Mark exports of importee chunks
  fn compute_chunk_imports(
    &self,
    chunk_graph: &ChunkGraph,
    index_chunk_depended_symbols: &IndexChunkDependedSymbols,
    index_chunk_exported_symbols: &mut IndexChunkExportedSymbols,
    index_cross_chunk_imports: &mut IndexCrossChunkImports,
    index_imports_from_other_chunks: &mut IndexImportsFromOtherChunks,
    index_chunk_indirect_imports_from_external_modules: &mut IndexChunkAllImportsFromExternalModules,
  ) {
    chunk_graph
      .chunk_table
      .iter_enumerated()
      // Skip chunks that are purely removed (merged into other chunks without preserving exports).
      // Chunks with PreserveExports flag (e.g., emitted chunks merged into common chunks) are kept
      // because their exports still need to be computed.
      .filter(|(chunk_id, _)| {
        !chunk_graph
          .post_chunk_optimization_operations
          .get(chunk_id)
          .map(|flag| *flag == PostChunkOptimizationOperation::Removed)
          .unwrap_or(false)
      })
      .for_each(|(chunk_id, chunk)| {
        match chunk.kind {
          ChunkKind::EntryPoint { module: module_idx, meta, .. } => {
            let is_dynamic_imported = meta.contains(ChunkMeta::DynamicImported);
            let is_user_defined =
              meta.intersects(ChunkMeta::UserDefinedEntry | ChunkMeta::EmittedChunk);

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
                index_chunk_exported_symbols[chunk_id]
                  .entry(symbol)
                  .or_default()
                  .push(name.clone());
              }
            }
          }
          ChunkKind::Common => {
            if let Some(set) =
              chunk_graph.common_chunk_exported_facade_chunk_namespace.get(&chunk_id)
            {
              for dynamic_entry_module in set {
                let meta = &self.link_output.metas[*dynamic_entry_module];
                match meta.wrap_kind() {
                  WrapKind::Cjs => {
                    // For CJS modules, export only wrapper_ref (require_xxx)
                    // Generated code: `import('./chunk.js').then((n) => __toESM(n.require_xxx()))`
                    if let Some(wrapper_ref) = meta.wrapper_ref {
                      index_chunk_exported_symbols[chunk_id].entry(wrapper_ref).or_default();
                    }
                  }
                  WrapKind::Esm => {
                    // For ESM modules, export both wrapper_ref (init_xxx) and namespace
                    // Generated code: `import('./chunk.js').then((n) => (n.init_xxx(), n.namespace))`
                    if let Some(wrapper_ref) = meta.wrapper_ref {
                      index_chunk_exported_symbols[chunk_id].entry(wrapper_ref).or_default();
                    }
                    index_chunk_exported_symbols[chunk_id]
                      .entry(SymbolId::module_namespace_symbol_ref(*dynamic_entry_module))
                      .or_default();
                  }
                  WrapKind::None => {
                    // For non-wrapped modules, export only namespace
                    // Generated code: `import('./chunk.js').then((n) => n.namespace)`
                    index_chunk_exported_symbols[chunk_id]
                      .entry(SymbolId::module_namespace_symbol_ref(*dynamic_entry_module))
                      .or_default();
                  }
                }
              }
            }
          }
        }

        let chunk_meta_imports = &index_chunk_depended_symbols[chunk_id];
        for import_ref in chunk_meta_imports.iter().copied() {
          if !self.link_output.used_symbol_refs.contains(&import_ref) {
            continue;
          }
          // If the symbol from external module and the format is commonjs, we might need to insert runtime
          // symbol ref `__toESM` if it's being used (for namespace or default imports)
          // related to https://github.com/rolldown/rolldown/blob/c100a53c6cfc67b4f92e230da072eef8494862ef/crates/rolldown/src/ecmascript/format/cjs.rs?plain=1#L120-L124
          let import_ref = if self.link_output.module_table[import_ref.owner].is_external() {
            index_chunk_indirect_imports_from_external_modules[chunk_id].insert(import_ref.owner);
            if matches!(self.options.format, OutputFormat::Esm) {
              continue;
            }

            // Note: the `__toESM` might have been referenced during `collect_depended_symbols` phase
            // for namespace or default imports from external modules.
            // For named-only imports, we don't use __toESM, so we should not try to resolve it.
            // Check if __toESM is actually used before trying to resolve it.
            let to_esm_ref = self.link_output.runtime.resolve_symbol("__toESM");
            if self.link_output.symbol_db.get(to_esm_ref).chunk_idx.is_some() {
              // __toESM is in a chunk, so it's being used
              to_esm_ref
            } else {
              // __toESM is not being used, so skip this import
              // This happens for named-only imports from external modules where we don't need interop
              continue;
            }
          } else {
            import_ref
          };
          let import_symbol = self.link_output.symbol_db.get(import_ref);
          let importee_chunk_idx = import_symbol.chunk_idx.unwrap_or_else(|| {
            let symbol_owner = &self.link_output.module_table[import_ref.owner];
            let symbol_name = import_ref.name(&self.link_output.symbol_db);
            panic!(
              "Symbol {:?} in {:?} should belong to a chunk",
              symbol_name,
              symbol_owner.id().as_str()
            )
          });
          // Check if the import is from another chunk
          if chunk_id != importee_chunk_idx {
            index_cross_chunk_imports[chunk_id].insert(importee_chunk_idx);
            let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
            imports_from_other_chunks
              .entry(importee_chunk_idx)
              .or_default()
              .push(CrossChunkImportItem { import_ref });
            index_chunk_exported_symbols[importee_chunk_idx].entry(import_ref).or_default();
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
              .filter_map(|r| r.resolved_module)
              .for_each(|module_idx| {
                if !self.link_output.module_table[module_idx].side_effects().has_side_effects() {
                  return;
                }
                let Some(importee_chunk_idx) = chunk_graph.module_to_chunk[module_idx] else {
                  return;
                };
                index_cross_chunk_imports[chunk_id].insert(importee_chunk_idx);
                let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
                imports_from_other_chunks.entry(importee_chunk_idx).or_default();
              });
          } else if !self.options.is_strict_execution_order_enabled() {
            // With strict_execution_order/wrapping, modules aren't executed in loading but on-demand.
            // So we don't need to do plain imports to address the side effects. It would be ensured
            // by those `init_xxx()` calls.
            chunk_graph
              .chunk_table
              .iter_enumerated()
              .filter(|(id, _)| *id != chunk_id)
              .filter(|(_, importee_chunk)| {
                importee_chunk.bits.has_bit(*importer_chunk_bit)
                  && importee_chunk.has_side_effect(&self.link_output.module_table)
              })
              .for_each(|(importee_chunk_id, _)| {
                index_cross_chunk_imports[chunk_id].insert(importee_chunk_id);
                let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
                imports_from_other_chunks.entry(importee_chunk_id).or_default();
              });

            // Also check direct imports from all modules in this chunk to modules in other chunks.
            // This handles cases where the bit-based check above misses cross-chunk dependencies:
            // 1. Entry A imports entry B - entry B's chunk bits don't contain entry A's bit
            // 2. Dynamic chunk imports a module that was inlined into another chunk
            //
            // We need to check ALL modules in the chunk because non-entry modules may be
            // inlined into the entry chunk and their imports need to be preserved.
            for &module_idx in &chunk.modules {
              let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
                continue;
              };
              for rec in &module.import_records {
                if rec.kind == ImportKind::DynamicImport {
                  continue;
                }
                let Some(importee_module_idx) = rec.resolved_module else {
                  continue;
                };
                if !self.link_output.module_table[importee_module_idx]
                  .side_effects()
                  .has_side_effects()
                {
                  continue;
                }
                let Some(importee_chunk_idx) = chunk_graph.module_to_chunk[importee_module_idx]
                else {
                  continue;
                };
                if importee_chunk_idx == chunk_id {
                  continue;
                }
                // Skip if already covered by the bit-based check above
                let importee_chunk = &chunk_graph.chunk_table[importee_chunk_idx];
                if importee_chunk.bits.has_bit(*importer_chunk_bit) {
                  continue;
                }
                index_cross_chunk_imports[chunk_id].insert(importee_chunk_idx);
                let imports_from_other_chunks = &mut index_imports_from_other_chunks[chunk_id];
                imports_from_other_chunks.entry(importee_chunk_idx).or_default();
              }
            }
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
    let preserve_export_names_modules =
      std::mem::take(&mut chunk_graph.common_chunk_preserve_export_names_modules);
    for (chunk_id, chunk) in chunk_graph.chunk_table.iter_mut_enumerated() {
      if allow_to_minify_internal_exports {
        // Reference: https://github.com/rollup/rollup/blob/f76339428586620ff3e4c32fce48f923e7be7b05/src/utils/exportNames.ts#L5
        let mut named_index = 0;
        let mut used_names = FxHashSet::default();

        let mut processed_entry_exports = FxHashSet::default();
        if let Some(entry_module_idx) = chunk.entry_module_idx() {
          let exported_chunk_symbols = &index_chunk_exported_symbols[chunk_id];
          // If this's an entry point, we need to make sure the entry modules' exports are not minified.
          let entry_module = &self.link_output.metas[entry_module_idx];
          entry_module.canonical_exports(false).for_each(|(name, export)| {
            let export_ref = self.link_output.symbol_db.canonical_ref_for(export.symbol_ref);
            if !exported_chunk_symbols.contains_key(&export.symbol_ref)
              || !self.link_output.used_symbol_refs.contains(&export.symbol_ref)
            {
              // Rolldown supports tree-shaking on dynamic entries, so not all exports are used.
              return;
            }
            used_names.insert(name.clone());
            chunk.exports_to_other_chunks.entry(export_ref).or_default().push(name.clone());
            processed_entry_exports.insert(export_ref);
          });
        }
        // Also preserve exports from AllowExtension emitted chunks that were merged into this chunk
        if let Some(modules) = preserve_export_names_modules.get(&chunk_id) {
          let exported_chunk_symbols = &index_chunk_exported_symbols[chunk_id];
          for &module_idx in modules {
            let module_meta = &self.link_output.metas[module_idx];
            module_meta.canonical_exports(false).for_each(|(name, export)| {
              let export_ref = self.link_output.symbol_db.canonical_ref_for(export.symbol_ref);
              // Use canonical ref for lookup since that's the key in exported_chunk_symbols
              if !exported_chunk_symbols.contains_key(&export_ref)
                || !self.link_output.used_symbol_refs.contains(&export_ref)
              {
                return;
              }
              // Skip if already processed (e.g., same symbol re-exported from multiple modules)
              if processed_entry_exports.contains(&export_ref) {
                return;
              }
              used_names.insert(name.clone());
              chunk.exports_to_other_chunks.entry(export_ref).or_default().push(name.clone());
              processed_entry_exports.insert(export_ref);
            });
          }
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

          let mut export_name: CompactStr;
          loop {
            named_index += 1;
            export_name = generate_minified_names(named_index).into();
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
        if !self.link_output.used_symbol_refs.contains(chunk_export) {
          continue;
        }
        let original_name: CompactStr = match predefined_names.as_slice() {
          [] => CompactStr::new(chunk_export.name(&self.link_output.symbol_db)),
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
          let key: Cow<'_, CompactStr> = Cow::Owned(candidate_name.clone());
          match name_count.entry(key) {
            std::collections::hash_map::Entry::Occupied(mut occ) => {
              let next_conflict_index = *occ.get() + 1;
              *occ.get_mut() = next_conflict_index;
              candidate_name = CompactStr::new(&concat_string!(
                original_name,
                "$",
                itoa::Buffer::new().format(next_conflict_index)
              ));
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

// The same implementation with https://github.com/oxc-project/oxc/blob/crates_v0.86.0/crates/oxc_mangler/src/base54.rs#L30-L31
const FIRST_BASE: u32 = 54;
const REST_BASE: u32 = 64;
const FREQUENT_CHARS: &[u8; REST_BASE as usize] =
  b"etnriaoscludfpmhg_vybxSCwTEDOkAjMNPFILRzBVHUWGKqJYXZQ$1024368579";

fn generate_minified_names(mut value: u32) -> String {
  let mut buffer = vec![];

  // Base 54 at first because these are the usable first characters in JavaScript identifiers
  let byte = FREQUENT_CHARS[(value % FIRST_BASE) as usize];
  buffer.push(byte);
  value /= FIRST_BASE;

  while value > 0 {
    let byte = FREQUENT_CHARS[(value % REST_BASE) as usize];
    buffer.push(byte);
    value /= REST_BASE;
  }
  // SAFETY: `buffer` is base64 characters, it is valid utf8 characters
  unsafe { String::from_utf8_unchecked(buffer) }
}
