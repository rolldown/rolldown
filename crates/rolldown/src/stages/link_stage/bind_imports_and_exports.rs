use std::borrow::Cow;
use std::hash::{Hash, Hasher};

use arcstr::ArcStr;
use indexmap::IndexSet;
use oxc_str::CompactStr;
// TODO: The current implementation for matching imports is enough so far but incomplete. It needs to be refactored
// if we want more enhancements related to exports.
use rolldown_common::{
  EcmaModuleAstUsage, ExportsKind, ImportRecordIdx, IndexModules, MemberExprObjectReferencedType,
  MemberExprRefResolution, Module, ModuleIdx, ModuleType, NamespaceAlias, NormalModule,
  OutputFormat, ResolvedExport, Specifier, StmtInfos, SymbolOrMemberExprRef, SymbolRef,
  SymbolRefDb, SymbolRefFlags,
};
use rolldown_error::{
  AmbiguousExternalNamespaceModule, BuildDiagnostic, Diagnostics, EventKindSwitcher,
  NamespaceConflictExporter,
};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  ecmascript::{is_validate_identifier_name, legitimize_identifier_name},
  index_vec_ext::{IndexVecExt, IndexVecRefExt},
  indexmap::{FxIndexMap, FxIndexSet},
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

#[derive(Debug)]
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

pub enum RelationWithCommonjs {
  Commonjs,
  IndirectDependOnCommonjs,
}

impl PartialEq for MatchImportKindNormal {
  fn eq(&self, other: &Self) -> bool {
    self.symbol == other.symbol
  }
}

// Must agree with the `PartialEq` above: `reexports` is deliberately excluded from both, so that
// two resolutions of the same symbol hash and compare as one.
impl Hash for MatchImportKindNormal {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.symbol.hash(state);
  }
}

/// Bookkeeping for a collected star branch so it can be promoted if the primary path turns out to
/// be a cycle: the outer re-export hops accumulated up to the collection point, the branch's own
/// export symbol, and where the branch was collected — the star-exporting module (and the name
/// requested there) whose cached `resolved_exports` winner points into the cycle.
#[derive(Debug)]
struct StarBranchPromotion {
  reexports_prefix: Vec<SymbolRef>,
  export: SymbolRef,
  star_module: ModuleIdx,
  imported: Specifier,
}

/// Non-cycle star branches keyed by resolution, in collection order.
type DedupedStarBranches = FxIndexMap<MatchImportKind, StarBranchPromotion>;

#[derive(Debug, PartialEq, Eq, Hash)]
#[expect(clippy::box_collection)]
pub enum MatchImportKind {
  // "sourceIndex" and "ref" are in use
  Normal(MatchImportKindNormal),
  // "namespaceRef" and "alias" are in use
  Namespace {
    namespace_ref: SymbolRef,
  },
  // Both "matchImportNormal" and "matchImportNamespace"
  NormalAndNamespace {
    namespace_ref: SymbolRef,
    alias: CompactStr,
  },
  /// The import could not be evaluated due to a cycle; carries where it was detected so
  /// CIRCULAR_REEXPORT can be reported if no other branch resolves the binding.
  Cycle {
    importer: ModuleIdx,
    imported: Specifier,
  },
  // The import resolved to multiple symbols via "export * from"
  Ambiguous {
    symbol_ref: SymbolRef,
    potentially_ambiguous_symbol_refs: Box<Vec<SymbolRef>>,
  },
  NoMatch,
}

impl MatchImportKind {
  fn kind_name(&self) -> &'static str {
    match self {
      Self::Normal(_) => "normal",
      Self::Namespace { .. } => "namespace",
      Self::NormalAndNamespace { .. } => "normal-and-namespace",
      Self::Cycle { .. } => "cycle",
      Self::Ambiguous { .. } => "ambiguous",
      Self::NoMatch => "no-match",
    }
  }

  /// The symbol a resolution binds to, if it has one. Used to describe a resolution when reporting
  /// an ambiguous export.
  fn bound_symbol(&self) -> Option<SymbolRef> {
    match *self {
      MatchImportKind::Normal(MatchImportKindNormal { symbol, .. }) => Some(symbol),
      MatchImportKind::Namespace { namespace_ref }
      | MatchImportKind::NormalAndNamespace { namespace_ref, .. } => Some(namespace_ref),
      MatchImportKind::Cycle { .. }
      | MatchImportKind::Ambiguous { .. }
      | MatchImportKind::NoMatch => None,
    }
  }
}

#[derive(Debug)]
pub enum ImportStatus {
  /// The imported file has no matching export
  NoMatch,

  /// The imported file has a matching export
  Found {
    // owner: NormalModuleId,
    symbol: SymbolRef,
    potentially_ambiguous_export_star_refs: Vec<SymbolRef>,
  },

  /// The imported file is CommonJS and has unknown exports
  CommonJS,

  /// The import is missing but there is a dynamic fallback object
  DynamicFallback {
    namespace_ref: SymbolRef,
  },

  DynamicFallbackWithCommonjsReference {
    namespace_ref: SymbolRef,
    commonjs_symbol: SymbolRef,
  },

  /// The imported file is external and has unknown exports
  External(SymbolRef),
}

impl ImportStatus {
  fn kind_name(&self) -> &'static str {
    match self {
      Self::NoMatch => "no-match",
      Self::Found { .. } => "found",
      Self::CommonJS => "commonjs",
      Self::DynamicFallback { .. } => "dynamic-fallback",
      Self::DynamicFallbackWithCommonjsReference { .. } => {
        "dynamic-fallback-with-commonjs-reference"
      }
      Self::External(_) => "external",
    }
  }
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
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn bind_imports_and_exports(&mut self) {
    // Initialize `resolved_exports` to prepare for matching imports with exports
    self.metas.par_iter_mut_enumerated().for_each(|(module_id, meta)| {
      let Module::Normal(module) = &self.module_table[module_id] else {
        return;
      };
      let mut resolved_exports = module
        .named_exports
        .iter()
        .map(|(name, local)| {
          (name.clone(), ResolvedExport::new(local.referenced, local.came_from_commonjs))
        })
        .collect::<FxHashMap<_, _>>();

      let mut module_stack = vec![];
      // The star-export origin map only feeds `record_star_reexport_path`, which is strict-only.
      let mut star_export_record_by_name =
        self.options.is_strict_execution_order_enabled().then(FxHashMap::default);
      if module.has_star_export() || module.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport) {
        Self::add_exports_for_export_star(
          &self.module_table.modules,
          &mut resolved_exports,
          star_export_record_by_name.as_mut(),
          module_id,
          &mut module_stack,
          None,
        );
      }
      meta.resolved_exports = resolved_exports;
      meta.star_export_record_by_name = star_export_record_by_name.unwrap_or_default();
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
      diagnostics: Diagnostics::new(),
      external_import_binding_merger: FxHashMap::default(),
      side_effects_modules: &side_effects_modules,
      normal_symbol_exports_chain_map: &mut normal_symbol_exports_chain_map,
      star_reexport_records_by_imported_symbol: &mut self.star_reexport_records_by_imported_symbol,
      external_import_namespace_merger: FxHashMap::default(),
    };
    self.module_table.modules.iter().for_each(|module| {
      binding_ctx.match_imports_with_exports(module.idx());
    });

    self.diagnostics.extend(binding_ctx.diagnostics);

    self.external_import_namespace_merger = binding_ctx.external_import_namespace_merger;

    for (module_idx, map) in &binding_ctx.external_import_binding_merger {
      // `map` is a `FxHashMap` with randomized iteration order. Facade symbols must be created
      // deterministically, but only an external imported under more than one name has anything to
      // order — so sort by name only in that (rare) case and skip the work otherwise.
      // `sort_unstable_by` is fine here: the names are `FxHashMap` keys, so they're unique and
      // there are no ties for a stable sort to preserve.
      let mut entries = map.iter().collect::<Vec<_>>();
      if entries.len() > 1 {
        entries.sort_unstable_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
      }
      for (key, symbol_set) in entries {
        let name = if key.as_str() == "default" {
          // The default export has no intrinsic name, so we derive one from the local names that
          // import it. `symbol_set` collects them in non-deterministic module-load order, so pick
          // the candidate with the smallest `(exec_order, name)`: `exec_order` is a deterministic
          // `u32` assigned by `sort_modules` (which runs before this stage), so the chosen name is
          // stable across builds and the comparison is a cheap integer compare. This also matches
          // how the sibling `external_import_namespace_merger` is ordered (by `exec_order` in
          // `code_splitting.rs`).
          let key = symbol_set
            .iter()
            .min_by_key(|&&symbol_ref| {
              (
                self.module_table.modules[symbol_ref.owner].exec_order(),
                symbol_ref.name(&self.symbols),
              )
            })
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

          for ambiguous_ref in potentially_ambiguous_symbol_refs.iter() {
            let ambiguous_ref = self.symbols.canonical_ref_for(*ambiguous_ref);
            if main_ref != ambiguous_ref {
              continue 'next_export;
            }
          }
        }
        sorted_and_non_ambiguous_resolved_exports
          .push((exported_name.clone(), resolved_export.came_from_commonjs));
      }
      sorted_and_non_ambiguous_resolved_exports.sort_unstable();
      meta.sorted_and_non_ambiguous_resolved_exports =
        FxIndexMap::from_iter(sorted_and_non_ambiguous_resolved_exports);
    });
    if self.options.checks.contains(EventKindSwitcher::NamespaceConflict) {
      self.detect_namespace_conflicts();
    }
    self.record_namespace_consumed_star_reexport_paths();
    self.update_cjs_module_meta();
    self.resolve_member_expr_refs(&side_effects_modules, &normal_symbol_exports_chain_map);
    self.normal_symbol_exports_chain_map = normal_symbol_exports_chain_map;
  }

  fn detect_namespace_conflicts(&mut self) {
    let mut conflicts: FxHashMap<(CompactStr, Vec<SymbolRef>), (u32, ModuleIdx)> =
      FxHashMap::default();
    for (module_idx, meta) in self.metas.iter_enumerated() {
      if meta.resolved_exports.len() == meta.sorted_and_non_ambiguous_resolved_exports.len() {
        continue;
      }
      let Module::Normal(module) = &self.module_table[module_idx] else {
        continue;
      };
      for (export_name, resolved_export) in &meta.resolved_exports {
        let Some(potentially_ambiguous_symbol_refs) =
          &resolved_export.potentially_ambiguous_symbol_refs
        else {
          continue;
        };
        let mut canonical_refs = std::iter::once(resolved_export.symbol_ref)
          .chain(potentially_ambiguous_symbol_refs.iter().copied())
          .map(|symbol_ref| self.symbols.canonical_ref_for(symbol_ref))
          .collect::<Vec<_>>();
        canonical_refs.sort_unstable();
        canonical_refs.dedup();
        if canonical_refs.len() < 2 {
          continue;
        }
        let all_bound_local = canonical_refs.iter().all(|canonical_ref| {
          match &self.module_table[canonical_ref.owner] {
            Module::Normal(owner) => !owner.named_imports.contains_key(canonical_ref),
            Module::External(_) => false,
          }
        });
        if !all_bound_local {
          continue;
        }
        let candidate = (module.exec_order, module_idx);
        conflicts
          .entry((export_name.clone(), canonical_refs))
          .and_modify(|best| *best = (*best).min(candidate))
          .or_insert(candidate);
      }
    }

    let mut conflicts = conflicts.into_iter().collect::<Vec<_>>();
    conflicts.sort_unstable_by(|((a_name, _), a_best), ((b_name, _), b_best)| {
      a_best.cmp(b_best).then_with(|| a_name.cmp(b_name))
    });
    for ((export_name, _), (_, module_idx)) in conflicts {
      let Module::Normal(module) = &self.module_table[module_idx] else {
        continue;
      };
      let resolved_export = &self.metas[module_idx].resolved_exports[&export_name];
      let exporters = std::iter::once(resolved_export.symbol_ref)
        .chain(
          resolved_export
            .potentially_ambiguous_symbol_refs
            .iter()
            .flat_map(|refs| refs.iter().copied()),
        )
        .filter_map(|symbol_ref| {
          let owner = self.module_table[symbol_ref.owner].as_normal()?;
          let named_export = owner.named_exports.get(&export_name)?;
          Some(NamespaceConflictExporter {
            source: owner.source.clone(),
            module_id: owner.id.to_string(),
            stable_id: owner.stable_id.to_string(),
            span_of_identifier: named_export.span,
          })
        })
        .collect::<Vec<_>>();
      if exporters.len() < 2 {
        continue;
      }
      self.diagnostics.push(
        BuildDiagnostic::namespace_conflict(
          export_name.to_string(),
          module.id.to_string(),
          module.stable_id.to_string(),
          exporters,
        )
        .with_severity_warning(),
      );
    }
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
    let relation_with_commonjs_map = self
      .module_table
      .modules
      .iter()
      .filter_map(|module| {
        let module = module.as_normal()?;
        if module.exports_kind.is_commonjs() {
          Some((module.idx, RelationWithCommonjs::Commonjs))
        } else if self.metas[module.idx].has_dynamic_exports {
          Some((module.idx, RelationWithCommonjs::IndirectDependOnCommonjs))
        } else {
          None
        }
      })
      .collect::<FxHashMap<ModuleIdx, RelationWithCommonjs>>();

    let idx_to_symbol_ref_to_module_idx_map = self
      .module_table
      .par_iter_enumerated()
      .filter_map(|(idx, module)| {
        // a cjs module could only be referenced by normal modules.
        let module = module.as_normal()?;
        let mut named_import_to_cjs_module = FxHashMap::default();
        let mut import_record_ns_to_cjs_module = FxHashMap::default();
        module.named_imports.iter().for_each(|(_, named_import)| {
          let rec = &module.import_records[named_import.record_idx];
          if let Some(module_idx) = rec.resolved_module && relation_with_commonjs_map.contains_key(&module_idx) {
            named_import_to_cjs_module.insert(named_import.imported_as, module_idx);
          }
        });
        module.import_records.iter().for_each(|item| {
          if let Some(module_idx) = item.resolved_module && let Some(RelationWithCommonjs::Commonjs) = relation_with_commonjs_map.get(&module_idx) {
            import_record_ns_to_cjs_module.insert(item.namespace_ref, module_idx);
          }
        });
        (!named_import_to_cjs_module.is_empty() || !import_record_ns_to_cjs_module.is_empty()).then_some((idx, (named_import_to_cjs_module, import_record_ns_to_cjs_module)))
      })
      .collect::<FxHashMap<ModuleIdx, (FxHashMap<SymbolRef, ModuleIdx>, FxHashMap<SymbolRef, ModuleIdx>)>>();
    for (k, (named_import_to_cjs_module, import_record_ns_to_cjs_module)) in
      idx_to_symbol_ref_to_module_idx_map
    {
      let meta = &mut self.metas[k];
      meta.named_import_to_cjs_module = named_import_to_cjs_module;
      meta.import_record_ns_to_cjs_module = import_record_ns_to_cjs_module;
    }
  }

  fn add_exports_for_export_star(
    normal_modules: &IndexModules,
    resolve_exports: &mut FxHashMap<CompactStr, ResolvedExport>,
    mut star_export_record_by_name: Option<&mut FxHashMap<CompactStr, ImportRecordIdx>>,
    module_idx: ModuleIdx,
    module_stack: &mut Vec<ModuleIdx>,
    root_record: Option<ImportRecordIdx>,
  ) {
    if module_stack.contains(&module_idx) {
      return;
    }

    module_stack.push(module_idx);

    let Module::Normal(module) = &normal_modules[module_idx] else {
      return;
    };

    let cjs_reexport_modules: Vec<ModuleIdx> =
      if module.ast_usage.contains(EcmaModuleAstUsage::IsCjsReexport) {
        module
          .ecma_view
          .cjs_reexport_import_record_ids
          .iter()
          .filter_map(|&rec_idx| module.import_records[rec_idx].resolved_module)
          .collect()
      } else {
        vec![]
      };

    let star_exports = module
      .star_export_records()
      .map(|(rec_idx, module_idx)| (Some(rec_idx), module_idx))
      .chain(cjs_reexport_modules.into_iter().map(|module_idx| (None, module_idx)));
    for (rec_idx, dep_id) in star_exports {
      let root_record = root_record.or(rec_idx);
      let Module::Normal(dep_module) = &normal_modules[dep_id] else {
        continue;
      };
      // if matches!(dep_module.exports_kind, ExportsKind::CommonJs) {
      //   continue;
      // }

      for (exported_name, named_export) in &dep_module.named_exports {
        // ES6 export star statements ignore exports named "default"
        if exported_name.as_str() == "default" && !named_export.came_from_commonjs {
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
            if resolved_export.came_from_commonjs || named_export.came_from_commonjs {
              // CJS conflict: at least one side came from CJS (e.g., conditional re-exports
              // mixing ESM and CJS targets). Track these separately — they're expected runtime
              // branches, not static ambiguity errors.
              resolved_export
                .cjs_conflicting_symbol_refs
                .get_or_insert(Box::default())
                .push(named_export.referenced);
            } else {
              resolved_export
                .potentially_ambiguous_symbol_refs
                .get_or_insert(Box::default())
                .push(named_export.referenced);
            }
          }
        } else {
          resolve_exports.insert(
            exported_name.clone(),
            ResolvedExport::new(named_export.referenced, named_export.came_from_commonjs),
          );
          if let Some(root_record) = root_record
            && let Some(map) = star_export_record_by_name.as_deref_mut()
          {
            map.insert(exported_name.clone(), root_record);
          }
        }
      }

      Self::add_exports_for_export_star(
        normal_modules,
        resolve_exports,
        star_export_record_by_name.as_deref_mut(),
        dep_id,
        module_stack,
        root_record,
      );
    }

    module_stack.pop();
  }

  /// Strict-execution-order only: record the star re-export paths behind every non-ambiguous
  /// export of a statically namespace-imported module (`import * as ns`), keyed by the importee's
  /// namespace object. Dynamic imports are deliberately NOT scanned here: `import()` consumption
  /// flows through the entry export interface, and `create_exports_for_ecma_modules` already
  /// records those paths per referenced entry export (keyed by the export's symbol, gated by the
  /// entry-chunk arm of `collect_frozen_reexport_usage`).
  ///
  /// A namespace object consumed as a whole retains EVERY export — including bindings reached
  /// through `export *` chains that no named import or statically resolved member read records.
  /// Without a recorded path, an init-owning barrel on such a chain has no retained-re-export
  /// evidence for its excluded star hop, so its `init_*` never initializes a side-effect-free
  /// definer that only the namespace observes. `collect_frozen_reexport_usage` gates every
  /// namespace-keyed path on the namespace object actually being included, so a namespace
  /// tree-shaking drops retains nothing here.
  fn record_namespace_consumed_star_reexport_paths(&mut self) {
    if !self.options.is_strict_execution_order_enabled() {
      return;
    }
    let mut namespace_consumed = FxHashSet::default();
    for module in self.module_table.modules.iter().filter_map(|module| module.as_normal()) {
      for named_import in module.named_imports.values() {
        if matches!(named_import.imported, Specifier::Star)
          && let Some(importee_idx) = module.import_records[named_import.record_idx].resolved_module
        {
          namespace_consumed.insert(importee_idx);
        }
      }
    }
    for importee_idx in namespace_consumed {
      let Some(importee) = self.module_table[importee_idx].as_normal() else {
        continue;
      };
      // Key every path by the importee's namespace object, not by each export's canonical symbol:
      // an export used through a DIFFERENT route (a direct import elsewhere) must not retain a
      // barrel path nobody consumes. `collect_frozen_reexport_usage` gates a namespace-keyed path
      // on the namespace object actually being included — the exact breadth demand.
      let namespace_ref = importee.namespace_object_ref;
      let meta = &self.metas[importee_idx];
      for export_name in meta.sorted_and_non_ambiguous_resolved_exports.keys() {
        record_star_reexport_path(
          importee_idx,
          export_name,
          namespace_ref,
          &self.module_table.modules,
          &self.metas,
          &mut self.star_reexport_records_by_imported_symbol,
          &mut FxHashSet::default(),
        );
      }
    }
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
  #[expect(clippy::too_many_lines)]
  fn resolve_member_expr_refs(
    &mut self,
    side_effects_modules: &FxHashSet<ModuleIdx>,
    normal_symbol_exports_chain_map: &FxHashMap<SymbolRef, Vec<SymbolRef>>,
  ) {
    let warnings = append_only_vec::AppendOnlyVec::new();
    // A JSON default object whose `data.key` accesses get rewritten to the statically split
    // per-key exports must stay un-split once it is mutated or escapes (see
    // `collect_non_splittable_json_defaults`). Such a mutation/escape in one module invalidates
    // the optimization for reads of the same JSON default in *any* module (e.g. when the default
    // is mutated through a re-exporting wrapper), so the non-splittable set must be unioned across
    // the whole graph before any read is resolved below. Only pay for the scan when JSON exists.
    let has_json_module = self.module_table.modules.iter().any(|module| {
      module.as_normal().is_some_and(|module| matches!(module.module_type, ModuleType::Json))
    });
    let non_splittable_json_defaults: FxHashSet<SymbolRef> = if has_json_module {
      self
        .module_table
        .modules
        .par_iter()
        .zip(self.stmt_infos.par_iter())
        .map(|(module, stmt_infos)| match module {
          Module::Normal(_) => self.collect_non_splittable_json_defaults(stmt_infos),
          Module::External(_) => FxHashSet::default(),
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect()
    } else {
      FxHashSet::default()
    };
    let strict_execution_order = self.options.is_strict_execution_order_enabled();
    let resolved_meta_data = self
      .module_table
      .modules
      .par_iter()
      .zip(self.stmt_infos.par_iter())
      .map(|(module, stmt_infos)| match module {
        Module::Normal(module) => {
          let mut resolved_map = FxHashMap::default();
          let mut side_effects_dependency = vec![];
          let mut written_cjs_exports: Vec<SymbolRef> = vec![];
          let mut star_reexport_consumptions: Vec<(SymbolRef, Vec<(ModuleIdx, CompactStr)>)> =
            vec![];
          stmt_infos.iter().for_each(|stmt_info| {
            stmt_info.referenced_symbols.iter().for_each(|symbol_ref| {
              // `depended_refs` is used to store necessary symbols that must be included once the resolved symbol gets included
              let mut depended_refs: Vec<SymbolRef> = vec![];
              // Strict-only: every (module, export) step this member chain statically resolves
              // through, recorded so re-export hops consumed via a namespace read retain their
              // barrel-forwarding evidence exactly like named imports do.
              let mut star_reexport_steps: Vec<(ModuleIdx, CompactStr)> = vec![];

              if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = symbol_ref {
                // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
                let mut canonical_ref = self.symbols.canonical_ref_for(member_expr_ref.object_ref);
                let mut canonical_ref_owner: &NormalModule =
                  match &self.module_table[canonical_ref.owner] {
                    Module::Normal(module) => module,
                    Module::External(_) => return,
                  };
                // Treat `import data from './x.json'; data.foo` as a namespace access so
                // it can be optimized to the underlying `foo` export. Skip this for writes
                // (`data.foo = ...`) since rewriting the write target to a bare identifier
                // (or worse, an inlined constant) is unsound and would crash the finalizer.
                // Also skip all reads from a JSON default object that is non-splittable, i.e. it
                // is mutated or escapes anywhere in the graph (`collect_non_splittable_json_defaults`),
                // because then the split `foo` export no longer reflects the live `data.foo`.
                // Finally, skip when the first prop is `"default"`: `data` is already the
                // default export value, and the JSON module's `"default"` named export points
                // to that same symbol, so the optimization would resolve `.default` to `data`
                // itself and silently drop the access. Since JSON resolution is single-level,
                // gating at setup is sufficient — the loop never re-enters for a JSON module
                // after the first iteration.
                let is_json_import_ns = matches!(canonical_ref_owner.module_type, ModuleType::Json)
                  && member_expr_ref.object_ref_type == MemberExprObjectReferencedType::Default
                  && !member_expr_ref.is_write
                  && !non_splittable_json_defaults.contains(&canonical_ref)
                  && member_expr_ref
                    .prop_and_span_list
                    .first()
                    .is_none_or(|prop| prop.name.as_str() != "default");
                let mut is_namespace_ref =
                  canonical_ref_owner.namespace_object_ref == canonical_ref || is_json_import_ns;
                let mut cursor = 0;
                while cursor < member_expr_ref.prop_and_span_list.len() && is_namespace_ref {
                  let prop = &member_expr_ref.prop_and_span_list[cursor];
                  let name = &prop.name;
                  let meta = &self.metas[canonical_ref_owner.idx];
                  let export_symbol = meta.resolved_exports.get(name).and_then(|resolved_export| {
                    (!resolved_export.came_from_commonjs).then_some(resolved_export)
                  });
                  let Some(export_symbol) = export_symbol else {
                    // when we try to resolve `a.b.c`, and found that `b` is not exported by module
                    // that `a` pointed to, convert the `a.b.c` into `void 0` if module `a` do not
                    // have any dynamic exports.
                    if !self.metas[canonical_ref_owner.idx].has_dynamic_exports {
                      resolved_map.insert(
                        member_expr_ref.node_id,
                        MemberExprRefResolution {
                          resolved: if is_json_import_ns { Some(canonical_ref) } else { None },
                          prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..]
                            .to_vec(),
                          depended_refs: vec![],
                          target_commonjs_exported_symbol: None,
                          reference_id: member_expr_ref.reference_id,
                        },
                      );
                    }
                    if !self.metas[canonical_ref_owner.idx].has_dynamic_exports
                      && !is_json_import_ns
                    {
                      warnings.push(
                        BuildDiagnostic::import_is_undefined(
                          module.id.as_arc_str().clone(),
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
                  if !meta.sorted_and_non_ambiguous_resolved_exports.contains_key(name) {
                    resolved_map.insert(
                      member_expr_ref.node_id,
                      MemberExprRefResolution {
                        resolved: None,
                        prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..]
                          .to_vec(),
                        depended_refs: vec![],
                        target_commonjs_exported_symbol: None,
                        reference_id: member_expr_ref.reference_id,
                      },
                    );
                    return;
                  }

                  if strict_execution_order {
                    star_reexport_steps.push((canonical_ref_owner.idx, name.clone()));
                  }
                  depended_refs.push(export_symbol.symbol_ref);
                  if let Some(chains) =
                    normal_symbol_exports_chain_map.get(&export_symbol.symbol_ref)
                  {
                    depended_refs.extend(chains);
                    for item in chains {
                      if side_effects_modules.contains(&item.owner) {
                        side_effects_dependency.push(item.owner);
                      }
                    }
                  }
                  canonical_ref = self.symbols.canonical_ref_for(export_symbol.symbol_ref);
                  // If the canonical ref points to an external module, we can't resolve
                  // further properties statically. Break out of the loop and let the
                  // remaining properties be accessed at runtime.
                  let Some(normal_module) = self.module_table[canonical_ref.owner].as_normal()
                  else {
                    cursor += 1;
                    break;
                  };
                  canonical_ref_owner = normal_module;
                  cursor += 1;
                  is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
                }
                let mut target_commonjs_exported_symbol = None;
                // Although the last one may not be a namespace ref, but it may reference a cjs
                // import record namespace, which may reference a export in commonjs module.
                // TODO: we could record if a module could potential reference a cjs symbol
                // so that we could skip this step.
                if cursor < member_expr_ref.prop_and_span_list.len() {
                  let maybe_namespace = depended_refs
                    .last()
                    .copied()
                    .unwrap_or(self.symbols.canonical_ref_for(member_expr_ref.object_ref));
                  let maybe_namespace_symbol = self.symbols.get(maybe_namespace);
                  let continue_resolve =
                    if let Some(ref ns) = maybe_namespace_symbol.namespace_alias {
                      // If the property_name is not "default", it means the symbol reference a imported binding
                      // rather than a namespace object.
                      // e.g. import { foo } from './cjs'
                      ns.property_name.as_str() == "default"
                    } else {
                      true
                    };
                  // corresponding to cases in:
                  // https://github.com/rolldown/rolldown/blob/30a5a2fc8fa6785821153922e21dc0273cc00c7a/crates/rolldown/tests/rolldown/tree_shaking/commonjs/main.js?plain=1#L3-L10
                  let cjs_module_idx = continue_resolve
                    .then(|| {
                      self.metas[maybe_namespace.owner]
                        .named_import_to_cjs_module
                        .get(&maybe_namespace)
                        .or_else(|| {
                          self.metas[maybe_namespace.owner]
                            .import_record_ns_to_cjs_module
                            .get(&maybe_namespace)
                        })
                        .or_else(|| {
                          (self.metas[maybe_namespace.owner].has_dynamic_exports)
                            .then_some(&maybe_namespace.owner)
                        })
                        .copied()
                    })
                    .flatten();
                  if let Some(cjs_idx) = cjs_module_idx
                    && let Some(m) = self.metas[cjs_idx]
                      .resolved_exports
                      .get(&member_expr_ref.prop_and_span_list[cursor].name)
                      .and_then(|resolved_export| {
                        resolved_export.came_from_commonjs.then_some(resolved_export)
                      })
                  {
                    let is_default = member_expr_ref.prop_and_span_list[cursor].name == "default";
                    // When the accessed property is `default`, check if `.default` represents
                    // the whole `module.exports` (rather than `exports.default`). This is true when:
                    // - Node ESM mode: __toESM always ignores __esModule flag
                    // - Non-node mode without __esModule: __toESM sets .default = module.exports
                    // In these cases, skip resolving `.default` to a specific CJS export.
                    let default_is_module_exports = is_default && {
                      let is_node_esm = module.should_consider_node_esm_spec_for_static_import();
                      let importee_has_es_module_flag =
                        self.module_table[cjs_idx].as_normal().is_some_and(|importee| {
                          importee.ecma_view.ast_usage.contains(EcmaModuleAstUsage::EsModuleFlag)
                        });
                      is_node_esm || !importee_has_es_module_flag
                    };

                    // If the current property is `default` and it represents the whole `module.exports`,
                    // try to resolve the next property as a CJS export.
                    if default_is_module_exports
                      && let Some(next_prop) = member_expr_ref.prop_and_span_list.get(cursor + 1)
                    {
                      if let Some(property) = self.metas[cjs_idx]
                        .resolved_exports
                        .get(&next_prop.name)
                        .and_then(|resolved_export| {
                          resolved_export.came_from_commonjs.then_some(resolved_export)
                        })
                      {
                        let is_next_default = next_prop.name == "default";
                        if is_next_default && maybe_namespace_symbol.namespace_alias.is_none() {
                          // import * as ns; ns.default.default — can't optimize.
                          //
                          // __toESM sets import_ns.default = module.exports and __copyProps
                          // skips "default" (already set), so exports.default is only
                          // reachable via import_ns.default.default (two levels).
                          // If we advance cursor, props becomes ["default"] and the finalizer
                          // base is import_ns (#LOCAL_NAMESPACE has no namespace_alias to
                          // append .default), so the result is import_ns.default which is
                          // module.exports — not module.exports.default.
                          //
                          // Other non-"default" properties (e.g. ns.default.foo) work fine
                          // because __copyProps copies them onto the __toESM target, so
                          // import_ns.foo = module.exports.foo.
                        } else {
                          cursor += 1;
                          target_commonjs_exported_symbol =
                            Some((property.symbol_ref, is_next_default));
                          depended_refs.push(property.symbol_ref);

                          if member_expr_ref.is_write {
                            written_cjs_exports.push(property.symbol_ref);
                          }
                        }
                      }
                    } else if default_is_module_exports {
                      // `.default` represents the whole `module.exports` with no further
                      // property access. Leave target_commonjs_exported_symbol as None so
                      // that include_statements runs the CJS bailout check and keeps all
                      // exports for this opaque usage.
                    } else {
                      target_commonjs_exported_symbol = Some((m.symbol_ref, is_default));
                    }
                    depended_refs.push(m.symbol_ref);
                    // If this member expression is a write (e.g. `cjs.c = 'abcd'`), the
                    // CJS exported symbol should not be inlined as a constant since its
                    // value may change at runtime.
                    if member_expr_ref.is_write {
                      written_cjs_exports.push(m.symbol_ref);
                    }
                  }
                }

                if cursor > 0 {
                  // The module namespace might be created in the other module get imported via named import instead of `import * as`.
                  // We need to include the possible export chain.
                  depended_refs.push(member_expr_ref.object_ref);
                  normal_symbol_exports_chain_map.get(&member_expr_ref.object_ref).inspect(
                    |refs| {
                      depended_refs.extend(*refs);
                    },
                  );
                }

                if cursor > 0 || target_commonjs_exported_symbol.is_some() {
                  // Key the consumed steps by the final canonical ref: it is the symbol the
                  // inclusion pass marks used for this read (an inlined constant never is, and
                  // needs no init), which is exactly the usedness gate
                  // `collect_frozen_reexport_usage` applies to this index.
                  if !star_reexport_steps.is_empty() {
                    star_reexport_consumptions.push((canonical_ref, star_reexport_steps));
                  }
                  resolved_map.insert(
                    member_expr_ref.node_id,
                    MemberExprRefResolution {
                      resolved: Some(canonical_ref),
                      prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..]
                        .to_vec(),
                      depended_refs,
                      target_commonjs_exported_symbol,
                      reference_id: member_expr_ref.reference_id,
                    },
                  );
                }
              }
            });
          });

          (resolved_map, side_effects_dependency, written_cjs_exports, star_reexport_consumptions)
        }
        Module::External(_) => (FxHashMap::default(), vec![], vec![], vec![]),
      })
      .collect::<Vec<_>>();

    debug_assert_eq!(self.metas.len(), resolved_meta_data.len());
    self.diagnostics.extend(warnings);
    // Remove CJS exported symbols that are written to by importers from the constant map
    // to prevent incorrect inlining of mutated values.
    // First, collect statically-known written symbols gathered during resolution above.
    // Then, for imports flagged with `HasComputedMemberWrite` (dynamic computed writes like
    // `cjs[name] = value`, or writes through `ns.default`), bail out all CJS exports of the
    // target module since we can't determine which specific property is affected.
    let mut written_cjs_export_symbols: Vec<SymbolRef> = Vec::new();
    for (meta, (_, _, written_cjs_exports, _)) in self.metas.iter().zip(resolved_meta_data.iter()) {
      written_cjs_export_symbols.extend(written_cjs_exports);
      for (import_symbol, cjs_module_idx) in
        meta.named_import_to_cjs_module.iter().chain(meta.import_record_ns_to_cjs_module.iter())
      {
        if import_symbol
          .flags(&self.symbols)
          .is_some_and(|f| f.contains(SymbolRefFlags::HasComputedMemberWrite))
        {
          written_cjs_export_symbols.extend(
            self.metas[*cjs_module_idx]
              .resolved_exports
              .values()
              .filter(|e| e.came_from_commonjs)
              .map(|e| e.symbol_ref),
          );
        }
      }
    }
    for symbol_ref in &written_cjs_export_symbols {
      self.global_constant_symbol_map.remove(symbol_ref);
    }
    // A statically resolved namespace member read consumes re-export hops exactly like a named
    // import: without this, a side-effect-free definer reached only through an `export *` barrel
    // by namespace readers leaves the barrel's forwarding hop without retention evidence, and the
    // wrapped barrel's `init_*` never initializes the definer (`collect_frozen_reexport_usage`
    // keys its retained re-export paths off this same index).
    if strict_execution_order {
      let mut recorded = FxHashSet::default();
      for (_, _, _, star_reexport_consumptions) in &resolved_meta_data {
        for (imported_as_ref, steps) in star_reexport_consumptions {
          for (module_idx, export_name) in steps {
            if !recorded.insert((*module_idx, export_name.clone(), *imported_as_ref)) {
              continue;
            }
            record_star_reexport_path(
              *module_idx,
              export_name,
              *imported_as_ref,
              &self.module_table.modules,
              &self.metas,
              &mut self.star_reexport_records_by_imported_symbol,
              &mut FxHashSet::default(),
            );
          }
        }
      }
    }
    self.metas.iter_mut().zip(resolved_meta_data).for_each(
      |(meta, (resolved_map, side_effects_dependency, _, _))| {
        meta.resolved_member_expr_refs = resolved_map;
        meta.dependencies.extend(side_effects_dependency);
      },
    );
  }

  /// Collect the JSON-module default exports in `stmt_infos` that must NOT be split into their
  /// per-key named exports, because the live default object is observably mutated or escapes.
  ///
  /// The `data.foo` → split-`foo`-export rewrite (see `resolve_member_expr_refs`) is only sound
  /// while every reference to the default object is a plain `data.<key>` read. Once the object is
  /// mutated (`data.foo = ...`) or escapes — aliased (`const o = data`), passed to a function
  /// (`f(data)`), indexed dynamically (`data[k]`), returned, ... — the split exports may diverge
  /// from the live object, so the whole default has to stay materialized and the access left as-is.
  ///
  /// This scans a single module; the caller unions the results across the whole graph, since a
  /// mutation/escape in one module invalidates the optimization for reads of the same JSON default
  /// in any other module (e.g. through a re-exporting wrapper).
  fn collect_non_splittable_json_defaults(&self, stmt_infos: &StmtInfos) -> FxHashSet<SymbolRef> {
    let mut non_splittable = FxHashSet::default();
    for stmt_info in stmt_infos.iter() {
      for reference in &stmt_info.referenced_symbols {
        if let Some(json_default) = self.json_default_made_non_splittable_by(reference) {
          non_splittable.insert(json_default);
        }
      }
    }
    non_splittable
  }

  /// If `reference` makes a JSON default non-splittable (mutation or escape, see
  /// [`Self::collect_non_splittable_json_defaults`]), return that default's canonical `SymbolRef`.
  fn json_default_made_non_splittable_by(
    &self,
    reference: &SymbolOrMemberExprRef,
  ) -> Option<SymbolRef> {
    match reference {
      // A bare reference to the binding (anything other than a static `x.key` access) means the
      // default object flows somewhere we can't track: an alias (`const o = data`), a call
      // argument (`f(data)`), a dynamic index (`data[k]`, which also covers computed writes
      // `data[k] = ...`), a `return`, etc. Any of these may mutate or alias it, so bail. Static
      // `data.key` accesses are recorded as `MemberExpr` (below), never as a bare `Symbol`, so
      // this does not disable the optimization for the case it targets.
      SymbolOrMemberExprRef::Symbol(symbol_ref) => self.json_default_canonical_ref(*symbol_ref),
      SymbolOrMemberExprRef::MemberExpr(member_expr_ref) => {
        // Reads (`data.key`) are exactly the optimizable use — never bail on them.
        if !member_expr_ref.is_write {
          return None;
        }
        match member_expr_ref.object_ref_type {
          // `data.key = ...` where `data` is a default-imported JSON object, possibly reached
          // through a named re-export (`export { default as data } from './x.json'`).
          MemberExprObjectReferencedType::Default | MemberExprObjectReferencedType::Named => {
            self.json_default_canonical_ref(member_expr_ref.object_ref)
          }
          // `ns.default.key = ...` — the mutated object is the namespace's default export.
          MemberExprObjectReferencedType::Namespace
            if member_expr_ref
              .prop_and_span_list
              .first()
              .is_some_and(|prop| prop.name.as_str() == "default") =>
          {
            let canonical_ref = self.symbols.canonical_ref_for(member_expr_ref.object_ref);
            let is_json_namespace =
              self.module_table[canonical_ref.owner].as_normal().is_some_and(|module| {
                matches!(module.module_type, ModuleType::Json)
                  && module.namespace_object_ref == canonical_ref
              });
            if !is_json_namespace {
              return None;
            }
            self.metas[canonical_ref.owner]
              .resolved_exports
              .get("default")
              .map(|export| self.symbols.canonical_ref_for(export.symbol_ref))
          }
          MemberExprObjectReferencedType::Namespace => None,
        }
      }
    }
  }

  /// Canonicalize `symbol_ref` and return it iff it resolves to a JSON module's `default` export.
  fn json_default_canonical_ref(&self, symbol_ref: SymbolRef) -> Option<SymbolRef> {
    let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
    let is_json_default =
      self.module_table[canonical_ref.owner].as_normal().is_some_and(|module| {
        matches!(module.module_type, ModuleType::Json)
          && self.metas[canonical_ref.owner].resolved_exports.get("default").is_some_and(
            |default_export| {
              self.symbols.canonical_ref_for(default_export.symbol_ref) == canonical_ref
            },
          )
      });
    is_json_default.then_some(canonical_ref)
  }
}

struct BindImportsAndExportsContext<'a> {
  pub index_modules: &'a IndexModules,
  pub metas: &'a mut LinkingMetadataVec,
  pub symbol_db: &'a mut SymbolRefDb,
  pub options: &'a SharedOptions,
  pub diagnostics: Diagnostics,
  pub external_import_binding_merger:
    FxHashMap<ModuleIdx, FxHashMap<CompactStr, IndexSet<SymbolRef>>>,
  pub external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
  pub side_effects_modules: &'a FxHashSet<ModuleIdx>,
  pub normal_symbol_exports_chain_map: &'a mut FxHashMap<SymbolRef, Vec<SymbolRef>>,
  pub star_reexport_records_by_imported_symbol:
    &'a mut FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, ImportRecordIdx)>>>,
}

impl BindImportsAndExportsContext<'_> {
  fn match_imports_with_exports(&mut self, module_idx: ModuleIdx) {
    let Module::Normal(module) = &self.index_modules[module_idx] else {
      return;
    };
    let is_esm = matches!(self.options.format, OutputFormat::Esm);
    // Reused across all named imports of this module. `match_import_with_export` always starts
    // from an empty stack (and the ambiguous-reexport recursion gets its own cloned stack), so
    // clearing before each call preserves behavior while reusing this allocation instead of
    // allocating a fresh `Vec` per import.
    let mut matching_ctx = MatchingContext { tracker_stack: Vec::new() };
    for (imported_as_ref, named_import) in &module.named_imports {
      let match_import_span = tracing::trace_span!(
        "MATCH_IMPORT",
        module_id = module.stable_id.as_str(),
        imported_specifier = named_import.imported.to_string()
      );
      let _enter = match_import_span.enter();

      let rec = &module.import_records[named_import.record_idx];
      let Some(resolved_module_idx) = rec.resolved_module else { continue };
      if self.options.is_strict_execution_order_enabled()
        && let Specifier::Literal(name) = &named_import.imported
      {
        record_star_reexport_path(
          resolved_module_idx,
          name,
          *imported_as_ref,
          self.index_modules,
          self.metas,
          self.star_reexport_records_by_imported_symbol,
          &mut FxHashSet::default(),
        );
      }
      let is_external = matches!(self.index_modules[resolved_module_idx], Module::External(_));

      if is_esm && is_external {
        match named_import.imported {
          Specifier::Star => {
            self
              .external_import_namespace_merger
              .entry(resolved_module_idx)
              .or_default()
              .insert(*imported_as_ref);
          }
          Specifier::Literal(ref name)
            if self.metas[module_idx]
              .resolved_exports
              .iter()
              .all(|(_, resolved_export)| resolved_export.symbol_ref != *imported_as_ref) =>
          {
            self
              .external_import_binding_merger
              .entry(resolved_module_idx)
              .or_default()
              .entry(name.clone())
              .or_default()
              .insert(*imported_as_ref);
          }
          Specifier::Literal(_) => {}
        }
      }
      matching_ctx.tracker_stack.clear();
      let ret = self.match_import_with_export(
        self.index_modules,
        &mut matching_ctx,
        ImportTracker {
          importer: module_idx,
          importee: resolved_module_idx,
          imported: named_import.imported.clone(),
          imported_as: *imported_as_ref,
        },
      );

      tracing::trace!(result = ret.kind_name(), "matched import with export");
      match ret {
        MatchImportKind::Cycle { importer, imported } => {
          self.diagnostics.push(BuildDiagnostic::circular_reexport(
            self.index_modules[importer].id().to_string(),
            imported.to_string(),
          ));
        }
        MatchImportKind::Ambiguous { symbol_ref, potentially_ambiguous_symbol_refs } => {
          let importee = self.index_modules[resolved_module_idx].stable_id().to_string();

          let mut exporter = Vec::with_capacity(potentially_ambiguous_symbol_refs.len() + 1);
          if let Some(owner) = self.index_modules[symbol_ref.owner].as_normal() {
            if let Specifier::Literal(name) = &named_import.imported {
              if let Some(named_export) = owner.named_exports.get(name) {
                exporter.push(AmbiguousExternalNamespaceModule {
                  source: owner.source.clone(),
                  module_id: owner.id.to_string(),
                  stable_id: owner.stable_id.to_string(),
                  span_of_identifier: named_export.span,
                });
              }
            }
          }

          exporter.extend(potentially_ambiguous_symbol_refs.iter().filter_map(|&symbol_ref| {
            let normal_module = self.index_modules[symbol_ref.owner].as_normal()?;
            if let Specifier::Literal(name) = &named_import.imported {
              let named_export = normal_module.named_exports.get(name)?;
              return Some(AmbiguousExternalNamespaceModule {
                source: normal_module.source.clone(),
                module_id: normal_module.id.to_string(),
                stable_id: normal_module.stable_id.to_string(),
                span_of_identifier: named_export.span,
              });
            }

            None
          }));

          if exporter.is_empty() {
            self.diagnostics.push(BuildDiagnostic::missing_export(
              module.id.to_string(),
              module.stable_id.to_string(),
              self.index_modules[resolved_module_idx].id().to_string(),
              self.index_modules[resolved_module_idx].stable_id().to_string(),
              module.source.clone(),
              named_import.imported.to_string(),
              named_import.span_imported,
              None,
            ));
          } else {
            self.diagnostics.push(BuildDiagnostic::ambiguous_external_namespace(
              named_import.imported.to_string(),
              importee,
              AmbiguousExternalNamespaceModule {
                source: module.source.clone(),
                module_id: module.id.to_string(),
                stable_id: module.stable_id.to_string(),
                span_of_identifier: named_import.span_imported,
              },
              exporter,
            ));
          }
        }
        MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports }) => {
          for r in &reexports {
            if self.side_effects_modules.contains(&r.owner) {
              self.metas[module_idx].dependencies.insert(r.owner);
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

          // Propagate JSX flags from the imported symbol to its namespace_ref.
          // When we have: `import React from './cjs-module'; <React.Fragment/>`
          // `React` has UsedAsJSXMemberExprRoot, but the rendered binding is
          // the namespace_ref (`import_react`). Since namespace_ref is a facade
          // (generated) symbol, we promote it to MustStartWithCapitalLetterForJSX
          // so the renamer uppercases it (e.g. `Import_react`).
          if imported_as_ref.flags(self.symbol_db).is_some_and(|flags| {
            flags.intersects(
              SymbolRefFlags::MustStartWithCapitalLetterForJSX
                | SymbolRefFlags::UsedAsJSXMemberExprRoot,
            )
          }) {
            let ns_flags = namespace_ref.flags_mut(self.symbol_db);
            *ns_flags |= SymbolRefFlags::MustStartWithCapitalLetterForJSX;
          }
        }
        MatchImportKind::NoMatch => {
          let importee = &self.index_modules[resolved_module_idx];
          let is_ts_like_importing_ts_like =
            matches!(
              importee.as_normal().map(|m| &m.module_type),
              Some(ModuleType::Ts | ModuleType::Tsx)
            ) && matches!(module.module_type, ModuleType::Ts | ModuleType::Tsx);
          let diagnostic = BuildDiagnostic::missing_export(
            module.id.to_string(),
            module.stable_id.to_string(),
            importee.id().to_string(),
            importee.stable_id().to_string(),
            module.source.clone(),
            named_import.imported.to_string(),
            named_import.span_imported,
            is_ts_like_importing_ts_like.then(|| format!("If you meant to import a type rather than a value, make sure to add the `type` modifier (e.g. `import {{ type Foo }} from '{}'`).", rec.module_request))
          );
          self.diagnostics.push(diagnostic);
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
    let Some(importee_id) = importer.import_records[named_import.record_idx].resolved_module else {
      return ImportStatus::NoMatch {};
    };

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
            if export.came_from_commonjs {
              ImportStatus::DynamicFallbackWithCommonjsReference {
                namespace_ref: importee.namespace_object_ref,
                commonjs_symbol: export.symbol_ref,
              }
            } else {
              ImportStatus::Found {
                // owner: importee_id,
                symbol: export.symbol_ref,
                potentially_ambiguous_export_star_refs: export
                  .potentially_ambiguous_symbol_refs
                  .as_deref()
                  .cloned()
                  .unwrap_or_default(),
              }
            }
          }
          _ => {
            if self.metas[importee_id].has_dynamic_exports {
              ImportStatus::DynamicFallback { namespace_ref: importee.namespace_object_ref }
            } else {
              ImportStatus::NoMatch
            }
          }
        }
      }
    }
  }

  /// Apply `ResolveExport`'s null-cycle rule to the collected star branches. Cycle branches are
  /// dropped from the agreement set: `ResolveExport` skips a star branch that resolves to null
  /// instead of failing the whole lookup. Walking a cycle also re-visits the same module once per
  /// lap, which collects the surviving branches repeatedly — those duplicates would otherwise
  /// show up as repeated exporters, and repeated files, in the ambiguity diagnostic. An
  /// insertion-ordered map keeps source order for the "first branch wins" rule and the first
  /// collection's promotion state while deduplicating in linear time, since a name supplied by N
  /// distinct `export *` sources would otherwise make the scan quadratic.
  ///
  /// When the primary path was itself a cycle it resolves to null, so the binding is whatever the
  /// remaining branches say: per `ResolveExport` the first non-null branch wins and the rest only
  /// have to agree with it. The promoted branch's state is returned (its prefix already spliced
  /// into the result) so the caller can realign the collection module's cached winner.
  fn promote_first_surviving_star_branch(
    ret: MatchImportKind,
    ambiguous_results: Vec<(MatchImportKind, StarBranchPromotion)>,
  ) -> (MatchImportKind, DedupedStarBranches, Option<StarBranchPromotion>) {
    let mut deduped_ambiguous_results = DedupedStarBranches::default();
    for (result, promotion_state) in ambiguous_results {
      if !matches!(result, MatchImportKind::Cycle { .. }) {
        deduped_ambiguous_results.entry(result).or_insert(promotion_state);
      }
    }

    let (ret, promoted) = if matches!(ret, MatchImportKind::Cycle { .. }) {
      if let Some((mut promoted, mut promotion)) = deduped_ambiguous_results.shift_remove_index(0) {
        if let MatchImportKind::Normal(normal) = &mut promoted {
          let mut reexports = std::mem::take(&mut promotion.reexports_prefix);
          reexports.append(&mut normal.reexports);
          normal.reexports = reexports;
        }
        (promoted, Some(promotion))
      } else {
        (ret, None)
      }
    } else {
      (ret, None)
    };

    (ret, deduped_ambiguous_results, promoted)
  }

  fn match_import_with_export(
    &mut self,
    index_modules: &IndexModules,
    ctx: &mut MatchingContext,
    mut tracker: ImportTracker,
  ) -> MatchImportKind {
    let tracking_span = tracing::trace_span!(
      "TRACKING_MATCH_IMPORT",
      importer = index_modules[tracker.importer].stable_id().as_str(),
      importee = index_modules[tracker.importee].stable_id().as_str(),
      imported_specifier = tracker.imported.to_string()
    );
    let _enter = tracking_span.enter();

    let mut ambiguous_results = vec![];
    let mut reexports = vec![];
    // The ambiguous-branch recursion below inherits a clone of the caller's stack, so a cycle can
    // match a frame belonging to the *outer* request. Resolving that as null is still right for
    // THIS request, but such a resolution is context-dependent and must not be persisted into the
    // context-free `resolved_exports` cache: standalone, the collection module may resolve the
    // name to a different branch, or find it ambiguous. Frames at `stack_base` and beyond were
    // pushed by this invocation, so only a cycle hitting them justifies the cache write below.
    let stack_base = ctx.tracker_stack.len();
    let mut cycle_in_own_frames = false;
    let ret = loop {
      if let Some(matched) = ctx.tracker_stack.iter().rposition(|prev| {
        prev.importer == tracker.importer && prev.imported_as == tracker.imported_as
      }) {
        // ResolveExport treats an in-progress (module, binding) request as null.
        cycle_in_own_frames = matched >= stack_base;
        break MatchImportKind::Cycle {
          importer: tracker.importer,
          imported: tracker.imported.clone(),
        };
      }
      ctx.tracker_stack.push(tracker.clone());
      let import_status = self.advance_import_tracker(ctx);
      tracing::trace!(status = import_status.kind_name(), "advanced import tracker");
      let importer = &self.index_modules[tracker.importer];
      let named_import = &importer.as_normal().unwrap().named_imports[&tracker.imported_as];
      let importer_record = &importer.as_normal().unwrap().import_records[named_import.record_idx];

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
        ImportStatus::DynamicFallbackWithCommonjsReference { namespace_ref, commonjs_symbol } => {
          self.metas[tracker.importee].included_commonjs_export_symbol.insert(commonjs_symbol);
          match &tracker.imported {
            Specifier::Star => MatchImportKind::Namespace { namespace_ref },
            Specifier::Literal(alias) => {
              MatchImportKind::NormalAndNamespace { namespace_ref, alias: alias.clone() }
            }
          }
        }
        ImportStatus::NoMatch => {
          break MatchImportKind::NoMatch;
        }
        ImportStatus::Found { symbol, potentially_ambiguous_export_star_refs } => {
          for ambiguous_ref in &potentially_ambiguous_export_star_refs {
            let ambiguous_ref_owner = &index_modules[ambiguous_ref.owner];
            match ambiguous_ref_owner.as_normal().unwrap().named_imports.get(ambiguous_ref) {
              Some(another_named_import) => {
                let rec = &ambiguous_ref_owner.as_normal().unwrap().import_records
                  [another_named_import.record_idx];
                if let Some(resolved_module_idx) = rec.resolved_module {
                  let ambiguous_result = self.match_import_with_export(
                    index_modules,
                    &mut MatchingContext { tracker_stack: ctx.tracker_stack.clone() },
                    ImportTracker {
                      importer: ambiguous_ref_owner.idx(),
                      importee: resolved_module_idx,
                      imported: another_named_import.imported.clone(),
                      imported_as: another_named_import.imported_as,
                    },
                  );
                  let mut reexports_prefix = reexports.clone();
                  reexports_prefix.push(another_named_import.imported_as);
                  ambiguous_results.push((
                    ambiguous_result,
                    StarBranchPromotion {
                      reexports_prefix,
                      export: *ambiguous_ref,
                      star_module: tracker.importee,
                      imported: tracker.imported.clone(),
                    },
                  ));
                }
              }
              _ => {
                ambiguous_results.push((
                  MatchImportKind::Normal(MatchImportKindNormal {
                    symbol: *ambiguous_ref,
                    reexports: vec![],
                  }),
                  StarBranchPromotion {
                    reexports_prefix: reexports.clone(),
                    export: *ambiguous_ref,
                    star_module: tracker.importee,
                    imported: tracker.imported.clone(),
                  },
                ));
              }
            }
          }

          // If this is a re-export of another import, continue for another
          // iteration of the loop to resolve that import as well
          let owner = &index_modules[symbol.owner];
          if let Some(another_named_import) = owner.as_normal().unwrap().named_imports.get(&symbol)
          {
            let rec = &owner.as_normal().unwrap().import_records[another_named_import.record_idx];
            if let Some(resolved_module_idx) = rec.resolved_module {
              match &self.index_modules[resolved_module_idx] {
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
          }

          break MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports });
        }
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

    tracing::trace!(
      ambiguous_results = ambiguous_results.len(),
      result = ret.kind_name(),
      "matched import with export"
    );
    let (ret, deduped_ambiguous_results, promoted) =
      Self::promote_first_surviving_star_branch(ret, ambiguous_results);

    if let Some(symbol_ref) = ret.bound_symbol()
      && deduped_ambiguous_results.keys().any(|result| *result != ret)
    {
      return MatchImportKind::Ambiguous {
        symbol_ref,
        potentially_ambiguous_symbol_refs: Box::new(
          deduped_ambiguous_results.keys().filter_map(MatchImportKind::bound_symbol).collect(),
        ),
      };
    }

    // A star-exporting module on the walked chain cached the cycle branch as its
    // `resolved_exports` winner (`add_exports_for_export_star` records the first branch in source
    // order — the one this walk just found circular), and module interfaces are generated from
    // that raw winner (e.g. the per-module exports under `preserveModules`), so re-point it at
    // the promoted branch. The write targets the module the branch was collected from: that is
    // the module whose cached winner is stale. The loop-final `tracker.importee` is merely where
    // the lap closed — for cycles longer than one named hop it is a different module whose own
    // direct export must not be clobbered. `cycle_in_own_frames` keeps context-dependent
    // resolutions (cycles against an outer request's frames) out of the cache. Per slot the write
    // is idempotent — the promoted branch for a given (module, name) is deterministic — but a
    // cycle threading several star modules only gets the collection module realigned; the other
    // stale winners on the lap are a known limitation of this cache design.
    if cycle_in_own_frames
      && matches!(&ret, MatchImportKind::Normal(_))
      && let Some(StarBranchPromotion { export, star_module, imported, .. }) = promoted
      && let Specifier::Literal(imported) = &imported
      && let Some(resolved_export) = self.metas[star_module].resolved_exports.get_mut(imported)
    {
      resolved_export.symbol_ref = export;
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

pub(super) fn record_star_reexport_path(
  module_idx: ModuleIdx,
  export_name: &CompactStr,
  imported_as_ref: SymbolRef,
  modules: &IndexModules,
  metas: &LinkingMetadataVec,
  records_by_imported_symbol: &mut FxHashMap<SymbolRef, Vec<Vec<(ModuleIdx, ImportRecordIdx)>>>,
  visited: &mut FxHashSet<(ModuleIdx, CompactStr)>,
) {
  let mut path = vec![];
  collect_star_reexport_path(module_idx, export_name, modules, metas, &mut path, visited);
  if path.is_empty() {
    return;
  }
  let paths = records_by_imported_symbol.entry(imported_as_ref).or_default();
  if !paths.contains(&path) {
    paths.push(path);
  }
}

fn collect_star_reexport_path(
  module_idx: ModuleIdx,
  export_name: &CompactStr,
  modules: &IndexModules,
  metas: &LinkingMetadataVec,
  path: &mut Vec<(ModuleIdx, ImportRecordIdx)>,
  visited: &mut FxHashSet<(ModuleIdx, CompactStr)>,
) {
  if !visited.insert((module_idx, export_name.clone())) {
    return;
  }
  let Some(module) = modules[module_idx].as_normal() else {
    return;
  };
  let (record_idx, next_export_name) = if let Some(record_idx) =
    metas[module_idx].star_export_record_by_name.get(export_name).copied()
  {
    (record_idx, export_name.clone())
  } else {
    let Some(named_export) = module.named_exports.get(export_name) else {
      return;
    };
    let Some(named_import) = module.named_imports.get(&named_export.referenced) else {
      return;
    };
    let Specifier::Literal(next_export_name) = &named_import.imported else {
      return;
    };
    (named_import.record_idx, next_export_name.clone())
  };
  path.push((module_idx, record_idx));
  let Some(importee_idx) = module.import_records[record_idx].resolved_module else {
    return;
  };
  collect_star_reexport_path(importee_idx, &next_export_name, modules, metas, path, visited);
}
