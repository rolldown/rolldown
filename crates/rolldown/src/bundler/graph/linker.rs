use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ExportsKind, ImportKind, LocalOrReExport, ModuleId, StmtInfo, SymbolRef};
use rustc_hash::FxHashMap;

use super::{graph::Graph, symbols::NamespaceAlias};
use crate::bundler::{
  graph::symbols::Symbols,
  module::{normal_module::Resolution, Module, NormalModule},
};

/// Store the linking info for module
#[derive(Debug, Default)]
pub struct LinkingInfo {
  // The symbol for wrapped module
  pub wrap_symbol: Option<SymbolRef>,
  pub facade_stmt_infos: Vec<StmtInfo>,
  pub resolved_exports: FxHashMap<Atom, SymbolRef>,
  pub resolved_star_exports: Vec<ModuleId>,
  pub is_symbol_for_namespace_referenced: bool,
}

pub type LinkingInfoVec = IndexVec<ModuleId, LinkingInfo>;

pub struct Linker<'graph> {
  graph: &'graph mut Graph,
}

impl<'graph> Linker<'graph> {
  pub fn new(graph: &'graph mut Graph) -> Self {
    Self { graph }
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
          let resolved = importer.resolve_star_exports(&self.graph.modules);
          linking_infos[*id].resolved_star_exports = resolved;
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
      let linking_info = &mut linking_infos[id];
      self.resolve_exports(id, &mut symbols, linking_info);
      self.resolve_imports(id, &mut symbols, linking_info);
    });

    // Set the symbols back and add linker modules to graph
    self.graph.symbols = symbols;
    self.graph.linking_infos = linking_infos;
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
              importer.generate_symbol_import_and_use(
                importee_warp_symbol,
                importer_linking_info,
                symbols,
              );
              importer.generate_symbol_import_and_use(
                importee.namespace_symbol,
                importer_linking_info,
                symbols,
              );
              match (importer.exports_kind, importee.exports_kind) {
                (ExportsKind::Esm, ExportsKind::CommonJs) => {
                  importer.generate_symbol_import_and_use(
                    self.graph.runtime.resolve_symbol(&"__toESM".into()),
                    importer_linking_info,
                    symbols,
                  );
                }
                (_, ExportsKind::Esm) => {
                  importer.generate_symbol_import_and_use(
                    self.graph.runtime.resolve_symbol(&"__toCommonJS".into()),
                    importer_linking_info,
                    symbols,
                  );
                }
                _ => {}
              }
            }
          });
          importer.get_star_exports_modules().for_each(|id| match &self.graph.modules[id] {
            Module::Normal(importee) => {
              if importee.exports_kind == ExportsKind::CommonJs {
                importer.generate_symbol_import_and_use(
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
    if let Module::Normal(module) = &self.graph.modules[target] {
      module.initialize_namespace(linking_info);
    }
    if linking_info.wrap_symbol.is_some() {
      return;
    }

    // Generate symbol for wrap module declaration
    // Case commonjs, eg var require_a = __commonJS()
    // Case esm, eg var init_a = __esm()
    match &self.graph.modules[target] {
      Module::Normal(module) => {
        module.create_wrap_symbol(linking_info, symbols);

        let name = if module.exports_kind == ExportsKind::CommonJs {
          "__commonJS".into()
        } else {
          "__esm".into()
        };
        let runtime_symbol = self.graph.runtime.resolve_symbol(&name);
        module.generate_symbol_import_and_use(runtime_symbol, linking_info, symbols);
        module.import_records.iter().for_each(|record| {
          self.wrap_module(record.resolved_module, symbols, linking_infos);
        });
      }
      Module::External(_) => {}
    }
  }

  #[allow(clippy::needless_collect)]
  fn mark_extra_symbols(&mut self, symbols: &mut Symbols, linking_infos: &mut LinkingInfoVec) {
    // Determine if the namespace symbol need to be generated
    for importer_id in &self.graph.sorted_modules {
      let importer = &self.graph.modules[*importer_id];

      if let Module::Normal(importer) = importer {
        let has_reexport_all_from_cjs = importer.get_star_exports_modules().any(|importee| matches!(&self.graph.modules[importee], Module::Normal(m) if m.exports_kind == ExportsKind::CommonJs));
        if has_reexport_all_from_cjs {
          self.graph.modules[*importer_id]
            .mark_symbol_for_namespace_referenced(&mut linking_infos[*importer_id]);
        }
      }

      importer
        .import_records()
        .iter()
        .filter_map(|rec| {
          ((rec.is_import_namespace || matches!(rec.kind, ImportKind::Require))
            && rec.resolved_module.is_valid())
          .then_some(rec.resolved_module)
        })
        .for_each(|importee| {
          self.graph.modules[importee]
            .mark_symbol_for_namespace_referenced(&mut linking_infos[importee]);
        });

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

  fn resolve_exports(&self, id: ModuleId, symbols: &mut Symbols, linking_info: &mut LinkingInfo) {
    let importer = &self.graph.modules[id];
    match importer {
      Module::Normal(importer) => {
        let exported_names = importer.get_exported_names(&mut Vec::default(), &self.graph.modules);

        let mut resolutions = exported_names
          .iter()
          .map(|exported| {
            (
              *exported,
              importer.resolve_export_for_esm_and_cjs(
                exported,
                &mut Vec::default(),
                &self.graph.modules,
                symbols,
              ),
            )
          })
          .collect::<FxHashMap<_, _>>();

        #[allow(clippy::items_after_statements)]
        fn create_local_symbol_for_found_resolution(
          symbol_ref: SymbolRef,
          importer: &NormalModule,
          importer_linking_info: &mut LinkingInfo,
          symbols: &mut Symbols,
        ) -> SymbolRef {
          if symbol_ref.owner == importer.id {
            symbol_ref
          } else {
            let local_symbol_ref = importer.generate_local_symbol(
              Atom::from("#FACADE#"),
              importer_linking_info,
              symbols,
            );
            symbols.union(local_symbol_ref, symbol_ref);
            local_symbol_ref
          }
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
              let local_symbol_ref =
                create_local_symbol_for_found_resolution(ext, importer, linking_info, symbols);
              linking_info.resolved_exports.insert(exported.clone(), local_symbol_ref);
            }
            Resolution::Runtime(ns_ref) => {
              // export { a } from './foo.cjs' => `var a = foo.a; export { a }`
              let local_binding_ref = symbols.create_symbol(importer.id, exported.clone());
              let local_binding = symbols.get_mut(local_binding_ref);
              local_binding.namespace_alias =
                Some(NamespaceAlias { property_name: exported.clone(), namespace_ref: ns_ref });
              linking_info.facade_stmt_infos.push(StmtInfo {
                stmt_idx: None,
                declared_symbols: vec![local_binding_ref],
                referenced_symbols: vec![ns_ref],
              });
              linking_info.resolved_exports.insert(exported.clone(), local_binding_ref);
            }
          }
        });

        resolutions.into_iter().for_each(|(exported, left)| match left {
          Resolution::None => panic!("shouldn't has left which is None"),
          Resolution::Found(ext) => {
            let local_symbol_ref =
              create_local_symbol_for_found_resolution(ext, importer, linking_info, symbols);
            linking_info.resolved_exports.insert(exported.clone(), local_symbol_ref);
          }
          Resolution::Ambiguous => {}
          Resolution::Runtime(ns_ref) => {
            let local_binding_ref = symbols.create_symbol(importer.id, exported.clone());
            let local_binding = symbols.get_mut(local_binding_ref);
            local_binding.namespace_alias =
              Some(NamespaceAlias { property_name: exported.clone(), namespace_ref: ns_ref });
            linking_info.facade_stmt_infos.push(StmtInfo {
              stmt_idx: None,
              declared_symbols: vec![local_binding_ref],
              referenced_symbols: vec![ns_ref],
            });
          }
        });
      }
      Module::External(_) => {
        // TODO: handle external module
      }
    }
  }

  fn resolve_imports(&self, id: ModuleId, symbols: &mut Symbols, linking_info: &mut LinkingInfo) {
    let importer = &self.graph.modules[id];
    match importer {
      Module::Normal(importer) => {
        importer.named_imports.iter().for_each(|(_id, info)| {
          let import_record = &importer.import_records[info.record_id];
          let importee = &self.graph.modules[import_record.resolved_module];
          match importee {
            Module::Normal(importee) => {
              let resolved_ref = if info.is_imported_star {
                importee.namespace_symbol
              } else {
                match importee.resolve_export_for_esm_and_cjs(
                  &info.imported,
                  &mut Vec::default(),
                  &self.graph.modules,
                  symbols,
                ) {
                  Resolution::Ambiguous | Resolution::None => panic!(""),
                  Resolution::Found(founded) => founded,
                  Resolution::Runtime(_) => {
                    symbols.get_mut(info.imported_as).namespace_alias = Some(NamespaceAlias {
                      property_name: info.imported.clone(),
                      namespace_ref: importee.namespace_symbol,
                    });

                    importer.generate_symbol_import_and_use(
                      importee.namespace_symbol,
                      linking_info,
                      symbols,
                    );

                    return;
                  }
                }
              };
              symbols.union(info.imported_as, resolved_ref);
            }
            Module::External(importee) => {
              let resolved_ref = importee.resolve_export(&info.imported, info.is_imported_star);
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
}
