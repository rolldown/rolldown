use index_vec::IndexVec;
use rolldown_common::{
  ExportsKind, ImportKind, LocalOrReExport, ModuleId, NamedImport, Specifier, StmtInfoId,
  SymbolRef, WrapKind,
};
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

  fn include_statements(&mut self) {
    use rayon::prelude::*;
    struct Context<'a> {
      graph: &'a LinkStageOutput,
      is_included_vec: &'a mut IndexVec<ModuleId, IndexVec<StmtInfoId, bool>>,
    }

    fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
      let mut canonical_ref = ctx.graph.symbols.par_canonical_ref_for(symbol_ref);
      let canonical_ref_module = &ctx.graph.modules[canonical_ref.owner];
      let canonical_ref_symbol = ctx.graph.symbols.get(canonical_ref);
      if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
        canonical_ref = namespace_alias.namespace_ref;
      }
      let Module::Normal(canonical_ref_module) = canonical_ref_module else {
        return;
      };
      canonical_ref_module
        .stmt_infos
        .declared_stmts_by_symbol(&canonical_ref)
        .iter()
        .copied()
        .for_each(|stmt_info_id| {
          include_statement(ctx, canonical_ref_module, stmt_info_id);
        });
    }

    fn include_statement(ctx: &mut Context, module: &NormalModule, stmt_info_id: StmtInfoId) {
      let is_included = &mut ctx.is_included_vec[module.id][stmt_info_id];
      if *is_included {
        return;
      }

      // include the statement itself
      *is_included = true;

      let stmt_info = module.stmt_infos.get(stmt_info_id);

      // include statements that are referenced by this statement
      stmt_info.declared_symbols.iter().chain(stmt_info.referenced_symbols.iter()).for_each(
        |symbol_ref| {
          include_symbol(ctx, *symbol_ref);
        },
      );
    }

    let mut is_included_vec: IndexVec<ModuleId, IndexVec<StmtInfoId, bool>> = self
      .graph
      .modules
      .iter()
      .map(|m| match m {
        Module::Normal(m) => {
          m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoId, _>>()
        }
        Module::External(_) => IndexVec::default(),
      })
      .collect::<IndexVec<ModuleId, _>>();

    let context = &mut Context { graph: self.graph, is_included_vec: &mut is_included_vec };

    for module in &self.graph.modules {
      match module {
        Module::Normal(module) => {
          let mut stmt_infos = module.stmt_infos.iter_enumerated();
          // Skip the first one, because it's the namespace variable declaration.
          // We want to include it on demand.
          stmt_infos.next();
          // Since we won't implement tree shaking, we just include all statements.
          stmt_infos.for_each(|(stmt_info_id, _)| {
            include_statement(context, module, stmt_info_id);
          });
          if module.is_entry {
            let linking_info = &self.graph.linking_infos[module.id];
            linking_info.resolved_exports.values().for_each(|resolved_export| {
              include_symbol(context, resolved_export.symbol_ref);
            });
          }
        }
        Module::External(_) => {}
      }
    }
    self.graph.modules.iter_mut().par_bridge().for_each(|module| {
      let Module::Normal(module) = module else {
        return;
      };
      is_included_vec[module.id].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
        module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
      });
    });
  }

  pub fn link(&mut self) {
    // Here take the symbols to avoid borrow graph and mut borrow graph at same time
    let mut symbols = std::mem::take(&mut self.graph.symbols);
    // Here add linker module for each module to avoid borrow module and mut borrow module at same time
    let mut linking_infos = IndexVec::from_vec(
      self.graph.modules.iter().map(|_| LinkingInfo::default()).collect::<Vec<_>>(),
    );

    self.mark_module_wrapped(&mut symbols, &mut linking_infos);

    // Create symbols for external module
    self.mark_extra_symbols(&mut symbols, &mut linking_infos);

    // Propagate star exports
    // Create resolved exports for named export declarations
    // Mark dynamic exports due to export star
    for id in &self.graph.sorted_modules {
      let importer = &self.graph.modules[*id];
      match importer {
        Module::Normal(importer) => {
          self.mark_dynamic_exports_due_to_export_star(
            *id,
            &mut linking_infos,
            &mut FxHashSet::default(),
          );
          let importer_linking_info = &mut linking_infos[*id];
          importer.create_initial_resolved_exports(importer_linking_info, &mut symbols);
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
            &mut linking_infos,
            &self.graph.modules,
            &mut Vec::default(),
          );
        }
        Module::External(_) => {}
      }
    });

    // Linking the module imports to resolved exports
    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      self.match_imports_with_exports(id, &mut symbols, &linking_infos, &self.graph.modules);
    });

    // Exclude ambiguous from resolved exports
    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      let linking_info = &mut linking_infos[id];
      linking_info.create_exclude_ambiguous_resolved_exports(&symbols);
    });

    // Set the symbols back and add linker modules to graph
    self.graph.symbols = symbols;

    // FIXME: should move `linking_info.facade_stmt_infos` into a separate field
    for (id, linking_info) in linking_infos.iter_mut_enumerated() {
      std::mem::take(&mut linking_info.facade_stmt_infos).into_iter().for_each(|info| {
        if let Module::Normal(module) = &mut self.graph.modules[id] {
          module.stmt_infos.add_stmt_info(info);
        }
      });
    }

    self.graph.linking_infos = linking_infos;

    self.include_statements();
  }

  #[allow(clippy::too_many_lines)]
  fn mark_module_wrapped(&self, symbols: &mut Symbols, linking_infos: &mut LinkingInfoVec) {
    // Detect module need wrapped, here has two cases:
    // - Commonjs module, because cjs symbols can't static binding, it need to be wrapped and lazy evaluated.
    // - Import esm module at commonjs module.
    for module in &self.graph.modules {
      match module {
        Module::Normal(module) => {
          if module.exports_kind == ExportsKind::CommonJs {
            self.wrap_module(module.id, symbols, linking_infos);
          } else {
            // Should mark wrapped for require import module
            module.import_records.iter().for_each(|record| {
              if record.kind == ImportKind::Require {
                self.wrap_module(record.resolved_module, symbols, linking_infos);
              }
            });
          }
        }
        Module::External(_) => {}
      }
    }

    // Generate symbol for import warp module
    // Case esm import commonjs, eg var commonjs_ns = __toESM(require_a())
    // Case commonjs require esm, eg (init_esm(), __toCommonJS(esm_ns))
    // Case esm export star commonjs, eg __reExport(esm_ns, __toESM(require_a())
    for module in &self.graph.modules {
      match module {
        Module::Normal(importer) => {
          importer.static_imports().for_each(|r| {
            let importee_linking_info =
              &linking_infos.get(r.resolved_module).unwrap_or_else(|| {
                panic!("importer: {:?}, importee_linking_info {:#?}", importer.resource_id, r,)
              });
            let importee = &self.graph.modules[r.resolved_module];
            let Module::Normal(importee) = importee else {
              return;
            };
            if let Some(importee_warp_symbol) = importee_linking_info.wrap_ref {
              let importer_linking_info = &mut linking_infos[importer.id];
              importer_linking_info.reference_symbol_in_facade_stmt_infos(importee_warp_symbol);
              match (importer.exports_kind, importee.exports_kind) {
                (ExportsKind::Esm, ExportsKind::CommonJs) => {
                  importer.create_local_symbol_for_import_cjs(
                    importee,
                    importer_linking_info,
                    symbols,
                  );
                  importer_linking_info.reference_symbol_in_facade_stmt_infos(
                    self.graph.runtime.resolve_symbol(&"__toESM".into()),
                  );
                }
                (_, ExportsKind::Esm) => {
                  importer_linking_info
                    .reference_symbol_in_facade_stmt_infos(importee.namespace_symbol);
                  importer_linking_info.reference_symbol_in_facade_stmt_infos(
                    self.graph.runtime.resolve_symbol(&"__toCommonJS".into()),
                  );
                }
                _ => {}
              }
            }
          });
          importer.star_export_modules().for_each(|id| match &self.graph.modules[id] {
            Module::Normal(importee) => {
              if importee.exports_kind == ExportsKind::CommonJs {
                let importee_linking_info = &mut linking_infos[importer.id];
                importee_linking_info.reference_symbol_in_facade_stmt_infos(
                  self.graph.runtime.resolve_symbol(&"__reExport".into()),
                );
              }
            }
            Module::External(_) => {}
          });
        }
        Module::External(_) => {}
      }
    }
  }

  fn wrap_module(
    &self,
    target: ModuleId,
    symbols: &mut Symbols,
    linking_infos: &mut LinkingInfoVec,
  ) {
    let linking_info = &mut linking_infos[target];
    if linking_info.wrap_ref.is_some() {
      return;
    }

    // Generate symbol for wrap module declaration
    // Case commonjs, eg var require_a = __commonJS()
    // Case esm, eg var init_a = __esm()
    match &self.graph.modules[target] {
      Module::Normal(module) => {
        linking_info.wrap_kind =
          if module.exports_kind == ExportsKind::CommonJs { WrapKind::Cjs } else { WrapKind::Esm };
        module.create_wrap_symbol(linking_info, symbols);

        let name = if module.exports_kind == ExportsKind::CommonJs {
          "__commonJS".into()
        } else {
          "__esm".into()
        };
        let runtime_symbol = self.graph.runtime.resolve_symbol(&name);
        linking_info.reference_symbol_in_facade_stmt_infos(runtime_symbol);
        module.import_records.iter().for_each(|record| {
          self.wrap_module(record.resolved_module, symbols, linking_infos);
        });
      }
      Module::External(_) => {}
    }
  }

  #[allow(clippy::needless_collect)]
  fn mark_extra_symbols(&mut self, symbols: &mut Symbols, _linking_infos: &mut LinkingInfoVec) {
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
          importer.named_exports.iter().for_each(|(_, export)| match &export {
            LocalOrReExport::Local(_) => {}
            LocalOrReExport::Re(re) => {
              let import_record = &importer.import_records[re.record_id];
              let importee = &self.graph.modules[import_record.resolved_module];
              if let Module::External(_) = importee {
                extra_symbols.push((import_record.resolved_module, re.imported.clone()));
              }
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
        Module::Normal(module) => {
          let module_linking_info = &linking_infos[module.id];
          if let Some(info) = module.named_imports.get(&symbol_ref.symbol) {
            let importee_id = module.import_records[info.record_id].resolved_module;
            match &modules[importee_id] {
              Module::Normal(importee) => {
                results.push(Self::match_import_with_export(
                  modules,
                  importee,
                  &linking_infos[importee_id],
                  module_linking_info,
                  info,
                ));
              }
              Module::External(_) => {}
            }
          } else if let Some(info) = module_linking_info.export_from_map.get(&symbol_ref.symbol) {
            let importee_id = module.import_records[info.record_id].resolved_module;
            match &modules[importee_id] {
              Module::Normal(importee) => {
                results.push(Self::match_import_with_export(
                  modules,
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
    modules: &ModuleVec,
    importee: &NormalModule,
    importee_linking_info: &LinkingInfo,
    importer_linking_info: &LinkingInfo,
    info: &NamedImport,
  ) -> MatchImportKind {
    // If importee module is commonjs module, it will generate property access to namespace symbol
    // The namespace symbols should be importer created local symbol.
    if importee.exports_kind == ExportsKind::CommonJs {
      return MatchImportKind::Namespace(
        importer_linking_info.local_symbol_for_import_cjs[&importee.id],
      );
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
          if let Some(id) = resolved_export.export_from {
            let module = &modules[id];
            match module {
              Module::Normal(module) => {
                if module.exports_kind == ExportsKind::CommonJs {
                  return MatchImportKind::Namespace(
                    importer_linking_info.local_symbol_for_import_cjs[&module.id],
                  );
                }
              }
              Module::External(_) => {}
            }
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
