// TODO: The current implementation for matching imports is enough so far but incomplete. It needs to be refactored
// if we want more enhancements related to exports.

use rayon::iter::{ParallelBridge, ParallelIterator};
use rolldown_common::{ExportsKind, NamedImport, ResolvedExport, Specifier, SymbolRef};

use crate::bundler::{
  module::{Module, ModuleVec, NormalModule},
  types::{
    linking_metadata::{LinkingMetadata, LinkingMetadataVec},
    match_import_kind::MatchImportKind,
  },
  utils::symbols::NamespaceAlias,
};

use super::LinkStage;

impl<'a> LinkStage<'a> {
  pub fn bind_imports_and_exports(&mut self) {
    self.modules.iter().zip(self.metas.iter_mut()).par_bridge().for_each(|(module, meta)| {
      match module {
        Module::Normal(module) => {
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
        }
        Module::External(_) => {}
      }
    });

    // Add exports for export star. Notice that:
    // - There will be potentially ambiguous exports, which need to be resolved later
    let mut module_stack_for_export_star = Vec::default();
    self.modules.iter_enumerated().for_each(|(id, module)| match module {
      Module::Normal(module) => {
        module_stack_for_export_star.clear();
        module.add_exports_for_export_star(
          id,
          &mut self.metas,
          &self.modules,
          &mut module_stack_for_export_star,
        );
      }
      Module::External(_) => {}
    });

    // match imports with exports
    self.modules.iter().for_each(|importer| {
      let Module::Normal(importer) = importer else {
        return;
      };

      importer.named_imports.values().for_each(|import| {
        let import_record = &importer.import_records[import.record_id];
        let Module::Normal(importee) = &self.modules[import_record.resolved_module] else {
          return;
        };

        match Self::match_import_with_export(importer, importee, &self.metas[importee.id], import) {
          MatchImportKind::NotFound => panic!("info {import:#?}"),
          MatchImportKind::PotentiallyAmbiguous(
            symbol_ref,
            mut potentially_ambiguous_symbol_refs,
          ) => {
            potentially_ambiguous_symbol_refs.push(symbol_ref);
            if Self::determine_ambiguous_export(
              &self.modules,
              potentially_ambiguous_symbol_refs,
              &self.metas,
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
    importee_meta: &LinkingMetadata,
    import: &NamedImport,
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

    MatchImportKind::NotFound
  }

  // Iterate all potentially ambiguous symbol refs, If all results not be same, it's a ambiguous export
  fn determine_ambiguous_export(
    modules: &ModuleVec,
    potentially_ambiguous_symbol_refs: Vec<SymbolRef>,
    metas: &LinkingMetadataVec,
  ) -> bool {
    let mut results = vec![];

    for symbol_ref in potentially_ambiguous_symbol_refs {
      match &modules[symbol_ref.owner] {
        Module::Normal(importer) => {
          if let Some(info) = importer.named_imports.get(&symbol_ref.symbol) {
            let importee_id = importer.import_records[info.record_id].resolved_module;
            match &modules[importee_id] {
              Module::Normal(importee) => {
                results.push(Self::match_import_with_export(
                  importer,
                  importee,
                  &metas[importee_id],
                  info,
                ));
              }
              Module::External(_) => {}
            }
          } else {
            results.push(MatchImportKind::Found(symbol_ref));
          }
        }
        Module::External(_) => {}
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
