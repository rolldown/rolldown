use std::{borrow::Cow, collections::hash_map::Entry};

use arcstr::ArcStr;
use indexmap::IndexSet;
use oxc::span::CompactStr;
// TODO: The current implementation for matching imports is enough so far but incomplete. It needs to be refactored
// if we want more enhancements related to exports.
use rolldown_common::{
  EcmaModuleAstUsage, ExportsKind, IndexModules, Module, ModuleIdx, ModuleType, NamespaceAlias,
  NormalModule, OutputFormat, ResolvedExport, Specifier, SymbolOrMemberExprRef, SymbolRef,
  SymbolRefDb,
};
use rolldown_error::{AmbiguousExternalNamespaceModule, BuildDiagnostic};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_std_utils::OptionExt;
use rolldown_utils::{
  ecmascript::{is_validate_identifier_name, legitimize_identifier_name},
  index_vec_ext::IndexVecExt,
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{SharedOptions, types::linking_metadata::LinkingMetadataVec};

use super::LinkStage;

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
  External(SymbolRef),
}

impl LinkStage<'_> {
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
  pub(super) fn bind_imports_and_exports(&mut self) {
    // Initialize `resolved_exports` to prepare for matching imports with exports
    self.metas.par_iter_mut_enumerated().for_each(|(module_id, meta)| {
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
      if module.has_star_export() {
        Self::add_exports_for_export_star(
          &self.module_table.modules,
          &mut resolved_exports,
          module_id,
          &mut module_stack,
        );
      }
      meta.resolved_exports = resolved_exports;
    });
    let side_effects_modules = self
      .module_table
      .modules
      .iter_enumerated()
      .filter(|(_, item)| item.side_effects().has_side_effects())
      .map(|(idx, _)| idx)
      .collect::<FxHashSet<ModuleIdx>>();
    let mut normal_symbol_exports_chain_map = FxHashMap::default();
    let mut binding_ctx = BindImportsAndExportsContext {
      index_modules: &self.module_table.modules,
      metas: &mut self.metas,
      symbol_db: &mut self.symbols,
      options: self.options,
      errors: Vec::default(),
      warnings: Vec::default(),
      external_import_binding_merger: FxHashMap::default(),
      side_effects_modules: &side_effects_modules,
      normal_symbol_exports_chain_map: &mut normal_symbol_exports_chain_map,
    };
    self.module_table.modules.iter().for_each(|module| {
      binding_ctx.match_imports_with_exports(module.idx());
    });

    self.errors.extend(binding_ctx.errors);
    self.warnings.extend(binding_ctx.warnings);

    for (module_idx, map) in &binding_ctx.external_import_binding_merger {
      for (key, symbol_set) in map {
        let name = if key.as_str() == "default" {
          let key = symbol_set
            .first()
            .map_or_else(|| key.clone(), |sym_ref| sym_ref.name(&self.symbols).into());
          Cow::Owned(key)
        } else if is_validate_identifier_name(key.as_str()) {
          Cow::Borrowed(key)
        } else {
          let legal_name = legitimize_identifier_name(key);
          Cow::Owned(legal_name.as_ref().into())
        };

        let target_symbol = self.symbols.create_facade_root_symbol_ref(*module_idx, &name);
        for symbol_ref in symbol_set {
          self.symbols.link(*symbol_ref, target_symbol);
        }
      }
    }
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
    self.resolve_member_expr_refs(&side_effects_modules, &normal_symbol_exports_chain_map);
    self.update_cjs_module_meta();
  }

  /// Update the metadata of CommonJS modules.
  /// - Safe to eliminate interop default export
  ///   e.g.
  /// ```js
  /// // index.js
  /// import a from './a'
  /// a.something // this property could safely rewrite to `a.something` rather than `a.default.something`
  ///
  /// // a.js
  /// module.exports = require('./mod.js')
  ///
  /// // mod.js
  /// exports.something = 1
  /// ```
  fn update_cjs_module_meta(&mut self) {
    enum CacheStatus {
      Seen,
      Value(bool),
    }
    /// caller should guarantee that the idx of module belongs to a normal module
    fn recursive_update_cjs_module_interop_default_removable(
      module_tables: &IndexModules,
      module_idx: ModuleIdx,
      cache: &mut FxHashMap<ModuleIdx, CacheStatus>,
    ) -> bool {
      match cache.entry(module_idx) {
        Entry::Occupied(mut occ) => {
          match occ.get_mut() {
            // Find a cycle
            CacheStatus::Seen => return false,
            CacheStatus::Value(v) => return *v,
          }
        }
        Entry::Vacant(vac) => {
          vac.insert(CacheStatus::Seen);
        }
      };
      let module = module_tables[module_idx].as_normal().unwrap();
      let v = if module.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport) {
        module.import_records.iter().filter(|item| !item.is_dummy()).all(|item| {
          let Some(importee) = module_tables[item.resolved_module].as_normal() else {
            return false;
          };
          if importee.exports_kind.is_commonjs() {
            recursive_update_cjs_module_interop_default_removable(
              module_tables,
              importee.idx,
              cache,
            )
          } else {
            false
          }
        })
      } else {
        module.ast_usage.contains(EcmaModuleAstUsage::AllStaticExportPropertyAccess)
      };
      cache.insert(module_idx, CacheStatus::Value(v));
      v
    }

    let mut cache = FxHashMap::default();
    let cjs_exports_type_modules = self
      .module_table
      .modules
      .iter()
      .filter_map(|module| {
        let Module::Normal(module) = module else {
          return None;
        };
        if module.exports_kind.is_commonjs() { Some(module.idx) } else { None }
      })
      .collect::<Vec<_>>();
    for module_idx in cjs_exports_type_modules {
      let v = recursive_update_cjs_module_interop_default_removable(
        &self.module_table.modules,
        module_idx,
        &mut cache,
      );
      self.metas[module_idx].safe_cjs_to_eliminate_interop_default = v;
    }
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
  fn resolve_member_expr_refs(
    &mut self,
    side_effects_modules: &FxHashSet<ModuleIdx>,
    normal_symbol_exports_chain_map: &FxHashMap<SymbolRef, Vec<SymbolRef>>,
  ) {
    let warnings = append_only_vec::AppendOnlyVec::new();
    let resolved_meta_data = self
      .module_table
      .modules
      .par_iter()
      .map(|module| match module {
        Module::Normal(module) => {
          let mut resolved_map = FxHashMap::default();
          let mut side_effects_dependency = vec![];
          module.stmt_infos.iter().for_each(|stmt_info| {
            stmt_info.referenced_symbols.iter().for_each(|symbol_ref| {
              if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = symbol_ref {
                // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
                let mut canonical_ref = self.symbols.canonical_ref_for(member_expr_ref.object_ref);
                let mut canonical_ref_owner: &NormalModule =
                  match &self.module_table.modules[canonical_ref.owner] {
                    Module::Normal(module) => module,
                    Module::External(_) => return,
                  };
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
                      resolved_map.insert(
                        member_expr_ref.span,
                        (None, member_expr_ref.props[cursor..].to_vec()),
                      );
                      warnings.push(
                        BuildDiagnostic::import_is_undefined(
                          module.id.resource_id().clone(),
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
                    resolved_map.insert(
                      member_expr_ref.span,
                      (None, member_expr_ref.props[cursor..].to_vec()),
                    );
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
                  if let Some(chains) =
                    normal_symbol_exports_chain_map.get(&export_symbol.symbol_ref)
                  {
                    for item in chains {
                      if side_effects_modules.contains(&item.owner) {
                        side_effects_dependency.push(item.owner);
                      }
                    }
                  }
                  ns_symbol_list.push((canonical_ref, name.to_rstr()));
                  canonical_ref = self.symbols.canonical_ref_for(export_symbol.symbol_ref);
                  canonical_ref_owner =
                    self.module_table.modules[canonical_ref.owner].as_normal().unwrap();
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

          (resolved_map, side_effects_dependency)
        }
        Module::External(_) => (FxHashMap::default(), vec![]),
      })
      .collect::<Vec<_>>();

    debug_assert_eq!(self.metas.len(), resolved_meta_data.len());
    self.warnings.extend(warnings);
    self.metas.iter_mut_enumerated().zip(resolved_meta_data).for_each(
      |((_idx, meta), (resolved_map, side_effects_dependency))| {
        meta.resolved_member_expr_refs = resolved_map;
        meta.dependencies.extend(side_effects_dependency);
      },
    );
  }
}

struct BindImportsAndExportsContext<'a> {
  pub index_modules: &'a IndexModules,
  pub metas: &'a mut LinkingMetadataVec,
  pub symbol_db: &'a mut SymbolRefDb,
  pub options: &'a SharedOptions,
  pub errors: Vec<BuildDiagnostic>,
  pub warnings: Vec<BuildDiagnostic>,
  pub external_import_binding_merger:
    FxHashMap<ModuleIdx, FxHashMap<CompactStr, IndexSet<SymbolRef>>>,
  pub side_effects_modules: &'a FxHashSet<ModuleIdx>,
  pub normal_symbol_exports_chain_map: &'a mut FxHashMap<SymbolRef, Vec<SymbolRef>>,
}

impl BindImportsAndExportsContext<'_> {
  #[allow(clippy::too_many_lines)]
  fn match_imports_with_exports(&mut self, module_id: ModuleIdx) {
    let Module::Normal(module) = &self.index_modules[module_id] else {
      return;
    };
    let is_esm = matches!(self.options.format, OutputFormat::Esm);
    for (imported_as_ref, named_import) in &module.named_imports {
      let match_import_span = tracing::trace_span!(
        "MATCH_IMPORT",
        module_id = module.stable_id,
        imported_specifier = named_import.imported.to_string()
      );
      let _enter = match_import_span.enter();

      let rec = &module.import_records[named_import.record_id];
      let is_external = matches!(self.index_modules[rec.resolved_module], Module::External(_));

      if is_esm
        && is_external
        && self.metas[module_id]
          .resolved_exports
          .iter()
          .all(|(_, resolved_export)| resolved_export.symbol_ref != *imported_as_ref)
        && matches!(named_import.imported, Specifier::Literal(_))
      {
        self
          .external_import_binding_merger
          .entry(rec.resolved_module)
          .or_default()
          .entry(imported_as_ref.name(self.symbol_db).into())
          .or_default()
          .insert(*imported_as_ref);
      }
      let ret = self.match_import_with_export(
        self.index_modules,
        &mut MatchingContext { tracker_stack: Vec::default() },
        ImportTracker {
          importer: module_id,
          importee: rec.resolved_module,
          imported: named_import.imported.clone(),
          imported_as: *imported_as_ref,
        },
      );
      tracing::trace!("Got match result {:?}", ret);
      match ret {
        MatchImportKind::_Ignore | MatchImportKind::Cycle => {}
        MatchImportKind::Ambiguous { symbol_ref, potentially_ambiguous_symbol_refs } => {
          let importee = self.index_modules[rec.resolved_module].stable_id().to_string();

          let mut exporter = Vec::with_capacity(potentially_ambiguous_symbol_refs.len() + 1);
          if let Some(owner) = self.index_modules[symbol_ref.owner].as_normal() {
            if let Specifier::Literal(name) = &named_import.imported {
              let named_export = &owner.named_exports[name];
              exporter.push(AmbiguousExternalNamespaceModule {
                source: owner.source.clone(),
                filename: owner.stable_id.to_string(),
                span_of_identifier: named_export.span,
              });
            }
          }

          exporter.extend(potentially_ambiguous_symbol_refs.iter().filter_map(|&symbol_ref| {
            let normal_module = self.index_modules[symbol_ref.owner].as_normal()?;
            if let Specifier::Literal(name) = &named_import.imported {
              let named_export = &normal_module.named_exports[name];
              return Some(AmbiguousExternalNamespaceModule {
                source: normal_module.source.clone(),
                filename: normal_module.stable_id.to_string(),
                span_of_identifier: named_export.span,
              });
            }

            None
          }));

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
        MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports }) => {
          for r in &reexports {
            if self.side_effects_modules.contains(&r.owner) {
              self.metas[module_id].dependencies.insert(r.owner);
            }
          }
          self.normal_symbol_exports_chain_map.insert(*imported_as_ref, reexports);

          self.symbol_db.link(*imported_as_ref, symbol);
        }
        MatchImportKind::Namespace { namespace_ref } => {
          self.symbol_db.link(*imported_as_ref, namespace_ref);
        }
        MatchImportKind::NormalAndNamespace { namespace_ref, alias } => {
          self.symbol_db.get_mut(*imported_as_ref).namespace_alias =
            Some(NamespaceAlias { property_name: alias, namespace_ref });
        }
        MatchImportKind::NoMatch => {
          let importee = &self.index_modules[rec.resolved_module];
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

  fn advance_import_tracker(&self, ctx: &MatchingContext) -> ImportStatus {
    let tracker = ctx.current_tracker();
    let importer = &self.index_modules[tracker.importer]
      .as_normal()
      .expect("only normal module can be importer");
    let named_import = &importer.named_imports[&tracker.imported_as];

    // Is this an external file?
    let importee_id = importer.import_records[named_import.record_id].resolved_module;
    let importee = match &self.index_modules[importee_id] {
      Module::Normal(importee) => importee.as_ref(),
      Module::External(external) => return ImportStatus::External(external.namespace_ref),
    };

    // Is this a named import of a file without any exports?
    debug_assert!(
      matches!(importee.exports_kind, ExportsKind::Esm | ExportsKind::CommonJs)
        || importee.meta.has_lazy_export()
        || importee.module_type == ModuleType::Empty
    );
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
          _ => {
            if self.metas[importee_id].has_dynamic_exports {
              ImportStatus::DynamicFallback { namespace_ref: importee.namespace_object_ref }
            } else {
              ImportStatus::NoMatch {}
            }
          }
        }
      }
    }
  }

  #[allow(clippy::too_many_lines)]
  fn match_import_with_export(
    &mut self,
    index_modules: &IndexModules,
    ctx: &mut MatchingContext,
    mut tracker: ImportTracker,
  ) -> MatchImportKind {
    let tracking_span = tracing::trace_span!(
      "TRACKING_MATCH_IMPORT",
      importer = index_modules[tracker.importer].stable_id(),
      importee = index_modules[tracker.importee].stable_id(),
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
      let importer = &self.index_modules[tracker.importer];
      let named_import = &importer.as_normal().unwrap().named_imports[&tracker.imported_as];
      let importer_record = &importer.as_normal().unwrap().import_records[named_import.record_id];
      let importee = &self.index_modules[importer_record.resolved_module];

      let kind = match import_status {
        ImportStatus::CommonJS => {
          let esm_namespace = if importer.as_normal().unpack().should_consider_node_esm_spec() {
            importee.as_normal().unpack().esm_namespace_in_cjs_node_mode.unpack_ref().namespace_ref
          } else {
            importee.as_normal().unpack().esm_namespace_in_cjs.unpack_ref().namespace_ref
          };
          match &tracker.imported {
            Specifier::Star => MatchImportKind::Namespace { namespace_ref: esm_namespace },
            Specifier::Literal(alias) => MatchImportKind::NormalAndNamespace {
              namespace_ref: esm_namespace,
              alias: alias.clone(),
            },
          }
        }
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
            let ambiguous_ref_owner = &index_modules[ambiguous_ref.owner];
            match ambiguous_ref_owner.as_normal().unwrap().named_imports.get(ambiguous_ref) {
              Some(another_named_import) => {
                let rec = &ambiguous_ref_owner.as_normal().unwrap().import_records
                  [another_named_import.record_id];
                let ambiguous_result = self.match_import_with_export(
                  index_modules,
                  &mut MatchingContext { tracker_stack: ctx.tracker_stack.clone() },
                  ImportTracker {
                    importer: ambiguous_ref_owner.idx(),
                    importee: rec.resolved_module,
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
          let owner = &index_modules[symbol.owner];
          if let Some(another_named_import) = owner.as_normal().unwrap().named_imports.get(&symbol)
          {
            let rec = &owner.as_normal().unwrap().import_records[another_named_import.record_id];
            match &self.index_modules[rec.resolved_module] {
              Module::External(_) => {
                break MatchImportKind::Normal(MatchImportKindNormal {
                  symbol: another_named_import.imported_as,
                  reexports: vec![],
                });
              }
              Module::Normal(importee) => {
                tracker.importee = importee.idx;
                tracker.importer = owner.idx();
                tracker.imported = another_named_import.imported.clone();
                tracker.imported_as = another_named_import.imported_as;
                reexports.push(another_named_import.imported_as);
                continue;
              }
            }
          }

          break MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports });
        }
        ImportStatus::_CommonJSWithoutExports => todo!(),
        ImportStatus::_Disabled => todo!(),
        ImportStatus::External(symbol_ref) => {
          if self.options.format.keep_esm_import_export_syntax() {
            // Imports from external modules should not be converted to CommonJS
            // if the output format preserves the original ES6 import statements
            break MatchImportKind::Normal(MatchImportKindNormal {
              symbol: tracker.imported_as,
              reexports: vec![],
            });
          }

          match &tracker.imported {
            Specifier::Star => MatchImportKind::Namespace { namespace_ref: symbol_ref },
            Specifier::Literal(alias) => MatchImportKind::NormalAndNamespace {
              namespace_ref: symbol_ref,
              alias: alias.clone(),
            },
          }
        }
      };
      break kind;
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

    if let Module::Normal(importee) = &self.index_modules[tracker.importee] {
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
                self.symbol_db.create_facade_root_symbol_ref(tracker.importee, imported.as_str())
              });
            return MatchImportKind::Normal(MatchImportKindNormal {
              symbol: *shimmed_symbol_ref,
              reexports: vec![],
            });
          }
        }
      }
    }

    ret
  }
}
