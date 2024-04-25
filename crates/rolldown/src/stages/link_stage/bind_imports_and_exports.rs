// TODO: The current implementation for matching imports is enough so far but incomplete. It needs to be refactored
// if we want more enhancements related to exports.

use rolldown_common::{
  ExportsKind, ModuleId, NamedImport, NormalModule, NormalModuleId, NormalModuleVec,
  ResolvedExport, Specifier, SymbolRef,
};
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};

use crate::{
  types::{
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    match_import_kind::MatchImportKind,
    namespace_alias::NamespaceAlias,
    symbols::Symbols,
  },
  SharedOptions,
};

use super::LinkStage;

impl<'a> LinkStage<'a> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn bind_imports_and_exports(&mut self) {
    self.module_table.normal_modules.iter().zip(self.metas.iter_mut()).par_bridge().for_each(
      |(module, meta)| {
        meta.resolved_exports = module
          .named_exports
          .iter()
          .map(|(name, local)| {
            let resolved_export = ResolvedExport {
              symbol_ref: local.referenced,
              potentially_ambiguous_symbol_refs: None,
            };
            (name.clone(), resolved_export)
          })
          .collect();
      },
    );

    // Add exports for export star. Notice that:
    // - There will be potentially ambiguous exports, which need to be resolved later
    let mut module_stack_for_export_star = Vec::default();
    self.module_table.normal_modules.iter_enumerated().for_each(|(id, module)| {
      module_stack_for_export_star.clear();
      add_exports_for_export_star(
        module,
        id,
        &mut self.metas,
        &self.module_table.normal_modules,
        &mut module_stack_for_export_star,
      );
    });

    // match imports with exports
    self.module_table.normal_modules.iter().for_each(|importer| {
      importer.named_imports.values().for_each(|import| {
        let import_record = &importer.import_records[import.record_id];
        let ModuleId::Normal(importee_id) = import_record.resolved_module else {
          return;
        };
        let importee = &self.module_table.normal_modules[importee_id];

        match Self::match_import_with_export(
          importer,
          importee,
          &mut self.metas[importee.id],
          import,
          &mut self.symbols,
          self.input_options,
        ) {
          MatchImportKind::NotFound => panic!("info {import:#?}"),
          MatchImportKind::PotentiallyAmbiguous(
            symbol_ref,
            mut potentially_ambiguous_symbol_refs,
          ) => {
            potentially_ambiguous_symbol_refs.push(symbol_ref);
            if Self::determine_ambiguous_export(
              &self.module_table.normal_modules,
              potentially_ambiguous_symbol_refs,
              &mut self.metas,
              &mut self.symbols,
              self.input_options,
            ) {
              // ambiguous export
              panic!("ambiguous export");
            }
            self.symbols.union(import.imported_as, symbol_ref);
          }
          MatchImportKind::Found(symbol_ref) => {
            self.symbols.union(import.imported_as, symbol_ref);
          }
          MatchImportKind::Namespace(ns_ref) => match &import.imported {
            Specifier::Star => {
              self.symbols.union(import.imported_as, ns_ref);
            }
            Specifier::Literal(imported) => {
              self.symbols.get_mut(import.imported_as).namespace_alias =
                Some(NamespaceAlias { property_name: imported.clone(), namespace_ref: ns_ref });
            }
          },
        }
      });
    });

    // Exclude ambiguous from resolved exports
    self.sorted_modules.clone().into_iter().for_each(|id| {
      let linking_info = &mut self.metas[id];
      linking_info.create_exclude_ambiguous_resolved_exports(&self.symbols);
    });
  }

  pub fn match_import_with_export(
    importer: &NormalModule,
    importee: &NormalModule,
    importee_meta: &mut LinkingMetadata,
    import: &NamedImport,
    symbols: &mut Symbols,
    options: &SharedOptions,
  ) -> MatchImportKind {
    // If importee module is commonjs module, it will generate property access to namespace symbol
    // The namespace symbols should be importer created local symbol.
    if importee.exports_kind == ExportsKind::CommonJs {
      let rec = &importer.import_records[import.record_id];
      match import.imported {
        Specifier::Star => {
          return MatchImportKind::Found(rec.namespace_ref);
        }
        Specifier::Literal(_) => {
          return MatchImportKind::Namespace(rec.namespace_ref);
        }
      }
    }

    match &import.imported {
      Specifier::Star => {
        return MatchImportKind::Found(importee.namespace_symbol);
      }
      Specifier::Literal(imported) => {
        if let Some(resolved_export) = importee_meta.resolved_exports.get(imported) {
          if let Some(potentially_ambiguous_export) =
            &resolved_export.potentially_ambiguous_symbol_refs
          {
            return MatchImportKind::PotentiallyAmbiguous(
              resolved_export.symbol_ref,
              potentially_ambiguous_export.clone(),
            );
          }
          return MatchImportKind::Found(resolved_export.symbol_ref);
        }
      }
    }

    // If the module has dynamic exports, the unknown export name will be resolved at runtime.
    // The namespace symbol should be importee namespace symbol.
    if importee_meta.has_dynamic_exports {
      return MatchImportKind::Namespace(importee.namespace_symbol);
    }
    if options.shim_missing_exports {
      match &import.imported {
        Specifier::Star => unreachable!("star should always exist, no need to shim"),
        Specifier::Literal(imported_name) => {
          // TODO: should emit warnings for shimmed exports
          let shimmed_symbol_ref =
            importee_meta.shimmed_missing_exports.entry(imported_name.clone()).or_insert_with(
              || symbols.create_symbol(importee.id, imported_name.clone().to_string().into()),
            );

          return MatchImportKind::Found(*shimmed_symbol_ref);
        }
      }
    }

    MatchImportKind::NotFound
  }

  // Iterate all potentially ambiguous symbol refs, If all results not be same, it's a ambiguous export
  fn determine_ambiguous_export(
    modules: &NormalModuleVec,
    potentially_ambiguous_symbol_refs: Vec<SymbolRef>,
    metas: &mut LinkingMetadataVec,
    symbols: &mut Symbols,
    options: &SharedOptions,
  ) -> bool {
    let mut results = vec![];

    for symbol_ref in potentially_ambiguous_symbol_refs {
      let importer = &modules[symbol_ref.owner];
      if let Some(info) = importer.named_imports.get(&symbol_ref) {
        let importee_id = importer.import_records[info.record_id].resolved_module;
        let ModuleId::Normal(importee_id) = importee_id else {
          continue;
        };
        let importee = &modules[importee_id];
        results.push(Self::match_import_with_export(
          importer,
          importee,
          &mut metas[importee_id],
          info,
          symbols,
          options,
        ));
      } else {
        results.push(MatchImportKind::Found(symbol_ref));
      }
    }
    let current_result = results.remove(results.len() - 1);
    if let MatchImportKind::Found(symbol_ref) = current_result {
      for result in results {
        if let MatchImportKind::Found(result_symbol_ref) = result {
          if result_symbol_ref != symbol_ref {
            // ambiguous export
            return true;
          }
        }
      }
    }
    false
  }
}

pub(crate) fn add_exports_for_export_star(
  module: &NormalModule,
  id: NormalModuleId,
  metas: &mut LinkingMetadataVec,
  modules: &NormalModuleVec,
  module_stack: &mut Vec<NormalModuleId>,
) {
  if module_stack.contains(&module.id) {
    return;
  }
  module_stack.push(module.id);

  module.star_export_modules().filter_map(ModuleId::as_normal).for_each(|importee_id| {
    let importee = &modules[importee_id];
    // Export star from commonjs will be resolved at runtime
    if importee.exports_kind == ExportsKind::CommonJs {
      return;
    }

    importee.named_exports.iter().for_each(|(alias, importee_export)| {
      // ES6 export star ignore default export
      if alias.as_str() == "default" {
        return;
      }

      // This export star is shadowed if any file in the stack has a matching real named export
      if module_stack
        .iter()
        .copied()
        .map(|id| &modules[id])
        .any(|prev_module| prev_module.named_exports.contains_key(alias))
      {
        return;
      }

      let importer_meta = &mut metas[id];

      importer_meta
        .resolved_exports
        .entry(alias.clone())
        .and_modify(|existing| {
          if existing.symbol_ref != importee_export.referenced {
            // This means that the importer already has a export with the same name, and it's not from its own
            // local named exports. Such a situation is already handled above, so this is a case of ambiguity.
            existing
              .potentially_ambiguous_symbol_refs
              .get_or_insert_with(Default::default)
              .push(importee_export.referenced);
          }
        })
        .or_insert_with(|| ResolvedExport::new(importee_export.referenced));
    });

    add_exports_for_export_star(importee, id, metas, modules, module_stack);
  });

  module_stack.remove(module_stack.len() - 1);
}
