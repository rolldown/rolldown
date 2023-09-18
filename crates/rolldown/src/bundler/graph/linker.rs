use oxc::{semantic::ReferenceId, span::Atom};
use rolldown_common::{ModuleId, ResolvedExport, SymbolRef};
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

    Self::mark_whether_namespace_referenced(self.graph);

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

  fn mark_whether_namespace_referenced(graph: &mut Graph) {
    for id in &graph.sorted_modules {
      let importer = &graph.modules[*id];
      let importee_list = importer
        .import_records()
        .iter()
        .filter_map(|rec| {
          (rec.is_import_namespace && rec.resolved_module.is_valid()).then_some(rec.resolved_module)
        })
        .collect::<Vec<_>>();

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
              let resolved_ref = if info.is_imported_star {
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
            Module::External(_) => {
              // handle external module
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
