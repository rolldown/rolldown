use std::{borrow::Cow, ptr::addr_of};

use crate::{
  bundler::{
    chunk::chunk::{ChunkKind, CrossChunkImportItem},
    chunk_graph::ChunkGraph,
    module::Module,
  },
  OutputFormat,
};

use super::BundleStage;
use index_vec::index_vec;
use rayon::iter::{ParallelBridge, ParallelIterator};
use rolldown_common::{ExportsKind, ImportKind, ModuleId, NamedImport, SymbolRef};
use rustc_hash::{FxHashMap, FxHashSet};

impl<'a> BundleStage<'a> {
  // TODO(hyf0): refactor this function
  #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
  pub fn compute_cross_chunk_links(&mut self, chunk_graph: &mut ChunkGraph) {
    let mut chunk_meta_imports_vec =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut chunk_meta_exports_vec =
      index_vec![FxHashSet::<SymbolRef>::default(); chunk_graph.chunks.len()];
    let mut chunk_meta_imports_from_external_modules_vec =
      index_vec![FxHashMap::<ModuleId, Vec<NamedImport>>::default(); chunk_graph.chunks.len()];

    let symbols = &self.link_output.symbols;

    // - Assign each symbol to the chunk it belongs to
    // - Collect all referenced symbols and consider them potential imports
    chunk_graph
      .chunks
      .iter_enumerated()
      .zip(
        chunk_meta_imports_vec
          .iter_mut()
          .zip(chunk_meta_imports_from_external_modules_vec.iter_mut()),
      )
      .par_bridge()
      .for_each(|((chunk_id, chunk), (chunk_meta_imports, imports_from_external_modules))| {
        chunk.modules.iter().copied().for_each(|module_id| {
          let Module::Normal(module) = &self.link_output.modules[module_id] else { return };
          module
            .import_records
            .iter()
            .filter(|rec| matches!(rec.kind, ImportKind::Import))
            .filter_map(|rec| self.link_output.modules[rec.resolved_module].as_external())
            .for_each(|importee| {
              // Ensure the external module is imported in case it has side effects.
              imports_from_external_modules.entry(importee.id).or_default();
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
          let entry_module = &self.link_output.modules[*entry_module_id];
          let Module::Normal(entry_module) = entry_module else {
            return;
          };
          let entry_linking_info = &self.link_output.linking_infos[entry_module.id];
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

    for (chunk_id, chunk) in chunk_graph.chunks.iter_enumerated() {
      let chunk_meta_imports = &mut chunk_meta_imports_vec[chunk_id];
      let imports_from_external_modules =
        &mut chunk_meta_imports_from_external_modules_vec[chunk_id];

      for module_id in chunk.modules.iter().copied() {
        match &self.link_output.modules[module_id] {
          Module::Normal(module) => {
            module.import_records.iter().for_each(|rec| {
              match &self.link_output.modules[rec.resolved_module] {
                Module::External(importee) if matches!(rec.kind, ImportKind::Import) => {
                  // Make sure the side effects of external module are evaluated.
                  imports_from_external_modules.entry(importee.id).or_default();
                }
                _ => {}
              }
            });
            module.named_imports.iter().for_each(|(_, import)| {
              let rec = &module.import_records[import.record_id];
              if let Module::External(importee) = &self.link_output.modules[rec.resolved_module] {
                imports_from_external_modules.entry(importee.id).or_default().push(import.clone());
              }
            });
            for stmt_info in module.stmt_infos.iter() {
              if !stmt_info.is_included {
                continue;
              }
              for declared in &stmt_info.declared_symbols {
                let symbol = self.link_output.symbols.get_mut(*declared);
                debug_assert!(
                  symbol.chunk_id.unwrap_or(chunk_id) == chunk_id,
                  "Symbol: {:?}, {:?} in {:?} should only be declared in one chunk",
                  symbol.name,
                  declared,
                  module.resource_id,
                );

                self.link_output.symbols.get_mut(*declared).chunk_id = Some(chunk_id);
              }

              for referenced in &stmt_info.referenced_symbols {
                let canonical_ref = self.link_output.symbols.canonical_ref_for(*referenced);
                chunk_meta_imports.insert(canonical_ref);
              }
            }
          }
          Module::External(_) => {}
        }
      }

      if let ChunkKind::EntryPoint { module: entry_module_id, .. } = &chunk.kind {
        let entry_module = &self.link_output.modules[*entry_module_id];
        let Module::Normal(entry_module) = entry_module else {
          return;
        };
        let entry_linking_info = &self.link_output.linking_infos[entry_module.id];
        if matches!(entry_module.exports_kind, ExportsKind::CommonJs)
          && matches!(self.output_options.format, OutputFormat::Esm)
        {
          chunk_meta_imports
            .insert(entry_linking_info.wrapper_ref.expect("cjs should be wrapped in esm output"));
        }
        for export_ref in entry_linking_info.resolved_exports.values() {
          let mut canonical_ref = self.link_output.symbols.canonical_ref_for(export_ref.symbol_ref);
          let symbol = self.link_output.symbols.get(canonical_ref);
          if let Some(ns_alias) = &symbol.namespace_alias {
            canonical_ref = ns_alias.namespace_ref;
          }
          chunk_meta_imports.insert(canonical_ref);
        }
      }
    }

    for (chunk_id, chunk) in chunk_graph.chunks.iter_mut_enumerated() {
      let chunk_meta_imports = &chunk_meta_imports_vec[chunk_id];
      for import_ref in chunk_meta_imports.iter().copied() {
        let import_symbol = self.link_output.symbols.get(import_ref);

        let importee_chunk_id = import_symbol.chunk_id.unwrap_or_else(|| {
          let symbol_owner = &self.link_output.modules[import_ref.owner];
          let symbol_name = self.link_output.symbols.get_original_name(import_ref);
          panic!(
            "Symbol {:?} in {:?} should belong to a chunk",
            symbol_name,
            symbol_owner.resource_id()
          )
        });
        // Find out the import_ref whether comes from the chunk or external module.
        if chunk_id != importee_chunk_id {
          chunk
            .imports_from_other_chunks
            .entry(importee_chunk_id)
            .or_default()
            .push(CrossChunkImportItem { import_ref, export_alias: None });
          chunk_meta_exports_vec[importee_chunk_id].insert(import_ref);
        }
      }

      if chunk.is_entry_point() {
        continue;
      }
      // If this is an entry point, make sure we import all chunks belonging to
      // this entry point, even if there are no imports. We need to make sure
      // these chunks are evaluated for their side effects too.
      // TODO: ensure chunks are evaluated for their side effects too.
    }
    // Generate cross-chunk exports. These must be computed before cross-chunk
    // imports because of export alias renaming, which must consider all export
    // aliases simultaneously to avoid collisions.
    let mut name_count = FxHashMap::default();
    for (chunk_id, chunk) in chunk_graph.chunks.iter_mut_enumerated() {
      for export in chunk_meta_exports_vec[chunk_id].iter().copied() {
        let original_name = self.link_output.symbols.get_original_name(export);
        let count = name_count.entry(Cow::Borrowed(original_name)).or_insert(0u32);
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
