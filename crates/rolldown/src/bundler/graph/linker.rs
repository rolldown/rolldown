use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{
  ExportsKind, ImportKind, LocalOrReExport, ModuleId, NamedImport, ResolvedExport, StmtInfo,
  StmtInfoId, SymbolRef, WrapKind,
};
use rustc_hash::FxHashMap;

use super::{graph::Graph, symbols::NamespaceAlias};
use crate::bundler::{
  graph::symbols::Symbols,
  module::{Module, ModuleVec, NormalModule},
};

/// Store the linking info for module
#[derive(Debug, Default)]
pub struct LinkingInfo {
  // The symbol for wrapped module
  pub wrap_symbol: Option<SymbolRef>,
  pub wrap_kind: WrapKind,
  pub facade_stmt_infos: Vec<StmtInfo>,
  // Convert `export { v } from "./a"` to `import { v } from "./a"; export { v }`.
  // It is used to prepare resolved exports generation.
  pub export_from_map: FxHashMap<Atom, NamedImport>,
  pub resolved_exports: FxHashMap<Atom, ResolvedExport>,
  pub exclude_ambiguous_resolved_exports: Vec<Atom>,
  pub resolved_star_exports: Vec<ModuleId>,
}

pub type LinkingInfoVec = IndexVec<ModuleId, LinkingInfo>;

pub struct Linker<'graph> {
  graph: &'graph mut Graph,
}

impl<'graph> Linker<'graph> {
  pub fn new(graph: &'graph mut Graph) -> Self {
    Self { graph }
  }

  fn include_statements(&mut self) {
    use rayon::prelude::*;
    struct Context<'a> {
      graph: &'a Graph,
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
    // propagate star exports
    for id in &self.graph.sorted_modules {
      let importer = &self.graph.modules[*id];
      match importer {
        Module::Normal(importer) => {
          let importer_linking_info = &mut linking_infos[*id];
          importer.add_initial_resolved_exports(importer_linking_info, &mut symbols);
          let resolved = importer.resolve_star_exports(&self.graph.modules);
          importer_linking_info.resolved_star_exports = resolved;
        }
        Module::External(_) => {
          // meaningless
        }
      }
    }
    // Mark namespace symbol for namespace referenced
    // Create symbols for external module
    self.mark_extra_symbols(&mut symbols, &mut linking_infos);

    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      let importer = &self.graph.modules[id];
      match importer {
        Module::Normal(importer) => {
          importer.add_resolved_exports_for_export_star(
            importer.id,
            &mut linking_infos,
            &self.graph.modules,
            &mut Vec::default(),
          );
        }
        Module::External(_) => {}
      }
    });

    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      self.match_imports_with_exports(id, &mut symbols, &mut linking_infos, &self.graph.modules);
    });

    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      let linking_info = &mut linking_infos[id];
      let mut export_names = linking_info
        .resolved_exports
        .iter()
        .filter_map(|(name, resolved_export)| {
          if let Some(v) = &resolved_export.potentially_ambiguous_symbol_refs {
            if is_ambiguous_export(resolved_export.symbol_ref, v, &symbols) {
              return None;
            }
          }
          Some(name.clone())
        })
        .collect::<Vec<_>>();
      export_names.sort_unstable_by_key(|s| s.to_string());
      linking_info.exclude_ambiguous_resolved_exports = export_names;
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
          importer.import_records.iter().for_each(|r| {
            let importee_linking_info = &linking_infos[r.resolved_module];
            let importee = &self.graph.modules[r.resolved_module];
            let Module::Normal(importee) = importee else {
              return;
            };
            if let Some(importee_warp_symbol) = importee_linking_info.wrap_symbol {
              let importer_linking_info = &mut linking_infos[importer.id];
              importer.reference_symbol_in_facade_stmt_infos(
                importee_warp_symbol,
                importer_linking_info,
                symbols,
              );
              importer.reference_symbol_in_facade_stmt_infos(
                importee.namespace_symbol,
                importer_linking_info,
                symbols,
              );
              match (importer.exports_kind, importee.exports_kind) {
                (ExportsKind::Esm, ExportsKind::CommonJs) => {
                  importer.reference_symbol_in_facade_stmt_infos(
                    self.graph.runtime.resolve_symbol(&"__toESM".into()),
                    importer_linking_info,
                    symbols,
                  );
                }
                (_, ExportsKind::Esm) => {
                  importer.reference_symbol_in_facade_stmt_infos(
                    self.graph.runtime.resolve_symbol(&"__toCommonJS".into()),
                    importer_linking_info,
                    symbols,
                  );
                }
                _ => {}
              }
            }
          });
          importer.star_export_modules().for_each(|id| match &self.graph.modules[id] {
            Module::Normal(importee) => {
              if importee.exports_kind == ExportsKind::CommonJs {
                importer.reference_symbol_in_facade_stmt_infos(
                  self.graph.runtime.resolve_symbol(&"__reExport".into()),
                  &mut linking_infos[importer.id],
                  symbols,
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
    if linking_info.wrap_symbol.is_some() {
      return;
    }

    // Generate symbol for wrap module declaration
    // Case commonjs, eg var require_a = __commonJS()
    // Case esm, eg var init_a = __esm()
    match &self.graph.modules[target] {
      Module::Normal(module) => {
        linking_info.wrap_kind =
          if module.exports_kind == ExportsKind::CommonJs { WrapKind::CJS } else { WrapKind::ESM };
        module.create_wrap_symbol(linking_info, symbols);

        let name = if module.exports_kind == ExportsKind::CommonJs {
          "__commonJS".into()
        } else {
          "__esm".into()
        };
        let runtime_symbol = self.graph.runtime.resolve_symbol(&name);
        module.reference_symbol_in_facade_stmt_infos(runtime_symbol, linking_info, symbols);
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
              extra_symbols.push((
                import_record.resolved_module,
                info.imported.clone(),
                info.is_imported_star,
              ));
            }
          });
          importer.named_exports.iter().for_each(|(_, export)| match &export {
            LocalOrReExport::Local(_) => {}
            LocalOrReExport::Re(re) => {
              let import_record = &importer.import_records[re.record_id];
              let importee = &self.graph.modules[import_record.resolved_module];
              if let Module::External(_) = importee {
                extra_symbols.push((
                  import_record.resolved_module,
                  re.imported.clone(),
                  re.is_imported_star,
                ));
              }
            }
          });
        }
        Module::External(_) => {}
      }
      extra_symbols.into_iter().for_each(|(importee, imported, is_imported_star)| {
        let importee = &mut self.graph.modules[importee];
        match importee {
          Module::Normal(_) => {}
          Module::External(importee) => {
            importee.add_export_symbol(symbols, imported, is_imported_star);
          }
        }
      });
    }
  }

  fn match_imports_with_exports(
    &self,
    id: ModuleId,
    symbols: &mut Symbols,
    linking_infos: &mut LinkingInfoVec,
    modules: &ModuleVec,
  ) {
    let importer = &self.graph.modules[id];
    match importer {
      Module::Normal(importer) => {
        let mut local_symbols = vec![];
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
                  info,
                  symbols,
                ) {
                  MatchImportKind::NotFound | MatchImportKind::Ambiguous => panic!(""),
                  MatchImportKind::Found(symbol_ref) => {
                    symbols.union(info.imported_as, symbol_ref);
                  }
                  MatchImportKind::NameSpace => {
                    symbols.get_mut(info.imported_as).namespace_alias = Some(NamespaceAlias {
                      property_name: info.imported.clone(),
                      namespace_ref: importee.namespace_symbol,
                    });
                    local_symbols.push(importee.namespace_symbol);
                  }
                }
              }
              Module::External(importee) => {
                let resolved_ref = importee.resolve_export(&info.imported, info.is_imported_star);
                symbols.union(info.imported_as, resolved_ref);
              }
            }
          });

        local_symbols.into_iter().for_each(|symbol_ref| {
          importer.reference_symbol_in_facade_stmt_infos(
            symbol_ref,
            &mut linking_infos[importer.id],
            symbols,
          );
        });
      }
      Module::External(_) => {
        // It's meaningless to be a importer for a external module.
      }
    }
  }

  pub fn match_import_with_export(
    modules: &ModuleVec,
    importee: &NormalModule,
    importee_linking_info: &LinkingInfo,
    info: &NamedImport,
    symbols: &Symbols,
  ) -> MatchImportKind {
    if info.is_imported_star {
      return MatchImportKind::Found(importee.namespace_symbol);
    }

    if importee.exports_kind == ExportsKind::CommonJs {
      return MatchImportKind::NameSpace;
    }

    if let Some(resolved_export) = importee_linking_info.resolved_exports.get(&info.imported) {
      if let Some(potentially_ambiguous_export) = &resolved_export.potentially_ambiguous_symbol_refs
      {
        if is_ambiguous_export(resolved_export.symbol_ref, potentially_ambiguous_export, symbols) {
          return MatchImportKind::Ambiguous;
        }
      }
      if let Some(id) = resolved_export.export_from {
        let module = &modules[id];
        match module {
          Module::Normal(module) => {
            if module.exports_kind == ExportsKind::CommonJs {
              return MatchImportKind::NameSpace;
            }
          }
          Module::External(_) => {}
        }
      }
      return MatchImportKind::Found(resolved_export.symbol_ref);
    }

    if importee
      .star_export_modules()
      .map(|id| {
        let importee = &modules[id];
        match importee {
          Module::Normal(importee) => importee.exports_kind == ExportsKind::CommonJs,
          Module::External(_) => false,
        }
      })
      .any(|is_cjs| is_cjs)
    {
      return MatchImportKind::NameSpace;
    }

    MatchImportKind::NotFound
  }
}

pub fn is_ambiguous_export(
  symbol_ref: SymbolRef,
  potentially_ambiguous_export: &Vec<SymbolRef>,
  symbols: &Symbols,
) -> bool {
  for export in potentially_ambiguous_export {
    if symbol_ref != symbols.par_canonical_ref_for(*export) {
      return true;
    }
  }
  false
}

#[derive(Debug)]
pub enum MatchImportKind {
  NotFound,
  // The import symbol will generate property access to namespace symbol
  NameSpace,
  Ambiguous,
  Found(SymbolRef),
}
