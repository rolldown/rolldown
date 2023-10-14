use index_vec::IndexVec;
use oxc::{
  semantic::{ReferenceId, SymbolId},
  span::Atom,
};
use rolldown_common::{LocalOrReExport, ModuleId, ModuleResolution, ResolvedExport, SymbolRef};
use rustc_hash::{FxHashMap, FxHashSet};

use super::graph::Graph;
use crate::bundler::{
  graph::symbols::Symbols,
  module::{module::Module, normal_module::Resolution, NormalModule},
};

pub struct Linker<'graph> {
  graph: &'graph mut Graph,
}

impl<'graph> Linker<'graph> {
  pub fn new(graph: &'graph mut Graph) -> Self {
    Self { graph }
  }

  pub fn link(&mut self) {
    self.create_module_wrap_symbol_and_reference();

    // propagate star exports
    for id in &self.graph.sorted_modules {
      let importer = &self.graph.modules[*id];
      match importer {
        Module::Normal(importer) => {
          let resolved = importer.resolve_star_exports(&self.graph.modules);
          self.graph.modules[*id]
            .expect_normal_mut()
            .resolved_star_exports = resolved;
        }
        Module::External(_) => {
          // meaningless
        }
      }
    }
    // Mark namespace symbol for namespace referenced
    // Create symbols for external module
    // Create symbols for import cjs module
    Self::mark_extra_symbols(self.graph);

    self
      .graph
      .sorted_modules
      .clone()
      .into_iter()
      .for_each(|id| {
        self.resolve_exports(id);
        self.resolve_imports(id);
      })
  }

  fn create_module_wrap_symbol_and_reference(&mut self) {
    let mut modules_wrap = index_vec::index_vec![
      FxHashSet::default();
      self.graph.modules.len()
    ];
    for (_, module_id) in &self.graph.entries {
      match &self.graph.modules[*module_id] {
        Module::Normal(importer) => {
          importer.import_records.iter().for_each(|r| {
            let importee = &self.graph.modules[r.resolved_module];
            if let Module::Normal(importee) = importee {
              if importee.module_resolution == ModuleResolution::CommonJs {
                mark_module_wrap_and_reference(
                  self.graph,
                  importer.id,
                  r.resolved_module,
                  &mut modules_wrap,
                );
              }
            }
          });
        }
        Module::External(_) => {}
      }
    }

    let runtime_esm_symbol = self.graph.runtime.resolve_symbol_id(&"__esm".into());
    let runtime_commonjs_symbol = self.graph.runtime.resolve_symbol_id(&"__commonJS".into());
    let runtime_to_esm_symbol = self.graph.runtime.resolve_symbol_id(&"__toESM".into());
    let runtime_to_commonjs_symbol = self.graph.runtime.resolve_symbol_id(&"__toCommonJS".into());

    for (module_id, importers) in modules_wrap.into_iter_enumerated() {
      if !importers.is_empty() {
        match &mut self.graph.modules[module_id] {
          Module::Normal(importee) => {
            importee.create_wrap_symbol(&mut self.graph.symbols);

            create_runtime_warp_symbol_reference(
              importee,
              &mut self.graph.symbols,
              runtime_esm_symbol,
              runtime_commonjs_symbol,
            );
          }
          Module::External(_) => {}
        }
        match &self.graph.modules[module_id] {
          Module::Normal(importee) => {
            importers.into_iter().for_each(|id| {
              self.graph.symbols.tables[id].create_reference(importee.wrap_symbol);

              create_runtime_interop_symbol_reference(
                self.graph.modules[id].expect_normal(),
                importee,
                &mut self.graph.symbols,
                runtime_to_esm_symbol,
                runtime_to_commonjs_symbol,
              );
            });
          }
          Module::External(_) => {}
        }
      }
    }

    fn mark_module_wrap_and_reference(
      graph: &Graph,
      importer: ModuleId,
      importee: ModuleId,
      modules_wrap: &mut IndexVec<ModuleId, FxHashSet<ModuleId>>,
    ) {
      match &graph.modules[importee] {
        Module::Normal(module) => {
          if modules_wrap[importee].contains(&importer) {
            return;
          }
          modules_wrap[importee].insert(importer);
          module.import_records.iter().for_each(|record| {
            mark_module_wrap_and_reference(graph, importee, record.resolved_module, modules_wrap);
          });
        }
        Module::External(_) => {}
      }
    }

    fn create_runtime_warp_symbol_reference(
      module: &NormalModule,
      symbols: &mut Symbols,
      runtime_esm_symbol: SymbolId,
      runtime_commonjs_symbol: SymbolId,
    ) {
      if module.module_resolution == ModuleResolution::CommonJs {
        symbols.tables[module.id].create_reference(Some(runtime_commonjs_symbol));
      } else {
        symbols.tables[module.id].create_reference(Some(runtime_esm_symbol));
      }
    }

    fn create_runtime_interop_symbol_reference(
      importer: &NormalModule,
      importee: &NormalModule,
      symbols: &mut Symbols,
      runtime_to_esm_symbol: SymbolId,
      runtime_to_commonjs_symbol: SymbolId,
    ) {
      if importer.module_resolution == ModuleResolution::Esm
        && importee.module_resolution == ModuleResolution::CommonJs
      {
        symbols.tables[importer.id].create_reference(Some(runtime_to_esm_symbol));
      } else if importer.module_resolution == ModuleResolution::CommonJs
        && importee.module_resolution == ModuleResolution::Esm
      {
        symbols.tables[importer.id].create_reference(Some(runtime_to_commonjs_symbol));
      }
    }
  }

  fn mark_extra_symbols(graph: &mut Graph) {
    for id in &graph.sorted_modules {
      let importer = &graph.modules[*id];
      let importee_list = importer
        .import_records()
        .iter()
        .filter_map(|rec| {
          (rec.is_import_namespace && rec.resolved_module.is_valid()).then_some(rec.resolved_module)
        })
        .collect::<Vec<_>>();

      // Create symbols for external module
      // Create symbols for import cjs module
      let mut extra_symbols = vec![];
      match importer {
        Module::Normal(importer) => {
          importer.named_imports.iter().for_each(|(_id, info)| {
            let import_record = &importer.import_records[info.record_id];
            let importee = &graph.modules[import_record.resolved_module];
            match importee {
              Module::Normal(importee) => {
                if importee.module_resolution == ModuleResolution::CommonJs {
                  extra_symbols.push((
                    import_record.resolved_module,
                    info.imported.clone(),
                    info.is_imported_star,
                  ));
                }
              }
              Module::External(_) => {
                extra_symbols.push((
                  import_record.resolved_module,
                  info.imported.clone(),
                  info.is_imported_star,
                ));
              }
            }
          });
          importer
            .named_exports
            .iter()
            .for_each(|(_, export)| match &export {
              LocalOrReExport::Local(_) => {}
              LocalOrReExport::Re(re) => {
                let import_record = &importer.import_records[re.record_id];
                let importee = &graph.modules[import_record.resolved_module];
                if let Module::External(_) = importee {
                  extra_symbols.push((
                    import_record.resolved_module,
                    re.imported.clone(),
                    re.is_imported_star,
                  ));
                }
              }
            })
        }
        Module::External(_) => {}
      }
      extra_symbols
        .into_iter()
        .for_each(|(importee, imported, is_imported_star)| {
          let importee = &mut graph.modules[importee];
          match importee {
            Module::Normal(importee) => {
              if importee.module_resolution == ModuleResolution::CommonJs {
                importee.add_cjs_symbol(&mut graph.symbols, imported, is_imported_star)
              }
            }
            Module::External(importee) => {
              importee.add_export_symbol(&mut graph.symbols, imported, is_imported_star);
            }
          }
        });

      importee_list.into_iter().for_each(|importee| {
        graph.modules[importee].mark_symbol_for_namespace_referenced();
      });
    }
  }

  fn resolve_exports(&mut self, id: ModuleId) {
    let importer = &self.graph.modules[id];
    match importer {
      crate::bundler::module::module::Module::Normal(importer) => {
        let exported_names =
          importer.get_exported_names(&mut Default::default(), &self.graph.modules);

        let mut resolutions = exported_names
          .iter()
          .map(|exported| {
            (
              *exported,
              importer.resolve_export(
                exported,
                &mut Default::default(),
                &self.graph.modules,
                &mut self.graph.symbols,
              ),
            )
          })
          .collect::<FxHashMap<_, _>>();

        let mut exported_name_to_local_symbol: FxHashMap<Atom, ResolvedExport> = Default::default();

        fn create_local_symbol_and_reference(
          symbol_ref: SymbolRef,
          exporter: ModuleId,
          symbols: &mut Symbols,
        ) -> (SymbolRef, ReferenceId) {
          let local_symbol = if symbol_ref.owner == exporter {
            symbol_ref.symbol
          } else {
            symbols.tables[exporter].create_symbol(Atom::from("#FACADE#"))
          };
          let symbol_ref_of_local: SymbolRef = (exporter, local_symbol).into();
          symbols.union(symbol_ref_of_local, symbol_ref);
          let ref_id = symbols.tables[exporter].create_reference(Some(local_symbol));

          (symbol_ref_of_local, ref_id)
        }

        importer.named_exports.keys().for_each(|exported| {
          let res = resolutions.remove(exported).unwrap();
          match res {
            Resolution::None => panic!(
              "named export {exported:?} must be resolved for exporter: {:?}",
              importer.resource_id
            ),
            Resolution::Ambiguous => panic!("named export must be resolved"),
            Resolution::Found(ext) => {
              let tmp =
                create_local_symbol_and_reference(ext, importer.id, &mut self.graph.symbols);
              exported_name_to_local_symbol.insert(
                exported.clone(),
                ResolvedExport {
                  local_symbol: tmp.0,
                  local_ref: tmp.1,
                },
              );
            }
          }
        });

        resolutions
          .into_iter()
          .for_each(|(exported, left)| match left {
            Resolution::None => panic!("shouldn't has left which is None"),
            Resolution::Found(ext) => {
              let tmp =
                create_local_symbol_and_reference(ext, importer.id, &mut self.graph.symbols);
              exported_name_to_local_symbol.insert(
                exported.clone(),
                ResolvedExport {
                  local_symbol: tmp.0,
                  local_ref: tmp.1,
                },
              );
            }
            Resolution::Ambiguous => {}
          });

        match &mut self.graph.modules[id] {
          Module::Normal(importer) => {
            importer.resolved_exports = exported_name_to_local_symbol;
          }
          Module::External(_) => unreachable!(),
        };
      }
      crate::bundler::module::module::Module::External(_) => {
        // TODO: handle external module
      }
    }
  }
  fn resolve_imports(&mut self, id: ModuleId) {
    let importer = &self.graph.modules[id];
    match importer {
      Module::Normal(importer) => {
        importer.named_imports.iter().for_each(|(_id, info)| {
          let import_record = &importer.import_records[info.record_id];
          let importee = &self.graph.modules[import_record.resolved_module];
          match importee {
            Module::Normal(importee) => {
              let resolved_ref = if importee.module_resolution == ModuleResolution::CommonJs {
                importee.resolve_cjs_symbol(&info.imported, info.is_imported_star)
              } else if info.is_imported_star {
                importee.namespace_symbol.0
              } else {
                match importee.resolve_export(
                  &info.imported,
                  &mut Default::default(),
                  &self.graph.modules,
                  &mut self.graph.symbols,
                ) {
                  Resolution::None => panic!(""),
                  Resolution::Ambiguous => panic!(""),
                  Resolution::Found(founded) => founded,
                }
              };
              self.graph.symbols.union(info.imported_as, resolved_ref);
            }
            Module::External(importee) => {
              let resolved_ref = importee.resolve_export(&info.imported, info.is_imported_star);
              self.graph.symbols.union(info.imported_as, resolved_ref);
            }
          }
        });
      }
      Module::External(_) => {
        // It's meaningless to be a importer for a external module.
      }
    }
  }
}
