use arcstr::ArcStr;
// TODO: The current implementation for matching imports is enough so far but incomplete. It needs to be refactored
// if we want more enhancements related to exports.
use rolldown_common::{
  ExportsKind, IndexModules, Module, ModuleIdx, ModuleType, ResolvedExport, Specifier,
  SymbolOrMemberExprRef, SymbolRef,
};
use rolldown_error::{AmbiguousExternalNamespaceModule, BuildDiagnostic};
use rolldown_rstr::{Rstr, ToRstr};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{
  IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelBridge, ParallelIterator,
};

use rustc_hash::FxHashMap;

use crate::{
  types::{
    linking_metadata::LinkingMetadataVec, namespace_alias::NamespaceAlias,
    symbol_ref_db::SymbolRefDb,
  },
  SharedOptions,
};

use super::LinkStage;

#[derive(Clone)]
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

#[derive(Debug, PartialEq, Eq)]
pub enum MatchImportKind {
  /// The import is either external or not defined.
  _Ignore,
  // "sourceIndex" and "ref" are in use
  Normal {
    symbol: SymbolRef,
  },
  // "namespaceRef" and "alias" are in use
  Namespace {
    namespace_ref: SymbolRef,
  },
  // Both "matchImportNormal" and "matchImportNamespace"
  NormalAndNamespace {
    namespace_ref: SymbolRef,
    alias: Rstr,
  },
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

  /// The imported file is CommonJS and has unknown exports
  CommonJS,

  /// The import is missing but there is a dynamic fallback object
  DynamicFallback { namespace_ref: SymbolRef },

  /// The import was treated as a CommonJS import but the file is known to have no exports
  _CommonJSWithoutExports,

  /// The imported file was disabled by mapping it to false in the "browser" field of package.json
  _Disabled,

  /// The imported file is external and has unknown exports
  External,
}

impl<'link> LinkStage<'link> {
  /// Notices:
  /// - For external import like
  /// ```js
  /// // main.js
  /// import { a } from 'external';
  ///
  /// // foo.js
  /// import { a } from 'external';
  /// export { a }
  /// ```
  ///
  /// Unlike import from normal modules, the imported variable deosn't have a place that declared the variable. So we consider `import { a } from 'external'` in `foo.js` as the declaration statement of `a`.
  pub fn bind_imports_and_exports(&mut self) {
    // Initialize `resolved_exports` to prepare for matching imports with exports
    self.metas.iter_mut_enumerated().par_bridge().for_each(|(module_id, meta)| {
      let Module::Normal(module) = &self.module_table.modules[module_id] else {
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
      if !module.star_exports.is_empty() {
        Self::add_exports_for_export_star(
          &self.module_table.modules,
          &mut resolved_exports,
          module_id,
          &mut module_stack,
        );
      }
      meta.resolved_exports = resolved_exports;
    });

    let mut binding_ctx = BindImportsAndExportsContext {
      normal_modules: &self.module_table.modules,
      metas: &mut self.metas,
      symbols: &mut self.symbols,
      options: self.options,
      errors: Vec::default(),
      warnings: Vec::default(),
    };

    self.module_table.modules.iter().for_each(|module| {
      binding_ctx.match_imports_with_exports(module.idx());
    });

    self.errors.extend(binding_ctx.errors);
    self.warnings.extend(binding_ctx.warnings);

    self.metas.par_iter_mut().for_each(|meta| {
      let mut sorted_and_non_ambiguous_resolved_exports = vec![];
      'next_export: for (exported_name, resolved_export) in &meta.resolved_exports {
        if let Some(potentially_ambiguous_symbol_refs) =
          &resolved_export.potentially_ambiguous_symbol_refs
        {
          let main_ref = self.symbols.par_canonical_ref_for(resolved_export.symbol_ref);

          for ambiguous_ref in potentially_ambiguous_symbol_refs {
            let ambiguous_ref = self.symbols.par_canonical_ref_for(*ambiguous_ref);
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
    normal_modules: &IndexModules,
    resolve_exports: &mut FxHashMap<Rstr, ResolvedExport>,
    module_id: ModuleIdx,
    module_stack: &mut Vec<ModuleIdx>,
  ) {
    if module_stack.contains(&module_id) {
      return;
    }

    module_stack.push(module_id);

    let Module::Normal(module) = &normal_modules[module_id] else {
      return;
    };

    for dep_id in module.star_export_module_ids() {
      let Module::Normal(dep_module) = &normal_modules[dep_id] else {
        continue;
      };
      if matches!(dep_module.exports_kind, ExportsKind::CommonJs) {
        continue;
      }

      for (exported_name, named_export) in &dep_module.named_exports {
        // ES6 export star statements ignore exports named "default"
        if exported_name.as_str() == "default" {
          continue;
        }
        // This export star is shadowed if any file in the stack has a matching real named export
        if module_stack
          .iter()
          .filter_map(|id| normal_modules[*id].as_normal())
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

      Self::add_exports_for_export_star(normal_modules, resolve_exports, dep_id, module_stack);
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
    let resolved_maps = self
      .module_table
      .modules
      .as_vec()
      .par_iter()
      .map(|module| match module {
        Module::Normal(module) => {
          let mut resolved = FxHashMap::default();
          module.stmt_infos.iter().for_each(|stmt_info| {
            stmt_info.referenced_symbols.iter().for_each(|symbol_ref| {
              if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = symbol_ref {
                // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
                let mut canonical_ref =
                  self.symbols.par_canonical_ref_for(member_expr_ref.object_ref);
                let mut canonical_ref_owner = self.module_table.modules[canonical_ref.owner]
                  .as_normal()
                  .expect("only normal module");
                let mut is_namespace_ref =
                  canonical_ref_owner.namespace_object_ref == canonical_ref;
                let mut ns_symbol_list = vec![];
                let mut cursor = 0;
                while cursor < member_expr_ref.props.len() && is_namespace_ref {
                  let name = &member_expr_ref.props[cursor];
                  let meta = &self.metas[canonical_ref_owner.idx];
                  let export_symbol = meta.resolved_exports.get(&name.to_rstr());
                  let Some(export_symbol) = export_symbol else {
                    // when we try to resolve `a.b.c`, and found that `b` is not exported by module
                    // that `a` pointed to, convert the `a.b.c` into `void 0` if module `a` do not
                    // have any dynamic exports.
                    if !self.metas[canonical_ref_owner.idx].has_dynamic_exports {
                      resolved.insert(member_expr_ref.span, None);
                      warnings.push(
                        BuildDiagnostic::import_is_undefined(
                          ArcStr::from(module.id.as_str()),
                          module.source.clone(),
                          member_expr_ref.span,
                          ArcStr::from(name.as_str()),
                          canonical_ref_owner.stable_id.to_string(),
                        )
                        .with_severity_warning(),
                      );
                    }
                    break;
                  };
                  if !meta.sorted_and_non_ambiguous_resolved_exports.contains(&name.to_rstr()) {
                    resolved.insert(member_expr_ref.span, None);
                    return;
                  };

                  // TODO(hyf0): suspicious cjs might just fallback to dynamic lookup?
                  if !self.module_table.modules[export_symbol.symbol_ref.owner]
                    .as_normal()
                    .unwrap()
                    .exports_kind
                    .is_esm()
                  {
                    break;
                  }
                  ns_symbol_list.push((canonical_ref, name.to_rstr()));
                  canonical_ref = self.symbols.par_canonical_ref_for(export_symbol.symbol_ref);
                  canonical_ref_owner =
                    self.module_table.modules[canonical_ref.owner].as_normal().unwrap();
                  cursor += 1;
                  is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
                }
                if cursor > 0 {
                  resolved.insert(
                    member_expr_ref.span,
                    Some((canonical_ref, member_expr_ref.props[cursor..].to_vec())),
                  );
                }
              }
            });
          });

          resolved
        }
        Module::External(_) => FxHashMap::default(),
      })
      .collect::<Vec<_>>();

    debug_assert_eq!(self.metas.len(), resolved_maps.len());
    self.warnings.extend(warnings);
    self.metas.par_iter_mut().zip(resolved_maps).for_each(|(meta, resolved_map)| {
      meta.resolved_member_expr_refs = resolved_map;
    });
  }
}

struct BindImportsAndExportsContext<'a> {
  pub normal_modules: &'a IndexModules,
  pub metas: &'a mut LinkingMetadataVec,
  pub symbols: &'a mut SymbolRefDb,
  pub options: &'a SharedOptions,
  pub errors: Vec<BuildDiagnostic>,
  pub warnings: Vec<BuildDiagnostic>,
}

impl<'a> BindImportsAndExportsContext<'a> {
  fn match_imports_with_exports(&mut self, module_id: ModuleIdx) {
    let Module::Normal(module) = &self.normal_modules[module_id] else {
      return;
    };
    for (imported_as_ref, named_import) in &module.named_imports {
      let match_import_span = tracing::trace_span!(
        "MATCH_IMPORT",
        module_id = module.stable_id,
        imported_specifier = format!("{}", named_import.imported)
      );
      let _enter = match_import_span.enter();

      let rec = &module.import_records[named_import.record_id];

      let ret = match &self.normal_modules[rec.resolved_module] {
        Module::External(_) => MatchImportKind::Normal { symbol: *imported_as_ref },
        Module::Normal(importee) => self.match_import_with_export(
          self.normal_modules,
          &mut MatchingContext { tracker_stack: Vec::default() },
          ImportTracker {
            importer: module_id,
            importee: importee.idx,
            imported: named_import.imported.clone(),
            imported_as: *imported_as_ref,
          },
        ),
      };
      tracing::trace!("Got match result {:?}", ret);
      match ret {
        MatchImportKind::_Ignore | MatchImportKind::Cycle => {}
        MatchImportKind::Ambiguous { symbol_ref, potentially_ambiguous_symbol_refs } => {
          let importee = self.normal_modules[rec.resolved_module].stable_id().to_string();

          let mut exporter = Vec::with_capacity(potentially_ambiguous_symbol_refs.len() + 1);
          if let Some(owner) = self.normal_modules[symbol_ref.owner].as_normal() {
            if let Specifier::Literal(name) = &named_import.imported {
              let named_export = &owner.named_exports[name];
              exporter.push(AmbiguousExternalNamespaceModule {
                source: owner.source.clone(),
                filename: owner.stable_id.to_string(),
                span_of_identifier: named_export.span,
              });
            }
          }

          exporter.extend(
            potentially_ambiguous_symbol_refs
              .iter()
              .filter_map(|&symbol_ref| {
                if let Some(owner) = self.normal_modules[symbol_ref.owner].as_normal() {
                  if let Specifier::Literal(name) = &named_import.imported {
                    let named_export = &owner.named_exports[name];
                    return Some(AmbiguousExternalNamespaceModule {
                      source: owner.source.clone(),
                      filename: owner.stable_id.to_string(),
                      span_of_identifier: named_export.span,
                    });
                  }
                }

                None
              })
              .collect::<Vec<_>>(),
          );

          self.errors.push(BuildDiagnostic::ambiguous_external_namespace(
            named_import.imported.to_string(),
            importee,
            AmbiguousExternalNamespaceModule {
              source: module.source.clone(),
              filename: module.stable_id.to_string(),
              span_of_identifier: named_import.span_imported,
            },
            exporter,
          ));
        }
        MatchImportKind::Normal { symbol } => {
          self.symbols.link(*imported_as_ref, symbol);
        }
        MatchImportKind::Namespace { namespace_ref } => {
          self.symbols.link(*imported_as_ref, namespace_ref);
        }
        MatchImportKind::NormalAndNamespace { namespace_ref, alias } => {
          self.symbols.get_mut(*imported_as_ref).namespace_alias =
            Some(NamespaceAlias { property_name: alias, namespace_ref });
        }
        MatchImportKind::NoMatch => {
          let importee = &self.normal_modules[rec.resolved_module];
          self.errors.push(BuildDiagnostic::missing_export(
            module.stable_id.to_string(),
            importee.stable_id().to_string(),
            module.source.clone(),
            named_import.imported.to_string(),
            named_import.span_imported,
          ));
        }
      }
    }
  }

  fn advance_import_tracker(&self, ctx: &mut MatchingContext) -> ImportStatus {
    let tracker = ctx.current_tracker();
    let importer = &self.normal_modules[tracker.importer]
      .as_normal()
      .expect("only normal module can be importer");
    let named_import = &importer.named_imports[&tracker.imported_as];

    // Is this an external file?
    let importee_id = importer.import_records[named_import.record_id].resolved_module;
    let importee_id = match &self.normal_modules[importee_id] {
      Module::Normal(importee) => importee.idx,
      Module::External(_) => return ImportStatus::External,
    };

    // Is this a named import of a file without any exports?
    let importee =
      &self.normal_modules[importee_id].as_normal().expect("external module is bailout above");
    debug_assert!(matches!(importee.exports_kind, ExportsKind::Esm | ExportsKind::CommonJs));
    // TODO: Deal with https://github.com/evanw/esbuild/blob/109449e5b80886f7bc7fc7e0cee745a0221eef8d/internal/linker/linker.go#L3062-L3072

    if matches!(importee.exports_kind, ExportsKind::CommonJs) {
      return ImportStatus::CommonJS;
    }

    match &named_import.imported {
      Specifier::Star => ImportStatus::Found {
        symbol: importee.namespace_object_ref,
        // owner: importee_id,
        potentially_ambiguous_export_star_refs: vec![],
      },
      Specifier::Literal(literal_imported) => {
        if let Some(export) = self.metas[importee_id].resolved_exports.get(literal_imported) {
          ImportStatus::Found {
            // owner: importee_id,
            symbol: export.symbol_ref,
            potentially_ambiguous_export_star_refs: export
              .potentially_ambiguous_symbol_refs
              .clone()
              .unwrap_or_default(),
          }
        } else if self.metas[importee_id].has_dynamic_exports {
          ImportStatus::DynamicFallback { namespace_ref: importee.namespace_object_ref }
        } else {
          ImportStatus::NoMatch {}
        }
      }
    }
  }

  #[allow(clippy::too_many_lines)]
  fn match_import_with_export(
    &mut self,
    normal_modules: &IndexModules,
    ctx: &mut MatchingContext,
    mut tracker: ImportTracker,
  ) -> MatchImportKind {
    let tracking_span = tracing::trace_span!(
      "TRACKING_MATCH_IMPORT",
      importer = normal_modules[tracker.importer].stable_id(),
      importee = normal_modules[tracker.importee].stable_id(),
      imported_specifier = format!("{}", tracker.imported)
    );
    let _enter = tracking_span.enter();

    let mut ambiguous_results = vec![];
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
      let importer = &self.normal_modules[tracker.importer];
      let named_import = &importer.as_normal().unwrap().named_imports[&tracker.imported_as];
      let importer_record = &importer.as_normal().unwrap().import_records[named_import.record_id];

      let kind = match import_status {
        ImportStatus::CommonJS => match &tracker.imported {
          Specifier::Star => {
            MatchImportKind::Namespace { namespace_ref: importer_record.namespace_ref }
          }
          Specifier::Literal(alias) => MatchImportKind::NormalAndNamespace {
            namespace_ref: importer_record.namespace_ref,
            alias: alias.clone(),
          },
        },
        ImportStatus::DynamicFallback { namespace_ref } => match &tracker.imported {
          Specifier::Star => MatchImportKind::Namespace { namespace_ref },
          Specifier::Literal(alias) => {
            MatchImportKind::NormalAndNamespace { namespace_ref, alias: alias.clone() }
          }
        },
        ImportStatus::NoMatch { .. } => {
          break MatchImportKind::NoMatch;
        }
        ImportStatus::Found { symbol, potentially_ambiguous_export_star_refs, .. } => {
          for ambiguous_ref in &potentially_ambiguous_export_star_refs {
            let ambiguous_ref_owner = &normal_modules[ambiguous_ref.owner];
            if let Some(another_named_import) =
              ambiguous_ref_owner.as_normal().unwrap().named_imports.get(ambiguous_ref)
            {
              let rec = &ambiguous_ref_owner.as_normal().unwrap().import_records
                [another_named_import.record_id];
              let ambiguous_result = match &self.normal_modules[rec.resolved_module] {
                Module::Normal(importee) => self.match_import_with_export(
                  normal_modules,
                  &mut MatchingContext { tracker_stack: ctx.tracker_stack.clone() },
                  ImportTracker {
                    importer: ambiguous_ref_owner.idx(),
                    importee: importee.idx,
                    imported: another_named_import.imported.clone(),
                    imported_as: another_named_import.imported_as,
                  },
                ),
                Module::External(_) => {
                  MatchImportKind::Normal { symbol: another_named_import.imported_as }
                }
              };
              ambiguous_results.push(ambiguous_result);
            } else {
              ambiguous_results.push(MatchImportKind::Normal { symbol: *ambiguous_ref });
            }
          }

          // If this is a re-export of another import, continue for another
          // iteration of the loop to resolve that import as well
          let owner = &normal_modules[symbol.owner];
          if let Some(another_named_import) = owner.as_normal().unwrap().named_imports.get(&symbol)
          {
            let rec = &owner.as_normal().unwrap().import_records[another_named_import.record_id];
            match &self.normal_modules[rec.resolved_module] {
              Module::External(_) => {
                break MatchImportKind::Normal { symbol: another_named_import.imported_as };
              }
              Module::Normal(importee) => {
                tracker.importee = importee.idx;
                tracker.importer = owner.idx();
                tracker.imported = another_named_import.imported.clone();
                tracker.imported_as = another_named_import.imported_as;
                continue;
              }
            }
          }

          break MatchImportKind::Normal { symbol };
        }
        ImportStatus::_CommonJSWithoutExports => todo!(),
        ImportStatus::_Disabled => todo!(),
        ImportStatus::External => todo!(),
      };
      break kind;
    };

    tracing::trace!("ambiguous_results {:#?}", ambiguous_results);
    tracing::trace!("ret {:#?}", ret);

    for ambiguous_result in &ambiguous_results {
      if *ambiguous_result != ret {
        if let MatchImportKind::Normal { symbol } = ret {
          return MatchImportKind::Ambiguous {
            symbol_ref: symbol,
            potentially_ambiguous_symbol_refs: ambiguous_results
              .iter()
              .filter_map(|kind| match *kind {
                MatchImportKind::Normal { symbol } => Some(symbol),
                MatchImportKind::Namespace { namespace_ref }
                | MatchImportKind::NormalAndNamespace { namespace_ref, .. } => Some(namespace_ref),
                _ => None,
              })
              .collect(),
          };
        }

        unreachable!("symbol should always exist");
      }
    }

    if let Module::Normal(importee) = &self.normal_modules[tracker.importee] {
      if (self.options.shim_missing_exports || matches!(importee.module_type, ModuleType::Empty))
        && matches!(ret, MatchImportKind::NoMatch)
      {
        match &tracker.imported {
          Specifier::Star => unreachable!("star should always exist, no need to shim"),
          Specifier::Literal(imported) => {
            let shimmed_symbol_ref = self.metas[tracker.importee]
              .shimmed_missing_exports
              .entry(imported.clone())
              .or_insert_with(|| {
                self.symbols.create_symbol(tracker.importee, imported.clone().to_string().into())
              });
            return MatchImportKind::Normal { symbol: *shimmed_symbol_ref };
          }
        }
      }
    }

    ret
  }
}
