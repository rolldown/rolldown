use rolldown_common::{ExportsKind, ModuleId, NamedImport, Specifier, SymbolRef};

use crate::bundler::{
  module::{Module, ModuleVec, NormalModule},
  stages::link_stage::LinkStage as LinkStageOutput,
  types::linking_metadata::{LinkingMetadata, LinkingMetadataVec},
  utils::symbols::{NamespaceAlias, Symbols},
};

pub struct ImportExportLinker<'a, 'b> {
  graph: &'a mut LinkStageOutput<'b>,
}

impl<'a, 'b> ImportExportLinker<'a, 'b> {
  pub fn new(graph: &'a mut LinkStageOutput<'b>) -> Self {
    Self { graph }
  }

  pub fn link(&mut self, linking_infos: &mut LinkingMetadataVec) {
    // Here take the symbols to avoid borrow graph and mut borrow graph at same time
    let mut symbols = std::mem::take(&mut self.graph.symbols);

    // Propagate star exports
    // Create resolved exports for named export declarations
    // Mark dynamic exports due to export star
    for id in &self.graph.sorted_modules {
      let importer = &self.graph.modules[*id];
      match importer {
        Module::Normal(importer) => {
          let importer_linking_info = &mut linking_infos[*id];
          importer.create_initial_resolved_exports(importer_linking_info);
          let resolved = importer.resolve_star_exports(&self.graph.modules);
          importer_linking_info.resolved_star_exports = resolved;
        }
        Module::External(_) => {
          // meaningless
        }
      }
    }

    // Create resolved exports for export star
    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      let importer = &self.graph.modules[id];
      match importer {
        Module::Normal(importer) => {
          importer.create_resolved_exports_for_export_star(
            importer.id,
            linking_infos,
            &self.graph.modules,
            &mut Vec::default(),
          );
        }
        Module::External(_) => {}
      }
    });

    // Linking the module imports to resolved exports
    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      self.match_imports_with_exports(id, &mut symbols, linking_infos, &self.graph.modules);
    });

    // Exclude ambiguous from resolved exports
    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      let linking_info = &mut linking_infos[id];
      linking_info.create_exclude_ambiguous_resolved_exports(&symbols);
    });

    // Set the symbols back and add linker modules to graph
    self.graph.symbols = symbols;
  }

  #[allow(clippy::too_many_lines, clippy::manual_assert)]
  fn match_imports_with_exports(
    &self,
    id: ModuleId,
    symbols: &mut Symbols,
    linking_infos: &LinkingMetadataVec,
    modules: &ModuleVec,
  ) {
    let importer = &self.graph.modules[id];
    match importer {
      Module::Normal(importer) => {
        importer.named_imports.values().for_each(|info| {
          let import_record = &importer.import_records[info.record_id];
          let importee = &self.graph.modules[import_record.resolved_module];
          match importee {
            Module::Normal(importee) => {
              match Self::match_import_with_export(
                modules,
                importer,
                importee,
                &linking_infos[importee.id],
                &linking_infos[importer.id],
                info,
              ) {
                MatchImportKind::NotFound => panic!("info {info:#?}"),
                MatchImportKind::PotentiallyAmbiguous(
                  symbol_ref,
                  mut potentially_ambiguous_symbol_refs,
                ) => {
                  potentially_ambiguous_symbol_refs.push(symbol_ref);
                  if self
                    .determine_ambiguous_export(potentially_ambiguous_symbol_refs, linking_infos)
                  {
                    // ambiguous export
                    panic!("");
                  }
                  symbols.union(info.imported_as, symbol_ref);
                }
                MatchImportKind::Found(symbol_ref) => {
                  symbols.union(info.imported_as, symbol_ref);
                }
                MatchImportKind::Namespace(ns_ref) => match &info.imported {
                  Specifier::Star => {
                    symbols.union(info.imported_as, ns_ref);
                  }
                  Specifier::Literal(imported) => {
                    symbols.get_mut(info.imported_as).namespace_alias = Some(NamespaceAlias {
                      property_name: imported.clone(),
                      namespace_ref: ns_ref,
                    });
                  }
                },
              }
            }
            Module::External(_) => {}
          }
        });
      }
      Module::External(_) => {
        // It's meaningless to be a importer for a external module.
      }
    }
  }

  // Iterate all potentially ambiguous symbol refs, If all results not be same, it's a ambiguous export
  pub fn determine_ambiguous_export(
    &self,
    potentially_ambiguous_symbol_refs: Vec<SymbolRef>,
    linking_infos: &LinkingMetadataVec,
  ) -> bool {
    let modules = &self.graph.modules;
    let mut results = vec![];

    for symbol_ref in potentially_ambiguous_symbol_refs {
      match &modules[symbol_ref.owner] {
        Module::Normal(importer) => {
          let module_linking_info = &linking_infos[importer.id];
          if let Some(info) = importer.named_imports.get(&symbol_ref.symbol) {
            let importee_id = importer.import_records[info.record_id].resolved_module;
            match &modules[importee_id] {
              Module::Normal(importee) => {
                results.push(Self::match_import_with_export(
                  modules,
                  importer,
                  importee,
                  &linking_infos[importee_id],
                  module_linking_info,
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

  pub fn match_import_with_export(
    _modules: &ModuleVec,
    importer: &NormalModule,
    importee: &NormalModule,
    importee_linking_info: &LinkingMetadata,
    _importer_linking_info: &LinkingMetadata,
    info: &NamedImport,
  ) -> MatchImportKind {
    // If importee module is commonjs module, it will generate property access to namespace symbol
    // The namespace symbols should be importer created local symbol.
    if importee.exports_kind == ExportsKind::CommonJs {
      let rec = &importer.import_records[info.record_id];
      match info.imported {
        Specifier::Star => {
          return MatchImportKind::Found(rec.namespace_ref);
        }
        Specifier::Literal(_) => {
          return MatchImportKind::Namespace(rec.namespace_ref);
        }
      }
    }

    match &info.imported {
      Specifier::Star => {
        return MatchImportKind::Found(importee.namespace_symbol);
      }
      Specifier::Literal(imported) => {
        if let Some(resolved_export) = importee_linking_info.resolved_exports.get(imported) {
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
    if importee_linking_info.has_dynamic_exports {
      return MatchImportKind::Namespace(importee.namespace_symbol);
    }

    MatchImportKind::NotFound
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatchImportKind {
  NotFound,
  // The import symbol will generate property access to namespace symbol
  Namespace(SymbolRef),
  // External,
  PotentiallyAmbiguous(SymbolRef, Vec<SymbolRef>),
  Found(SymbolRef),
}
