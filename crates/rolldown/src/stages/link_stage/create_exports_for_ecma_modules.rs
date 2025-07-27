use rolldown_common::{
  EntryPoint, ExportsKind, ModuleIdx, OutputFormat, PreserveEntrySignatures,
  SharedNormalizedBundlerOptions, StmtInfo, StmtInfoMeta, TaggedSymbolRef, WrapKind,
  dynamic_import_usage::DynamicImportExportsUsage,
};
use rustc_hash::FxHashMap;

use crate::{
  types::linking_metadata::LinkingMetadata, utils::chunk::normalize_preserve_entry_signature,
};

use super::LinkStage;

fn init_entry_point_stmt_info(
  meta: &mut LinkingMetadata,
  entry: &EntryPoint,
  dynamic_import_exports_usage_map: &FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
  options: &SharedNormalizedBundlerOptions,
  overrode_preserve_entry_signature_map: &FxHashMap<ModuleIdx, PreserveEntrySignatures>,
  is_dynamic_imported: bool,
) {
  let mut referenced_symbols = vec![];

  // Include the wrapper if present
  if !matches!(meta.wrap_kind, WrapKind::None) {
    // If a commonjs module becomes an entry point while targeting esm, we need to at least add a `export default require_foo();`
    // statement as some kind of syntax sugar. So users won't need to manually create a proxy file with `export default require('./foo.cjs')` in it.
    referenced_symbols.push((meta.wrapper_ref.unwrap(), false));
  }

  let normalized_entry_signature =
    normalize_preserve_entry_signature(overrode_preserve_entry_signature_map, options, entry.id);

  if !matches!(normalized_entry_signature, PreserveEntrySignatures::False) || is_dynamic_imported {
    referenced_symbols.extend(
      meta
        .referenced_canonical_exports_symbols(
          entry.id,
          entry.kind,
          dynamic_import_exports_usage_map,
          true,
        )
        .map(|(_, resolved_export)| (resolved_export.symbol_ref, resolved_export.came_from_cjs)),
    );
  }
  // Entry chunk need to generate exports, so we need reference to all exports to make sure they are included in tree-shaking.

  meta.referenced_symbols_by_entry_point_chunk.extend(referenced_symbols);
}

impl LinkStage<'_> {
  pub(super) fn create_exports_for_ecma_modules(&mut self) {
    self.module_table.modules.iter_mut().filter_map(|m| m.as_normal_mut()).for_each(
      |ecma_module| {
        let linking_info = &mut self.metas[ecma_module.idx];

        if let Some(entry) = self.entries.iter().find(|entry| entry.id == ecma_module.idx) {
          init_entry_point_stmt_info(
            linking_info,
            entry,
            &self.dynamic_import_exports_usage_map,
            self.options,
            &self.overrode_preserve_entry_signature_map,
            !ecma_module.dynamic_importers.is_empty(),
          );
        }

        // Create facade StmtInfo that declares variables based on the missing exports, so they can participate in the symbol de-conflict and
        // tree-shaking process.
        linking_info.shimmed_missing_exports.iter().for_each(|(_name, symbol_ref)| {
          let stmt_info = StmtInfo {
            stmt_idx: None,
            declared_symbols: vec![TaggedSymbolRef::Normal(*symbol_ref)],
            referenced_symbols: vec![],
            side_effect: false.into(),
            is_included: false,
            import_records: Vec::new(),
            #[cfg(debug_assertions)]
            debug_label: None,
            meta: StmtInfoMeta::default(),
            ..Default::default()
          };
          ecma_module.stmt_infos.add_stmt_info(stmt_info);
        });

        // Generate export of Module Namespace Object for Namespace Import
        // - Namespace import: https://tc39.es/ecma262/#prod-NameSpaceImport
        // - Module Namespace Object: https://tc39.es/ecma262/#sec-module-namespace-exotic-objects
        // Though Module Namespace Object is created in runtime, as a bundler, we have stimulus the behavior in compile-time and generate a
        // real statement to construct the Module Namespace Object and assign it to a variable.
        // This is only a concept of esm, so no need to care about this in commonjs.
        if matches!(ecma_module.exports_kind, ExportsKind::Esm) {
          let meta = &mut self.metas[ecma_module.idx];
          let mut referenced_symbols = vec![];
          let mut declared_symbols = vec![];
          if !meta.is_canonical_exports_empty() {
            referenced_symbols.push(self.runtime.resolve_symbol("__export").into());
            referenced_symbols
              .extend(meta.canonical_exports(false).map(|(_, export)| export.symbol_ref.into()));
          }
          if !meta.star_exports_from_external_modules.is_empty() {
            referenced_symbols.push(self.runtime.resolve_symbol("__reExport").into());
            match self.options.format {
              OutputFormat::Esm => {
                meta.star_exports_from_external_modules.iter().copied().for_each(|rec_idx| {
                  referenced_symbols.push(ecma_module.import_records[rec_idx].namespace_ref.into());
                  declared_symbols.push(TaggedSymbolRef::Normal(
                    ecma_module.import_records[rec_idx].namespace_ref,
                  ));
                });
              }
              OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd => {}
            }
          }
          // Create a StmtInfo to represent the statement that declares and constructs the Module Namespace Object.
          // Corresponding AST for this statement will be created by the finalizer.
          declared_symbols.push(TaggedSymbolRef::Normal(ecma_module.namespace_object_ref));
          let namespace_stmt_info = StmtInfo {
            stmt_idx: None,
            declared_symbols,
            referenced_symbols,
            side_effect: false.into(),
            is_included: false,
            import_records: Vec::new(),
            #[cfg(debug_assertions)]
            debug_label: None,
            meta: StmtInfoMeta::default(),
            ..Default::default()
          };
          ecma_module.stmt_infos.replace_namespace_stmt_info(namespace_stmt_info);
        }
      },
    );
  }
}
