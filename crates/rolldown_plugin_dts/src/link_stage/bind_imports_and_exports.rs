use oxc_index::IndexVec;
// TODO: The current implementation for matching imports is enough so far but incomplete. It needs to be refactored
// if we want more enhancements related to exports.
use rolldown_common::{
  ModuleIdx, ResolvedExport, Specifier, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
};
use rolldown_error::{AmbiguousExternalNamespaceModule, BuildDiagnostic};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::{
  index_vec_ext::IndexVecExt,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};

use rustc_hash::FxHashMap;

use crate::types::DtsModule;

use super::{DtsLinkStage, dts_linking_meta_data::DtsLinkingMetadataVec};

#[derive(Clone, Debug)]
struct ImportTracker {
  pub importer: ModuleIdx,
  pub importee: ModuleIdx,
  pub imported: Specifier,
  pub imported_as: SymbolRef,
}

pub struct MatchingContext {
  tracker_stack: Vec<ImportTracker>,
}

impl MatchingContext {
  fn current_tracker(&self) -> &ImportTracker {
    self.tracker_stack.last().expect("tracker_stack is not empty")
  }
}

#[derive(Debug, Eq)]
pub struct MatchImportKindNormal {
  symbol: SymbolRef,
  reexports: Vec<SymbolRef>,
}

impl PartialEq for MatchImportKindNormal {
  fn eq(&self, other: &Self) -> bool {
    self.symbol == other.symbol
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatchImportKind {
  /// The import is either external or not defined.
  _Ignore,
  // "sourceIndex" and "ref" are in use
  Normal(MatchImportKindNormal),
  // "namespaceRef" and "alias" are in use
  // Namespace {
  //   namespace_ref: SymbolRef,
  // },
  // // Both "matchImportNormal" and "matchImportNamespace"
  // NormalAndNamespace {
  //   namespace_ref: SymbolRef,
  //   alias: Rstr,
  // },
  // The import could not be evaluated due to a cycle
  Cycle,
  // The import resolved to multiple symbols via "export * from"
  Ambiguous {
    symbol_ref: SymbolRef,
    potentially_ambiguous_symbol_refs: Vec<SymbolRef>,
  },
  NoMatch,
}

#[derive(Debug)]
pub enum ImportStatus {
  /// The imported file has no matching export
  NoMatch {
    // importee_id: NormalModuleId,
  },

  /// The imported file has a matching export
  Found {
    // owner: NormalModuleId,
    symbol: SymbolRef,
    potentially_ambiguous_export_star_refs: Vec<SymbolRef>,
  },
  // /// The imported file is external and has unknown exports
  // External(SymbolRef),
}

impl DtsLinkStage {
  pub(super) fn bind_imports_and_exports(&mut self) {
    // Initialize `resolved_exports` to prepare for matching imports with exports
    self.metas.par_iter_mut_enumerated().for_each(|(module_id, meta)| {
      let Some(module) = &self.modules[module_id] else {
        return;
      };
      let mut resolved_exports = module
        .named_exports
        .iter()
        .map(|(name, local)| {
          let resolved_export = ResolvedExport {
            symbol_ref: local.referenced,
            potentially_ambiguous_symbol_refs: None,
          };
          (name.clone(), resolved_export)
        })
        .collect::<FxHashMap<_, _>>();

      let mut module_stack = vec![];
      if module.has_star_exports {
        Self::add_exports_for_export_star(
          &self.modules,
          &mut resolved_exports,
          module_id,
          &mut module_stack,
        );
      }
      meta.resolved_exports = resolved_exports;
    });
    let mut normal_symbol_exports_chain_map = FxHashMap::default();
    let mut binding_ctx = BindImportsAndExportsContext {
      index_modules: &self.modules,
      metas: &mut self.metas,
      symbol_db: &mut self.symbols,
      errors: Vec::default(),
      warnings: Vec::default(),
      // external_import_binding_merger: FxHashMap::default(),
      normal_symbol_exports_chain_map: &mut normal_symbol_exports_chain_map,
    };
    self.modules.iter_enumerated().for_each(|(module_idx, _)| {
      binding_ctx.match_imports_with_exports(module_idx);
    });

    self.errors.extend(binding_ctx.errors);
    self.warnings.extend(binding_ctx.warnings);

    // for (module_idx, map) in &binding_ctx.external_import_binding_merger {
    //   for (key, symbol_set) in map {
    //     let name = if key.as_str() == "default" {
    //       let key = symbol_set
    //         .first()
    //         .map_or_else(|| key.clone(), |sym_ref| sym_ref.name(&self.symbols).into());
    //       Cow::Owned(key)
    //     } else if is_validate_identifier_name(key.as_str()) {
    //       Cow::Borrowed(key)
    //     } else {
    //       let legal_name = legitimize_identifier_name(key);
    //       Cow::Owned(legal_name.as_ref().into())
    //     };
    //     let target_symbol = self.symbols.create_facade_root_symbol_ref(*module_idx, &name);
    //     for symbol_ref in symbol_set {
    //       self.symbols.link(*symbol_ref, target_symbol);
    //     }
    //   }
    // }
    self.metas.par_iter_mut().for_each(|meta| {
      let mut sorted_and_non_ambiguous_resolved_exports = vec![];
      'next_export: for (exported_name, resolved_export) in &meta.resolved_exports {
        if let Some(potentially_ambiguous_symbol_refs) =
          &resolved_export.potentially_ambiguous_symbol_refs
        {
          let main_ref = self.symbols.canonical_ref_for(resolved_export.symbol_ref);

          for ambiguous_ref in potentially_ambiguous_symbol_refs {
            let ambiguous_ref = self.symbols.canonical_ref_for(*ambiguous_ref);
            if main_ref != ambiguous_ref {
              continue 'next_export;
            }
          }
        };
        sorted_and_non_ambiguous_resolved_exports.push(exported_name.clone());
      }
      sorted_and_non_ambiguous_resolved_exports.sort_unstable();
      meta.sorted_and_non_ambiguous_resolved_exports = sorted_and_non_ambiguous_resolved_exports;
    });
    self.resolve_member_expr_refs();
  }

  fn add_exports_for_export_star(
    modules: &IndexVec<ModuleIdx, Option<DtsModule>>,
    resolve_exports: &mut FxHashMap<Rstr, ResolvedExport>,
    module_id: ModuleIdx,
    module_stack: &mut Vec<ModuleIdx>,
  ) {
    if module_stack.contains(&module_id) {
      return;
    }

    module_stack.push(module_id);

    let Some(module) = &modules[module_id] else {
      return;
    };

    for dep_id in module.star_export_module_ids() {
      let Some(dep_module) = &modules[dep_id] else {
        continue;
      };

      for (exported_name, named_export) in &dep_module.named_exports {
        // ES6 export star statements ignore exports named "default"
        if exported_name.as_str() == "default" {
          continue;
        }
        // This export star is shadowed if any file in the stack has a matching real named export
        if module_stack
          .iter()
          .filter_map(|id| modules[*id].as_ref())
          .any(|module| module.named_exports.contains_key(exported_name))
        {
          continue;
        }

        // We have filled `resolve_exports` with `named_exports`. If the export is already exists, it means that the importer
        // has a named export with the same name. So the export from dep module is shadowed.
        if let Some(resolved_export) = resolve_exports.get_mut(exported_name) {
          if named_export.referenced != resolved_export.symbol_ref {
            resolved_export
              .potentially_ambiguous_symbol_refs
              .get_or_insert(Vec::default())
              .push(named_export.referenced);
          }
        } else {
          let resolved_export = ResolvedExport {
            symbol_ref: named_export.referenced,
            potentially_ambiguous_symbol_refs: None,
          };
          resolve_exports.insert(exported_name.clone(), resolved_export);
        }
      }

      Self::add_exports_for_export_star(modules, resolve_exports, dep_id, module_stack);
    }

    module_stack.pop();
  }

  /// Try to find the final pointed `SymbolRef` of the member expression.
  /// ```js
  /// // index.js
  /// import * as foo_ns from './foo';
  /// foo_ns.bar_ns.c;
  /// // foo.js
  /// export * as bar_ns from './bar';
  /// // bar.js
  /// export const c = 1;
  /// ```
  /// The final pointed `SymbolRef` of `foo_ns.bar_ns.c` is the `c` in `bar.js`.
  fn resolve_member_expr_refs(&mut self) {
    let warnings = append_only_vec::AppendOnlyVec::new();
    let resolved_meta_data = self
      .modules
      .par_iter()
      .map(|module| match module {
        Some(module) => {
          let mut resolved_map = FxHashMap::default();
          module.stmt_infos.iter().for_each(|stmt_info| {
            stmt_info.referenced_symbols.iter().for_each(|symbol_ref| {
              if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = symbol_ref {
                // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
                let mut canonical_ref = self.symbols.canonical_ref_for(member_expr_ref.object_ref);
                let Some(mut canonical_ref_owner) = self.modules[canonical_ref.owner].as_ref()
                else {
                  return;
                };
                let mut is_namespace_ref =
                  canonical_ref_owner.namespace_object_ref == canonical_ref;
                let mut ns_symbol_list = vec![];
                let mut cursor = 0;
                while cursor < member_expr_ref.props.len() && is_namespace_ref {
                  let name = &member_expr_ref.props[cursor];
                  let meta = &self.metas[canonical_ref_owner.module_index];
                  let export_symbol = &meta.resolved_exports[&name.to_rstr()];
                  if !meta.sorted_and_non_ambiguous_resolved_exports.contains(&name.to_rstr()) {
                    resolved_map.insert(
                      member_expr_ref.span,
                      (None, member_expr_ref.props[cursor..].to_vec()),
                    );
                    return;
                  };

                  ns_symbol_list.push((canonical_ref, name.to_rstr()));
                  canonical_ref = self.symbols.canonical_ref_for(export_symbol.symbol_ref);
                  canonical_ref_owner = self.modules[canonical_ref.owner].as_ref().unwrap();
                  cursor += 1;
                  is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
                }
                if cursor > 0 {
                  resolved_map.insert(
                    member_expr_ref.span,
                    (Some(canonical_ref), member_expr_ref.props[cursor..].to_vec()),
                  );
                }
              }
            });
          });

          resolved_map
        }
        _ => FxHashMap::default(),
      })
      .collect::<Vec<_>>();

    debug_assert_eq!(self.metas.len(), resolved_meta_data.len());
    self.warnings.extend(warnings);
    self.metas.iter_mut_enumerated().zip(resolved_meta_data).for_each(
      |((_idx, meta), resolved_map)| {
        meta.resolved_member_expr_refs = resolved_map;
      },
    );
  }
}

struct BindImportsAndExportsContext<'a> {
  pub index_modules: &'a IndexVec<ModuleIdx, Option<DtsModule>>,
  pub metas: &'a mut DtsLinkingMetadataVec,
  pub symbol_db: &'a mut SymbolRefDb,
  pub errors: Vec<BuildDiagnostic>,
  pub warnings: Vec<BuildDiagnostic>,
  // pub external_import_binding_merger:
  //   FxHashMap<ModuleIdx, FxHashMap<CompactStr, IndexSet<SymbolRef>>>,
  pub normal_symbol_exports_chain_map: &'a mut FxHashMap<SymbolRef, Vec<SymbolRef>>,
}

impl BindImportsAndExportsContext<'_> {
  #[allow(clippy::too_many_lines)]
  fn match_imports_with_exports(&mut self, module_id: ModuleIdx) {
    let Some(module) = &self.index_modules[module_id] else {
      return;
    };
    for (imported_as_ref, named_import) in &module.named_imports {
      let match_import_span = tracing::trace_span!(
        "MATCH_IMPORT",
        module_id = module.stable_id.to_string(),
        imported_specifier = named_import.imported.to_string()
      );
      let _enter = match_import_span.enter();

      let resolved_module = module.import_record_to_module_idx[named_import.record_id];
      // let is_external = matches!(self.index_modules[rec.resolved_module], Module::External(_));

      // if is_esm
      //   && is_external
      //   && self.metas[module_id]
      //     .resolved_exports
      //     .iter()
      //     .all(|(_, resolved_export)| resolved_export.symbol_ref != *imported_as_ref)
      // {
      //   if let Specifier::Literal(ref name) = named_import.imported {
      //     self
      //       .external_import_binding_merger
      //       .entry(rec.resolved_module)
      //       .or_default()
      //       .entry(name.inner().clone())
      //       .or_default()
      //       .insert(*imported_as_ref);
      //   }
      // }
      let ret = self.match_import_with_export(
        self.index_modules,
        &mut MatchingContext { tracker_stack: Vec::default() },
        ImportTracker {
          importer: module_id,
          importee: resolved_module,
          imported: named_import.imported.clone(),
          imported_as: *imported_as_ref,
        },
      );
      tracing::trace!("Got match result {:?}", ret);
      match ret {
        MatchImportKind::_Ignore | MatchImportKind::Cycle => {}
        MatchImportKind::Ambiguous { symbol_ref, potentially_ambiguous_symbol_refs } => {
          let importee =
            self.index_modules[resolved_module].as_ref().unwrap().stable_id.to_string();

          let mut exporter = Vec::with_capacity(potentially_ambiguous_symbol_refs.len() + 1);
          if let Some(owner) = self.index_modules[symbol_ref.owner].as_ref() {
            if let Specifier::Literal(name) = &named_import.imported {
              let named_export = &owner.named_exports[name];
              exporter.push(AmbiguousExternalNamespaceModule {
                source: owner.dts_ast.source().clone(),
                filename: owner.stable_id.to_string(),
                span_of_identifier: named_export.span,
              });
            }
          }

          exporter.extend(potentially_ambiguous_symbol_refs.iter().filter_map(|&symbol_ref| {
            let module = self.index_modules[symbol_ref.owner].as_ref()?;
            if let Specifier::Literal(name) = &named_import.imported {
              let named_export = &module.named_exports[name];
              return Some(AmbiguousExternalNamespaceModule {
                source: module.dts_ast.source().clone(),
                filename: module.stable_id.to_string(),
                span_of_identifier: named_export.span,
              });
            }

            None
          }));

          self.errors.push(BuildDiagnostic::ambiguous_external_namespace(
            named_import.imported.to_string(),
            importee,
            AmbiguousExternalNamespaceModule {
              source: module.dts_ast.source().clone(),
              filename: module.stable_id.to_string(),
              span_of_identifier: named_import.span_imported,
            },
            exporter,
          ));
        }
        MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports }) => {
          self.normal_symbol_exports_chain_map.insert(*imported_as_ref, reexports);

          self.symbol_db.link(*imported_as_ref, symbol);
        }
        // MatchImportKind::Namespace { namespace_ref } => {
        //   self.symbol_db.link(*imported_as_ref, namespace_ref);
        // }
        // MatchImportKind::NormalAndNamespace { namespace_ref, alias } => {
        //   self.symbol_db.get_mut(*imported_as_ref).namespace_alias =
        //     Some(NamespaceAlias { property_name: alias, namespace_ref });
        // }
        MatchImportKind::NoMatch => {
          let importee = self.index_modules[resolved_module].as_ref().unwrap();
          self.errors.push(BuildDiagnostic::missing_export(
            module.stable_id.to_string(),
            importee.stable_id.to_string(),
            module.dts_ast.source().clone(),
            named_import.imported.to_string(),
            named_import.span_imported,
          ));
        }
      }
    }
  }

  fn advance_import_tracker(&self, ctx: &MatchingContext) -> ImportStatus {
    let tracker = ctx.current_tracker();
    let importer =
      self.index_modules[tracker.importer].as_ref().expect("only dts module can be importer");
    let named_import = &importer.named_imports[&tracker.imported_as];

    // Is this an external file?
    let importee_id = importer.import_record_to_module_idx[named_import.record_id];
    let Some(importee) = &self.index_modules[importee_id] else {
      // TODO external
      return ImportStatus::NoMatch {};
    };

    match &named_import.imported {
      Specifier::Star => ImportStatus::Found {
        symbol: importee.namespace_object_ref,
        // owner: importee_id,
        potentially_ambiguous_export_star_refs: vec![],
      },
      Specifier::Literal(literal_imported) => {
        match self.metas[importee_id].resolved_exports.get(literal_imported) {
          Some(export) => {
            ImportStatus::Found {
              // owner: importee_id,
              symbol: export.symbol_ref,
              potentially_ambiguous_export_star_refs: export
                .potentially_ambiguous_symbol_refs
                .clone()
                .unwrap_or_default(),
            }
          }
          _ => ImportStatus::NoMatch {},
        }
      }
    }
  }

  #[allow(clippy::too_many_lines)]
  fn match_import_with_export(
    &mut self,
    index_modules: &IndexVec<ModuleIdx, Option<DtsModule>>,
    ctx: &mut MatchingContext,
    mut tracker: ImportTracker,
  ) -> MatchImportKind {
    let tracking_span = tracing::trace_span!(
      "TRACKING_MATCH_IMPORT",
      importer = index_modules[tracker.importer].as_ref().unwrap().module_id.to_string(),
      importee = index_modules[tracker.importee].as_ref().unwrap().stable_id.to_string(),
      imported_specifier = tracker.imported.to_string()
    );
    let _enter = tracking_span.enter();

    let mut ambiguous_results = vec![];
    let mut reexports = vec![];
    let ret = loop {
      for prev_tracker in ctx.tracker_stack.iter().rev() {
        if prev_tracker.importer == tracker.importer
          && prev_tracker.imported_as == tracker.imported_as
        {
          // Cycle import. No need to continue, just return
          return MatchImportKind::Cycle;
        }
      }
      ctx.tracker_stack.push(tracker.clone());
      let import_status = self.advance_import_tracker(ctx);
      tracing::trace!("Got import_status {:?}", import_status);

      match import_status {
        ImportStatus::NoMatch { .. } => {
          break MatchImportKind::NoMatch;
        }
        ImportStatus::Found { symbol, potentially_ambiguous_export_star_refs, .. } => {
          for ambiguous_ref in &potentially_ambiguous_export_star_refs {
            let ambiguous_ref_owner = index_modules[ambiguous_ref.owner].as_ref().unwrap();
            match ambiguous_ref_owner.named_imports.get(ambiguous_ref) {
              Some(another_named_import) => {
                let ambiguous_result = self.match_import_with_export(
                  index_modules,
                  &mut MatchingContext { tracker_stack: ctx.tracker_stack.clone() },
                  ImportTracker {
                    importer: ambiguous_ref_owner.module_index,
                    importee: ambiguous_ref_owner.import_record_to_module_idx
                      [another_named_import.record_id],
                    imported: another_named_import.imported.clone(),
                    imported_as: another_named_import.imported_as,
                  },
                );
                ambiguous_results.push(ambiguous_result);
              }
              _ => {
                ambiguous_results.push(MatchImportKind::Normal(MatchImportKindNormal {
                  symbol: *ambiguous_ref,
                  reexports: vec![],
                }));
              }
            }
          }

          // If this is a re-export of another import, continue for another
          // iteration of the loop to resolve that import as well
          let owner = index_modules[symbol.owner].as_ref().unwrap();
          if let Some(another_named_import) = owner.named_imports.get(&symbol) {
            match self.index_modules
              [owner.import_record_to_module_idx[another_named_import.record_id]]
              .as_ref()
            {
              Some(importee) => {
                tracker.importee = importee.module_index;
                tracker.importer = owner.module_index;
                tracker.imported = another_named_import.imported.clone();
                tracker.imported_as = another_named_import.imported_as;
                reexports.push(another_named_import.imported_as);
                continue;
              }
              None => {
                break MatchImportKind::NoMatch;
                // TODO external
                // break MatchImportKind::Normal(MatchImportKindNormal {
                //   symbol: another_named_import.imported_as,
                //   reexports: vec![],
                // });
              }
            }
          }

          break MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports });
        } // ImportStatus::External(_) => {
          //   break MatchImportKind::Normal(MatchImportKindNormal {
          //     symbol: tracker.imported_as,
          //     reexports: vec![],
          //   });
          // }
      };
    };

    tracing::trace!("ambiguous_results {:#?}", ambiguous_results);
    tracing::trace!("ret {:#?}", ret);

    for ambiguous_result in &ambiguous_results {
      if *ambiguous_result != ret {
        if let MatchImportKind::Normal(MatchImportKindNormal { symbol, .. }) = ret {
          return MatchImportKind::Ambiguous {
            symbol_ref: symbol,
            potentially_ambiguous_symbol_refs: ambiguous_results
              .iter()
              .filter_map(|kind| match *kind {
                MatchImportKind::Normal(MatchImportKindNormal { symbol, .. }) => Some(symbol),
                // MatchImportKind::Namespace { namespace_ref }
                // | MatchImportKind::NormalAndNamespace { namespace_ref, .. } => Some(namespace_ref),
                _ => None,
              })
              .collect(),
          };
        }

        unreachable!("symbol should always exist");
      }
    }

    ret
  }
}
