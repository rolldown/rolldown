use rolldown_common::{
  DeclaredSymbols, EntryPoint, ExportsKind, ImportKind, ImportRecordMeta, Module, ModuleIdx,
  OutputFormat, PreserveEntrySignatures, SharedNormalizedBundlerOptions, StmtInfo, StmtInfoMeta,
  SymbolRefFlags, TaggedSymbolRef, WrapKind, dynamic_import_usage::DynamicImportExportsUsage,
};
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::smallvec;

use crate::{
  types::linking_metadata::LinkingMetadata, utils::chunk::normalize_preserve_entry_signature,
};

use super::{LinkStage, bind_imports_and_exports::record_star_reexport_path};

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
  if !matches!(meta.wrap_kind(), WrapKind::None) {
    // If a commonjs module becomes an entry point while targeting esm, we need to at least add a `export default require_foo();`
    // statement as some kind of syntax sugar. So users won't need to manually create a proxy file with `export default require('./foo.cjs')` in it.
    referenced_symbols.push((meta.wrapper_ref.unwrap(), false));
  }

  let normalized_entry_signature =
    normalize_preserve_entry_signature(overrode_preserve_entry_signature_map, options, entry.idx);

  if !matches!(normalized_entry_signature, PreserveEntrySignatures::False) || is_dynamic_imported {
    for (_, resolved_export) in meta.referenced_canonical_exports_symbols(
      entry.idx,
      entry.kind,
      dynamic_import_exports_usage_map,
      true,
    ) {
      referenced_symbols.push((resolved_export.symbol_ref, resolved_export.came_from_commonjs));
    }
  }
  // Entry chunk need to generate exports, so we need reference to all exports to make sure they are included in tree-shaking.

  meta.referenced_symbols_by_entry_point_chunk.extend(referenced_symbols);
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn create_exports_for_ecma_modules(&mut self) {
    // See internal-docs/code-splitting/implementation.md.
    let receiver_importees = self
      .module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .flat_map(|module| {
        module.ecma_view.dynamic_import_called_names.iter().filter_map(|(rec_idx, names)| {
          let importee = module.import_records[*rec_idx].resolved_module?;
          names
            .iter()
            .any(|name| self.called_export_may_read_this(importee, name))
            .then_some(importee)
        })
      })
      .collect::<FxHashSet<_>>();
    for importee in receiver_importees {
      self.dynamic_import_exports_usage_map.insert(importee, DynamicImportExportsUsage::Complete);
    }

    let non_callable_then: FxHashSet<ModuleIdx> = self
      .module_table
      .modules
      .iter()
      .filter_map(Module::as_normal)
      .filter(|module| super::exports_namespace_as_then(module))
      .map(|module| module.idx)
      .collect();
    for module in &mut self.module_table.modules {
      let Some(module) = module.as_normal_mut() else { continue };
      for record in &mut module.import_records {
        if record.kind != ImportKind::DynamicImport {
          continue;
        }
        let Some(importee) = record.resolved_module else { continue };
        let meta = &self.metas[importee];
        // Promise resolution calls `then` with the namespace as its receiver.
        if meta.has_dynamic_exports
          || (meta.resolved_exports.contains_key("then") && !non_callable_then.contains(&importee))
        {
          self
            .dynamic_import_exports_usage_map
            .insert(importee, DynamicImportExportsUsage::Complete);
          record.meta.remove(ImportRecordMeta::PureDynamicImport);
        }
      }
    }

    for m in &mut self.module_table.modules {
      let Some(ecma_module) = m.as_normal_mut() else { continue };
      let idx = ecma_module.idx;
      let linking_info = &mut self.metas[idx];
      let stmt_infos = &mut self.stmt_infos[idx];

      if let Some(entry) = self.entries.get(&idx).and_then(|entries| entries.first()) {
        init_entry_point_stmt_info(
          linking_info,
          entry,
          &self.dynamic_import_exports_usage_map,
          self.options,
          &self.overrode_preserve_entry_signature_map,
          self.unreachable_modules.dynamically_imported(ecma_module),
        );
      }

      // Create facade StmtInfo that declares variables based on the missing exports, so they can participate in the symbol de-conflict and
      // tree-shaking process.
      linking_info.shimmed_missing_exports.iter().for_each(|(_name, symbol_ref)| {
        let stmt_info = StmtInfo {
          declared_symbols: smallvec![TaggedSymbolRef::normal(*symbol_ref)],
          referenced_symbols: vec![],
          eval_flags: false.into(),
          import_records: Vec::new(),
          #[cfg(debug_assertions)]
          debug_label: None,
          meta: StmtInfoMeta::default(),
          ..Default::default()
        };
        stmt_infos.add_stmt_info(stmt_info);
      });

      // Generate export of Module Namespace Object for Namespace Import
      // - Namespace import: https://tc39.es/ecma262/#prod-NameSpaceImport
      // - Module Namespace Object: https://tc39.es/ecma262/#sec-module-namespace-exotic-objects
      // Though Module Namespace Object is created in runtime, as a bundler, we have stimulus the behavior in compile-time and generate a
      // real statement to construct the Module Namespace Object and assign it to a variable.
      // This is only a concept of esm, so no need to care about this in commonjs. A module without
      // module syntax (`ExportsKind::None`) still needs it when `import()` reads its namespace.
      if matches!(ecma_module.exports_kind, ExportsKind::Esm | ExportsKind::None) {
        let meta = &mut self.metas[ecma_module.idx];
        let mut referenced_symbols = vec![];
        let mut declared_symbols: DeclaredSymbols = smallvec![];
        // A namespace that star exports complete at runtime calls `__exportAll` even without
        // names of its own.
        if !meta.is_canonical_exports_empty()
          || self.options.generated_code.symbols
          || meta.has_dynamic_exports
          || !meta.star_exports_from_external_modules.is_empty()
        {
          referenced_symbols.push(self.runtime.resolve_symbol("__exportAll").into());
          referenced_symbols
            .extend(meta.canonical_exports(false).map(|(_, export)| export.symbol_ref.into()));
        }
        if !meta.star_exports_from_external_modules.is_empty() {
          referenced_symbols.push(self.runtime.resolve_symbol("__reExport").into());
          match self.options.format {
            OutputFormat::Esm => {
              meta.star_exports_from_external_modules.iter().copied().for_each(|rec_idx| {
                let rec = &ecma_module.import_records[rec_idx];
                if rec.meta.contains(ImportRecordMeta::EntryLevelExternal) {
                  return;
                }
                referenced_symbols.push(rec.namespace_ref.into());
                declared_symbols
                  .push(TaggedSymbolRef::normal(ecma_module.import_records[rec_idx].namespace_ref));
              });
            }
            OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd => {}
          }
        }
        // Create a StmtInfo to represent the statement that declares and constructs the Module Namespace Object.
        // Corresponding AST for this statement will be created by the finalizer.
        declared_symbols.push(TaggedSymbolRef::normal(ecma_module.namespace_object_ref));
        let namespace_stmt_info = StmtInfo {
          declared_symbols,
          referenced_symbols,
          eval_flags: false.into(),
          import_records: Vec::new(),
          #[cfg(debug_assertions)]
          debug_label: None,
          meta: StmtInfoMeta::default(),
          ..Default::default()
        };
        stmt_infos.replace_namespace_stmt_info(namespace_stmt_info);
      }
    }

    if !self.options.is_strict_execution_order_enabled() {
      return;
    }
    for (entry_idx, entries) in &self.entries {
      let Some(entry) = entries.first() else {
        continue;
      };
      let normalized_entry_signature = normalize_preserve_entry_signature(
        &self.overrode_preserve_entry_signature_map,
        self.options,
        *entry_idx,
      );
      let is_dynamic_imported = self.module_table.modules[*entry_idx]
        .as_normal()
        .is_some_and(|module| self.unreachable_modules.dynamically_imported(module));
      if matches!(normalized_entry_signature, PreserveEntrySignatures::False)
        && !is_dynamic_imported
      {
        continue;
      }
      let meta = &self.metas[*entry_idx];
      for (name, resolved_export) in meta.referenced_canonical_exports_symbols(
        *entry_idx,
        entry.kind,
        &self.dynamic_import_exports_usage_map,
        true,
      ) {
        record_star_reexport_path(
          *entry_idx,
          name,
          resolved_export.symbol_ref,
          &self.module_table.modules,
          &self.metas,
          &mut self.star_reexport_records_by_imported_symbol,
          &mut FxHashSet::default(),
        );
      }
    }
  }

  /// Whether calling `name` on `importee`'s namespace can observe the namespace through `this`.
  /// Only an export that resolves to a function marked `NeverReadsThis` cannot.
  fn called_export_may_read_this(&self, importee: ModuleIdx, name: &str) -> bool {
    if !self.module_table[importee].as_normal().is_some_and(|m| m.exports_kind == ExportsKind::Esm)
    {
      return true;
    }
    !self.metas[importee].resolved_exports.get(name).is_some_and(|export| {
      self
        .symbols
        .canonical_ref_for(export.symbol_ref)
        .flags(&self.symbols)
        .is_some_and(|flags| flags.contains(SymbolRefFlags::NeverReadsThis))
    })
  }
}
