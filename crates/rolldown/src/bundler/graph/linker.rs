use index_vec::IndexVec;
use oxc::{semantic::ReferenceId, span::Atom};
use rolldown_common::{LocalOrReExport, ModuleId, ModuleResolution, ResolvedExport, SymbolRef};
use rustc_hash::FxHashMap;

use super::graph::Graph;
use crate::bundler::{
  graph::symbols::Symbols,
  module::{module::Module, normal_module::Resolution},
};

pub struct Linker<'graph> {
  graph: &'graph mut Graph,
}

impl<'graph> Linker<'graph> {
  pub fn new(graph: &'graph mut Graph) -> Self {
    Self { graph }
  }

  pub fn link(&mut self) {
    self.wrap_modules_if_needed();

    // propagate star exports
    for id in &self.graph.sorted_modules {
      let importer = &self.graph.modules[*id];
      match importer {
        Module::Normal(importer) => {
          let resolved = importer.resolve_star_exports(&self.graph.modules);
          self.graph.modules[*id].expect_normal_mut().resolved_star_exports = resolved;
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

    self.graph.sorted_modules.clone().into_iter().for_each(|id| {
      self.resolve_exports(id);
      self.resolve_imports(id);
    });
  }

  fn wrap_modules_if_needed(&mut self) {
    let mut module_to_wrapped = index_vec::index_vec![
      false;
      self.graph.modules.len()
    ];

    for module in &self.graph.modules {
      match module {
        Module::Normal(module) => {
          if module.module_resolution == ModuleResolution::CommonJs {
            wrap_module(self.graph, module.id, &mut module_to_wrapped);
          }
        }
        Module::External(_) => {}
      }
    }

    for (module_id, wrapped) in module_to_wrapped.into_iter_enumerated() {
      if wrapped {
        match &mut self.graph.modules[module_id] {
          Module::Normal(module) => {
            module.create_wrap_symbol(&mut self.graph.symbols);
            let name = if module.module_resolution == ModuleResolution::CommonJs {
              "__commonJS".into()
            } else {
              "__esm".into()
            };
            let runtime_symbol = self.graph.runtime.resolve_symbol(&name);
            module.generate_symbol_import_and_use(&mut self.graph.symbols, runtime_symbol);
          }
          Module::External(_) => {}
        };
      }
    }

    let mut imported_symbols = vec![];

    for module in &self.graph.modules {
      match module {
        Module::Normal(importer) => {
          importer.import_records.iter().for_each(|r| {
            let importee = &self.graph.modules[r.resolved_module];
            let Module::Normal(importee) = importee else { return };

            if let Some(importee_warp_symbol) = importee.wrap_symbol {
              imported_symbols.push((importer.id, importee_warp_symbol));
            }

            match (importer.module_resolution, importee.module_resolution) {
              (ModuleResolution::Esm, ModuleResolution::CommonJs) => {
                imported_symbols
                  .push((importer.id, self.graph.runtime.resolve_symbol(&"__toESM".into())));
              }
              (ModuleResolution::CommonJs, ModuleResolution::Esm) => {
                imported_symbols
                  .push((importer.id, self.graph.runtime.resolve_symbol(&"__toCommonJS".into())));
              }
              _ => {}
            }
          });
        }
        Module::External(_) => {}
      }
    }

    for (module, symbol) in imported_symbols {
      let importer = &mut self.graph.modules[module];
      match importer {
        Module::Normal(importer) => {
          importer.generate_symbol_import_and_use(&mut self.graph.symbols, symbol);
        }
        Module::External(_) => {}
      }
    }

    #[allow(clippy::items_after_statements)]
    fn wrap_module(
      graph: &Graph,
      target: ModuleId,
      module_to_wrapped: &mut IndexVec<ModuleId, bool>,
    ) {
      if module_to_wrapped[target] {
        return;
      }

      match &graph.modules[target] {
        Module::Normal(module) => {
          module_to_wrapped[target] = true;
          module.import_records.iter().for_each(|record| {
            wrap_module(graph, record.resolved_module, module_to_wrapped);
          });
        }
        Module::External(_) => {}
      }
    }
  }

  #[allow(clippy::needless_collect)]
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
          importer.named_exports.iter().for_each(|(_, export)| match &export {
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
          });
        }
        Module::External(_) => {}
      }
      extra_symbols.into_iter().for_each(|(importee, imported, is_imported_star)| {
        let importee = &mut graph.modules[importee];
        match importee {
          Module::Normal(importee) => {
            if importee.module_resolution == ModuleResolution::CommonJs {
              importee.add_cjs_symbol(&mut graph.symbols, imported, is_imported_star);
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
        let exported_names = importer.get_exported_names(&mut Vec::default(), &self.graph.modules);

        let mut resolutions = exported_names
          .iter()
          .map(|exported| {
            (
              *exported,
              importer.resolve_export(
                exported,
                &mut Vec::default(),
                &self.graph.modules,
                &mut self.graph.symbols,
              ),
            )
          })
          .collect::<FxHashMap<_, _>>();

        let mut exported_name_to_local_symbol: FxHashMap<Atom, ResolvedExport> =
          FxHashMap::default();

        #[allow(clippy::items_after_statements)]
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
            Resolution::None => {
              panic!(
                "named export {exported:?} must be resolved for exporter: {:?}",
                importer.resource_id
              )
            }
            Resolution::Ambiguous => panic!("named export must be resolved"),
            Resolution::Found(ext) => {
              let tmp =
                create_local_symbol_and_reference(ext, importer.id, &mut self.graph.symbols);
              exported_name_to_local_symbol
                .insert(exported.clone(), ResolvedExport { local_symbol: tmp.0, local_ref: tmp.1 });
            }
          }
        });

        resolutions.into_iter().for_each(|(exported, left)| match left {
          Resolution::None => panic!("shouldn't has left which is None"),
          Resolution::Found(ext) => {
            let tmp = create_local_symbol_and_reference(ext, importer.id, &mut self.graph.symbols);
            exported_name_to_local_symbol
              .insert(exported.clone(), ResolvedExport { local_symbol: tmp.0, local_ref: tmp.1 });
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
                  &mut Vec::default(),
                  &self.graph.modules,
                  &mut self.graph.symbols,
                ) {
                  Resolution::Ambiguous | Resolution::None => panic!(""),
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
