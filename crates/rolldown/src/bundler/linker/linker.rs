use rolldown_common::{ExportsKind, ModuleId, NamedImport, Specifier, SymbolRef};
use rustc_hash::FxHashSet;

use super::linker_info::{LinkingInfo, LinkingInfoVec};
use crate::bundler::{
  module::{Module, ModuleVec, NormalModule},
  stages::link_stage::LinkStage as LinkStageOutput,
  utils::symbols::{NamespaceAlias, Symbols},
};

pub struct Linker<'graph> {
  graph: &'graph mut LinkStageOutput,
}

impl<'graph> Linker<'graph> {
  pub fn new(graph: &'graph mut LinkStageOutput) -> Self {
    Self { graph }
  }

  pub fn link(&mut self, linking_infos: &mut LinkingInfoVec) {
    // Here take the symbols to avoid borrow graph and mut borrow graph at same time
    let mut symbols = std::mem::take(&mut self.graph.symbols);

    // Create symbols for external module
    self.initialize_symbols_for_external_modules(&mut symbols);

    // Propagate star exports
    // Create resolved exports for named export declarations
    // Mark dynamic exports due to export star
    for id in &self.graph.sorted_modules {
      let importer = &self.graph.modules[*id];
      match importer {
        Module::Normal(importer) => {
          self.mark_dynamic_exports_due_to_export_star(
            *id,
            linking_infos,
            &mut FxHashSet::default(),
          );
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

  // TODO(hyf0): Probably should take a look when improving supporting external modules
  fn initialize_symbols_for_external_modules(&mut self, symbols: &mut Symbols) {
    for importer_id in &self.graph.sorted_modules {
      let importer = &self.graph.modules[*importer_id];

      // Create symbols for external module
      let mut extra_symbols = vec![];
      match importer {
        Module::Normal(importer) => {
          importer.named_imports.iter().for_each(|(_id, info)| {
            let import_record = &importer.import_records[info.record_id];
            let importee = &self.graph.modules[import_record.resolved_module];
            if let Module::External(_) = importee {
              extra_symbols.push((import_record.resolved_module, info.imported.clone()));
            }
          });
        }
        Module::External(_) => {}
      }
      extra_symbols.into_iter().for_each(|(importee, imported)| {
        let importee = &mut self.graph.modules[importee];
        match importee {
          Module::Normal(_) => {}
          Module::External(importee) => {
            importee.add_export_symbol(symbols, imported);
          }
        }
      });
    }
  }

  #[allow(clippy::too_many_lines, clippy::manual_assert)]
  fn match_imports_with_exports(
    &self,
    id: ModuleId,
    symbols: &mut Symbols,
    linking_infos: &LinkingInfoVec,
    modules: &ModuleVec,
  ) {
    let importer = &self.graph.modules[id];
    match importer {
      Module::Normal(importer) => {
        importer
          .named_imports
          .values()
          .chain(linking_infos[importer.id].export_from_map.values())
          .for_each(|info| {
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
              Module::External(importee) => {
                let resolved_ref = importee.resolve_export(&info.imported);
                symbols.union(info.imported_as, resolved_ref);
              }
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
    linking_infos: &LinkingInfoVec,
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
          } else if let Some(info) = module_linking_info.export_from_map.get(&symbol_ref.symbol) {
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
    importee_linking_info: &LinkingInfo,
    _importer_linking_info: &LinkingInfo,
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

  pub fn mark_dynamic_exports_due_to_export_star(
    &self,
    target: ModuleId,
    linking_infos: &mut LinkingInfoVec,
    visited_modules: &mut FxHashSet<ModuleId>,
  ) -> bool {
    if visited_modules.contains(&target) {
      return false;
    }
    visited_modules.insert(target);

    let module = &self.graph.modules[target];
    match module {
      Module::Normal(module) => {
        if module.exports_kind == ExportsKind::CommonJs || linking_infos[target].has_dynamic_exports
        {
          return true;
        }
        for id in module.star_export_modules() {
          if self.mark_dynamic_exports_due_to_export_star(id, linking_infos, visited_modules) {
            let module_linking_info = &mut linking_infos[target];
            module_linking_info.has_dynamic_exports = true;
            // Dynamic exports will generate `__reExport(ns, xx)`, here should reference self namespace symbol
            module_linking_info.reference_symbol_in_facade_stmt_infos(module.namespace_symbol);
            return true;
          }
        }
      }
      Module::External(_) => {}
    }
    false
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
