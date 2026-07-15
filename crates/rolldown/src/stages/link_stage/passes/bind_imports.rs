use std::{borrow::Cow, convert::Infallible};

use indexmap::IndexSet;
use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{
  ExportsKind, IndexModules, Module, ModuleIdx, ModuleTable, ModuleType, NamedImport,
  NamespaceAlias, NormalModule, OutputFormat, ResolvedExport, ResolvedImportRecord, Specifier,
  SymbolRef, SymbolRefDb, SymbolRefFlags,
};
use rolldown_error::{AmbiguousExternalNamespaceModule, BuildDiagnostic};
use rolldown_utils::{
  ecmascript::{is_validate_identifier_name, legitimize_identifier_name},
  indexmap::FxIndexSet,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  BindImportsPass, collect_initial_dependencies::ModuleDependenciesDraft,
  collect_resolved_exports::ResolvedExportsDraft, compute_dynamic_exports::DynamicExports,
  compute_module_execution_order::ModuleExecutionOrders, determine_module_formats::ModuleFormats,
  determine_module_side_effects::ModuleSideEffects,
};

// See internal-docs/pass-based-pipeline/implementation.md.

type ShimSlots = IndexVec<ModuleIdx, Option<FxHashMap<CompactStr, SymbolRef>>>;
type IncludedCommonJsSlots = IndexVec<ModuleIdx, Option<FxHashSet<SymbolRef>>>;
type DependencySlots = IndexVec<ModuleIdx, FxIndexSet<ModuleIdx>>;
type ExternalBindingGroups = FxHashMap<ModuleIdx, FxHashMap<CompactStr, IndexSet<SymbolRef>>>;

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct BindImportsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub resolved_exports: &'a ResolvedExportsDraft,
  pub module_formats: &'a ModuleFormats,
  pub dynamic_exports: &'a DynamicExports,
  pub module_side_effects: &'a ModuleSideEffects,
  pub execution_orders: &'a ModuleExecutionOrders,
  pub output_format: OutputFormat,
  pub shim_missing_exports: bool,
}

pub(in crate::stages::link_stage) struct BindImportsOwned {
  pub symbols: SymbolRefDb,
  pub dependencies: ModuleDependenciesDraft,
}

pub(in crate::stages::link_stage) struct ShimmedMissingExports {
  slots: ShimSlots,
}

impl ShimmedMissingExports {
  pub(in crate::stages::link_stage) fn into_slots(
    self,
  ) -> IndexVec<ModuleIdx, Option<FxHashMap<CompactStr, SymbolRef>>> {
    self.slots
  }
}

pub(in crate::stages::link_stage) struct IncludedCommonJsExportSymbols {
  slots: IncludedCommonJsSlots,
}

impl IncludedCommonJsExportSymbols {
  pub(in crate::stages::link_stage) fn into_slots(
    self,
  ) -> IndexVec<ModuleIdx, Option<FxHashSet<SymbolRef>>> {
    self.slots
  }
}

pub(in crate::stages::link_stage) struct NormalExportChains {
  chains: FxHashMap<SymbolRef, Vec<SymbolRef>>,
}

impl NormalExportChains {
  pub(in crate::stages::link_stage) fn into_inner(self) -> FxHashMap<SymbolRef, Vec<SymbolRef>> {
    self.chains
  }
}

pub(in crate::stages::link_stage) struct ExternalImportNamespaceMerges {
  merges: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
}

impl ExternalImportNamespaceMerges {
  pub(in crate::stages::link_stage) fn into_inner(
    self,
  ) -> FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>> {
    self.merges
  }
}

/// One-call ownership envelope. The driver must destructure this immediately; no pass accepts it.
pub(in crate::stages::link_stage) struct BindImportsOutput {
  pub symbols: SymbolRefDb,
  pub dependencies: ModuleDependenciesDraft,
  pub shimmed_missing_exports: ShimmedMissingExports,
  pub included_commonjs_export_symbols: IncludedCommonJsExportSymbols,
  pub normal_export_chains: NormalExportChains,
  pub external_namespace_merges: ExternalImportNamespaceMerges,
}

#[derive(Clone, Debug)]
struct ImportTracker {
  importer: ModuleIdx,
  importee: ModuleIdx,
  imported: Specifier,
  imported_as: SymbolRef,
}

struct MatchingContext {
  tracker_stack: Vec<ImportTracker>,
}

#[derive(Debug)]
struct MatchImportKindNormal {
  symbol: SymbolRef,
  reexports: Vec<SymbolRef>,
}

#[derive(Debug)]
enum MatchImportKind {
  Normal(MatchImportKindNormal),
  Namespace { namespace_ref: SymbolRef },
  NormalAndNamespace { namespace_ref: SymbolRef, alias: CompactStr },
  Cycle,
  Ambiguous { symbol_ref: SymbolRef, potentially_ambiguous_symbol_refs: Box<[SymbolRef]> },
  NoMatch,
}

impl MatchImportKind {
  fn is_equivalent_to(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Normal(left), Self::Normal(right)) => left.symbol == right.symbol,
      (Self::Namespace { namespace_ref: left }, Self::Namespace { namespace_ref: right }) => {
        left == right
      }
      (
        Self::NormalAndNamespace { namespace_ref: left_ref, alias: left_alias },
        Self::NormalAndNamespace { namespace_ref: right_ref, alias: right_alias },
      ) => left_ref == right_ref && left_alias == right_alias,
      (Self::Cycle, Self::Cycle) | (Self::NoMatch, Self::NoMatch) => true,
      (
        Self::Ambiguous { symbol_ref: left_ref, potentially_ambiguous_symbol_refs: left_ambiguous },
        Self::Ambiguous {
          symbol_ref: right_ref,
          potentially_ambiguous_symbol_refs: right_ambiguous,
        },
      ) => left_ref == right_ref && left_ambiguous == right_ambiguous,
      _ => false,
    }
  }
}

#[derive(Debug)]
enum ImportStatus {
  NoMatch,
  Found { symbol: SymbolRef, potentially_ambiguous_export_star_refs: Vec<SymbolRef> },
  CommonJs,
  DynamicFallback { namespace_ref: SymbolRef },
  DynamicFallbackWithCommonJsReference { namespace_ref: SymbolRef, commonjs_symbol: SymbolRef },
  External(SymbolRef),
}

enum ImportMatchStep {
  Continue,
  Break(MatchImportKind),
}

struct ImportMatcher<'a> {
  modules: &'a IndexModules,
  resolved_exports: &'a ResolvedExportsDraft,
  module_formats: &'a ModuleFormats,
  dynamic_exports: &'a DynamicExports,
  output_format: OutputFormat,
  shim_missing_exports: bool,
}

struct RecursiveMatchEffects<'effects, 'ctx> {
  cx: &'effects mut PassCtx<'ctx>,
  symbols: &'effects mut SymbolRefDb,
  shimmed_missing_exports: &'effects mut ShimSlots,
  included_commonjs_export_symbols: &'effects mut IncludedCommonJsSlots,
}

struct ImportChainState<'state> {
  tracker: &'state mut ImportTracker,
  reexports: &'state mut Vec<SymbolRef>,
  ambiguous_results: &'state mut Vec<MatchImportKind>,
}

struct SerialBindingCommit<'commit, 'ctx> {
  cx: &'commit mut PassCtx<'ctx>,
  symbols: &'commit mut SymbolRefDb,
  dependencies: &'commit mut DependencySlots,
  shimmed_missing_exports: &'commit mut ShimSlots,
  included_commonjs_export_symbols: &'commit mut IncludedCommonJsSlots,
  normal_export_chains: &'commit mut FxHashMap<SymbolRef, Vec<SymbolRef>>,
  external_binding_groups: &'commit mut ExternalBindingGroups,
  external_namespace_merges: &'commit mut FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>>,
}

struct NamedImportSite<'a> {
  importer_idx: ModuleIdx,
  importer: &'a NormalModule,
  imported_as: SymbolRef,
  named_import: &'a NamedImport,
  record: &'a ResolvedImportRecord,
  importee_idx: ModuleIdx,
}

impl ImportMatcher<'_> {
  fn exports_for_normal_module(
    &self,
    module_idx: ModuleIdx,
  ) -> Option<&FxHashMap<CompactStr, ResolvedExport>> {
    self.resolved_exports.get(module_idx)
  }

  fn advance_import_tracker(&self, tracker: &ImportTracker) -> ImportStatus {
    let importer =
      self.modules[tracker.importer].as_normal().expect("only normal modules can import symbols");
    let named_import = &importer.named_imports[&tracker.imported_as];

    let Some(importee_id) = importer.import_records[named_import.record_idx].resolved_module else {
      return ImportStatus::NoMatch;
    };

    let importee = match &self.modules[importee_id] {
      Module::Normal(importee) => importee.as_ref(),
      Module::External(external) => return ImportStatus::External(external.namespace_ref),
    };

    let has_known_module_format = std::matches!(
      self.module_formats.get(importee_id),
      Some(ExportsKind::Esm | ExportsKind::CommonJs)
    );
    std::debug_assert!(
      has_known_module_format
        || importee.meta.has_lazy_export()
        || importee.module_type == ModuleType::Empty
    );

    if self.module_formats.get(importee_id) == Some(ExportsKind::CommonJs) {
      return ImportStatus::CommonJs;
    }

    match &named_import.imported {
      Specifier::Star => ImportStatus::Found {
        symbol: importee.namespace_object_ref,
        potentially_ambiguous_export_star_refs: Vec::new(),
      },
      Specifier::Literal(literal_imported) => {
        let exports = self
          .exports_for_normal_module(importee_id)
          .expect("every normal module must have a resolved-export slot");
        match exports.get(literal_imported) {
          Some(export) if export.came_from_commonjs => {
            ImportStatus::DynamicFallbackWithCommonJsReference {
              namespace_ref: importee.namespace_object_ref,
              commonjs_symbol: export.symbol_ref,
            }
          }
          Some(export) => ImportStatus::Found {
            symbol: export.symbol_ref,
            potentially_ambiguous_export_star_refs: export
              .potentially_ambiguous_symbol_refs
              .as_deref()
              .map_or_else(Vec::new, Clone::clone),
          },
          None if self.dynamic_exports.contains(importee_id) => {
            ImportStatus::DynamicFallback { namespace_ref: importee.namespace_object_ref }
          }
          None => ImportStatus::NoMatch,
        }
      }
    }
  }

  fn match_import_with_export(
    &self,
    effects: &mut RecursiveMatchEffects<'_, '_>,
    ctx: &mut MatchingContext,
    tracker: ImportTracker,
  ) -> MatchImportKind {
    let tracking_span = tracing::trace_span!(
      "TRACKING_MATCH_IMPORT",
      importer = self.modules[tracker.importer].stable_id().as_str(),
      importee = self.modules[tracker.importee].stable_id().as_str(),
      imported_specifier = tracker.imported.to_string()
    );
    let _enter = tracking_span.enter();

    let (ret, ambiguous_results, tracker) = self.follow_import_chain(effects, ctx, tracker);
    tracing::trace!("ambiguous_results {:#?}", ambiguous_results);
    tracing::trace!("ret {:#?}", ret);
    let ret = self.classify_ambiguity(ret, &ambiguous_results);
    self.maybe_shim_missing_export(effects, &tracker, ret)
  }

  fn follow_import_chain(
    &self,
    effects: &mut RecursiveMatchEffects<'_, '_>,
    ctx: &mut MatchingContext,
    mut tracker: ImportTracker,
  ) -> (MatchImportKind, Vec<MatchImportKind>, ImportTracker) {
    let mut ambiguous_results = Vec::new();
    let mut reexports = Vec::new();
    loop {
      for prev_tracker in ctx.tracker_stack.iter().rev() {
        if prev_tracker.importer == tracker.importer
          && prev_tracker.imported_as == tracker.imported_as
        {
          let importer_id = self.modules[tracker.importer].id().to_string();
          effects
            .cx
            .push(BuildDiagnostic::circular_reexport(importer_id, tracker.imported.to_string()));
          return (MatchImportKind::Cycle, ambiguous_results, tracker);
        }
      }
      ctx.tracker_stack.push(tracker.clone());
      let import_status = self.advance_import_tracker(&tracker);
      tracing::trace!("Got import_status {:?}", import_status);
      let mut state = ImportChainState {
        tracker: &mut tracker,
        reexports: &mut reexports,
        ambiguous_results: &mut ambiguous_results,
      };
      match self.apply_import_status(effects, ctx, &mut state, import_status) {
        ImportMatchStep::Continue => {}
        ImportMatchStep::Break(result) => return (result, ambiguous_results, tracker),
      }
    }
  }

  fn apply_import_status(
    &self,
    effects: &mut RecursiveMatchEffects<'_, '_>,
    ctx: &MatchingContext,
    state: &mut ImportChainState<'_>,
    import_status: ImportStatus,
  ) -> ImportMatchStep {
    let importer = self.modules[state.tracker.importer]
      .as_normal()
      .expect("only normal modules can import symbols");
    let named_import = &importer.named_imports[&state.tracker.imported_as];
    let importer_record = &importer.import_records[named_import.record_idx];
    let result = match import_status {
      ImportStatus::CommonJs => self.namespace_match(importer_record.namespace_ref, state.tracker),
      ImportStatus::DynamicFallback { namespace_ref } => {
        self.namespace_match(namespace_ref, state.tracker)
      }
      ImportStatus::DynamicFallbackWithCommonJsReference { namespace_ref, commonjs_symbol } => {
        effects.included_commonjs_export_symbols[state.tracker.importee]
          .as_mut()
          .expect("every normal module must have an included-CommonJS-export slot")
          .insert(commonjs_symbol);
        self.namespace_match(namespace_ref, state.tracker)
      }
      ImportStatus::NoMatch => MatchImportKind::NoMatch,
      ImportStatus::Found { symbol, potentially_ambiguous_export_star_refs } => {
        return self.match_found_export(
          effects,
          ctx,
          state,
          symbol,
          &potentially_ambiguous_export_star_refs,
        );
      }
      ImportStatus::External(symbol_ref) => {
        if self.output_format.keep_esm_import_export_syntax() {
          MatchImportKind::Normal(MatchImportKindNormal {
            symbol: state.tracker.imported_as,
            reexports: Vec::new(),
          })
        } else {
          self.namespace_match(symbol_ref, state.tracker)
        }
      }
    };
    ImportMatchStep::Break(result)
  }

  fn namespace_match(&self, namespace_ref: SymbolRef, tracker: &ImportTracker) -> MatchImportKind {
    match &tracker.imported {
      Specifier::Star => MatchImportKind::Namespace { namespace_ref },
      Specifier::Literal(alias) => {
        MatchImportKind::NormalAndNamespace { namespace_ref, alias: alias.clone() }
      }
    }
  }

  fn match_found_export(
    &self,
    effects: &mut RecursiveMatchEffects<'_, '_>,
    ctx: &MatchingContext,
    state: &mut ImportChainState<'_>,
    symbol: SymbolRef,
    potentially_ambiguous_export_star_refs: &[SymbolRef],
  ) -> ImportMatchStep {
    for ambiguous_ref in potentially_ambiguous_export_star_refs {
      let owner = self.modules[ambiguous_ref.owner]
        .as_normal()
        .expect("ambiguous exports must be owned by normal modules");
      if let Some(named_import) = owner.named_imports.get(ambiguous_ref) {
        let record = &owner.import_records[named_import.record_idx];
        if let Some(importee) = record.resolved_module {
          state.ambiguous_results.push(self.match_import_with_export(
            effects,
            &mut MatchingContext { tracker_stack: ctx.tracker_stack.clone() },
            ImportTracker {
              importer: owner.idx,
              importee,
              imported: named_import.imported.clone(),
              imported_as: named_import.imported_as,
            },
          ));
        }
      } else {
        state.ambiguous_results.push(MatchImportKind::Normal(MatchImportKindNormal {
          symbol: *ambiguous_ref,
          reexports: Vec::new(),
        }));
      }
    }

    let owner = self.modules[symbol.owner]
      .as_normal()
      .expect("resolved exports must be owned by normal modules");
    let Some(named_import) = owner.named_imports.get(&symbol) else {
      return ImportMatchStep::Break(MatchImportKind::Normal(MatchImportKindNormal {
        symbol,
        reexports: std::mem::take(state.reexports),
      }));
    };
    let record = &owner.import_records[named_import.record_idx];
    let Some(importee) = record.resolved_module else {
      return ImportMatchStep::Break(MatchImportKind::Normal(MatchImportKindNormal {
        symbol,
        reexports: std::mem::take(state.reexports),
      }));
    };
    match &self.modules[importee] {
      Module::External(_) => {
        ImportMatchStep::Break(MatchImportKind::Normal(MatchImportKindNormal {
          symbol: named_import.imported_as,
          reexports: Vec::new(),
        }))
      }
      Module::Normal(importee_module) => {
        state.tracker.importee = importee_module.idx;
        state.tracker.importer = owner.idx;
        state.tracker.imported = named_import.imported.clone();
        state.tracker.imported_as = named_import.imported_as;
        state.reexports.push(named_import.imported_as);
        ImportMatchStep::Continue
      }
    }
  }

  fn classify_ambiguity(
    &self,
    ret: MatchImportKind,
    ambiguous_results: &[MatchImportKind],
  ) -> MatchImportKind {
    for ambiguous_result in ambiguous_results {
      if !ambiguous_result.is_equivalent_to(&ret) {
        if let MatchImportKind::Normal(MatchImportKindNormal { symbol, .. }) = ret {
          return MatchImportKind::Ambiguous {
            symbol_ref: symbol,
            potentially_ambiguous_symbol_refs: ambiguous_results
              .iter()
              .filter_map(|kind| match *kind {
                MatchImportKind::Normal(MatchImportKindNormal { symbol, .. }) => Some(symbol),
                MatchImportKind::Namespace { namespace_ref }
                | MatchImportKind::NormalAndNamespace { namespace_ref, .. } => Some(namespace_ref),
                MatchImportKind::Cycle
                | MatchImportKind::Ambiguous { .. }
                | MatchImportKind::NoMatch => None,
              })
              .collect::<Vec<_>>()
              .into_boxed_slice(),
          };
        }
        std::unreachable!("ambiguous alternatives require a normal primary symbol");
      }
    }
    ret
  }

  fn maybe_shim_missing_export(
    &self,
    effects: &mut RecursiveMatchEffects<'_, '_>,
    tracker: &ImportTracker,
    ret: MatchImportKind,
  ) -> MatchImportKind {
    if let Module::Normal(importee) = &self.modules[tracker.importee]
      && (self.shim_missing_exports || std::matches!(importee.module_type, ModuleType::Empty))
      && std::matches!(ret, MatchImportKind::NoMatch)
    {
      match &tracker.imported {
        Specifier::Star => {
          std::unreachable!("namespace imports must always resolve without a missing-export shim");
        }
        Specifier::Literal(imported) => {
          let shimmed = effects.shimmed_missing_exports[tracker.importee]
            .as_mut()
            .expect("every normal module must have a missing-export shim slot");
          let shimmed_symbol_ref = shimmed.entry(imported.clone()).or_insert_with(|| {
            effects.symbols.create_facade_root_symbol_ref(tracker.importee, imported.as_str())
          });
          return MatchImportKind::Normal(MatchImportKindNormal {
            symbol: *shimmed_symbol_ref,
            reexports: Vec::new(),
          });
        }
      }
    }

    ret
  }
}

fn record_external_groups(
  matcher: &ImportMatcher<'_>,
  site: &NamedImportSite<'_>,
  commit: &mut SerialBindingCommit<'_, '_>,
) {
  if !std::matches!(matcher.output_format, OutputFormat::Esm)
    || !std::matches!(matcher.modules[site.importee_idx], Module::External(_))
  {
    return;
  }

  match &site.named_import.imported {
    Specifier::Star => {
      commit
        .external_namespace_merges
        .entry(site.importee_idx)
        .or_default()
        .insert(site.imported_as);
    }
    Specifier::Literal(name) => {
      let exports = matcher
        .exports_for_normal_module(site.importer_idx)
        .expect("every normal module must have a resolved-export slot");
      if exports.values().all(|resolved_export| resolved_export.symbol_ref != site.imported_as) {
        commit
          .external_binding_groups
          .entry(site.importee_idx)
          .or_default()
          .entry(name.clone())
          .or_default()
          .insert(site.imported_as);
      }
    }
  }
}

fn ambiguous_exporter(
  modules: &IndexModules,
  symbol_ref: SymbolRef,
  imported: &Specifier,
) -> Option<AmbiguousExternalNamespaceModule> {
  match imported {
    Specifier::Star => None,
    Specifier::Literal(name) => modules[symbol_ref.owner].as_normal().map(|module| {
      let named_export = &module.named_exports[name];
      AmbiguousExternalNamespaceModule {
        source: module.source.clone(),
        module_id: module.id.to_string(),
        stable_id: module.stable_id.to_string(),
        span_of_identifier: named_export.span,
      }
    }),
  }
}

fn emit_ambiguous_diagnostic(
  modules: &IndexModules,
  site: &NamedImportSite<'_>,
  symbol_ref: SymbolRef,
  potentially_ambiguous_symbol_refs: &[SymbolRef],
  cx: &mut PassCtx<'_>,
) {
  let mut exporters = Vec::with_capacity(potentially_ambiguous_symbol_refs.len() + 1);
  if let Some(exporter) = ambiguous_exporter(modules, symbol_ref, &site.named_import.imported) {
    exporters.push(exporter);
  }
  exporters.extend(potentially_ambiguous_symbol_refs.iter().filter_map(|&symbol_ref| {
    ambiguous_exporter(modules, symbol_ref, &site.named_import.imported)
  }));
  cx.push(BuildDiagnostic::ambiguous_external_namespace(
    site.named_import.imported.to_string(),
    modules[site.importee_idx].stable_id().to_string(),
    AmbiguousExternalNamespaceModule {
      source: site.importer.source.clone(),
      module_id: site.importer.id.to_string(),
      stable_id: site.importer.stable_id.to_string(),
      span_of_identifier: site.named_import.span_imported,
    },
    exporters,
  ));
}

fn emit_missing_export_diagnostic(
  modules: &IndexModules,
  site: &NamedImportSite<'_>,
  cx: &mut PassCtx<'_>,
) {
  let importee = &modules[site.importee_idx];
  let is_ts_like_importing_ts_like =
    std::matches!(
      importee.as_normal().map(|module| &module.module_type),
      Some(ModuleType::Ts | ModuleType::Tsx)
    ) && std::matches!(site.importer.module_type, ModuleType::Ts | ModuleType::Tsx);
  cx.push(BuildDiagnostic::missing_export(
    site.importer.id.to_string(),
    site.importer.stable_id.to_string(),
    importee.id().to_string(),
    importee.stable_id().to_string(),
    site.importer.source.clone(),
    site.named_import.imported.to_string(),
    site.named_import.span_imported,
    is_ts_like_importing_ts_like.then(|| {
      std::format!(
        "If you meant to import a type rather than a value, make sure to add the `type` modifier (e.g. `import {{ type Foo }} from '{}'`).",
        site.record.module_request
      )
    }),
  ));
}

fn commit_match_result(
  matcher: &ImportMatcher<'_>,
  module_side_effects: &ModuleSideEffects,
  site: &NamedImportSite<'_>,
  result: MatchImportKind,
  commit: &mut SerialBindingCommit<'_, '_>,
) {
  match result {
    MatchImportKind::Cycle => {}
    MatchImportKind::Ambiguous { symbol_ref, potentially_ambiguous_symbol_refs } => {
      emit_ambiguous_diagnostic(
        matcher.modules,
        site,
        symbol_ref,
        &potentially_ambiguous_symbol_refs,
        commit.cx,
      );
    }
    MatchImportKind::Normal(MatchImportKindNormal { symbol, reexports }) => {
      for reexport in &reexports {
        if module_side_effects.get(reexport.owner).has_side_effects() {
          commit.dependencies[site.importer_idx].insert(reexport.owner);
        }
      }
      commit.normal_export_chains.insert(site.imported_as, reexports);
      commit.symbols.link(site.imported_as, symbol);
    }
    MatchImportKind::Namespace { namespace_ref } => {
      commit.symbols.link(site.imported_as, namespace_ref);
    }
    MatchImportKind::NormalAndNamespace { namespace_ref, alias } => {
      commit.symbols.get_mut(site.imported_as).namespace_alias =
        Some(NamespaceAlias { property_name: alias, namespace_ref });
      if site.imported_as.flags(commit.symbols).is_some_and(|flags| {
        flags.intersects(
          SymbolRefFlags::MustStartWithCapitalLetterForJSX
            | SymbolRefFlags::UsedAsJSXMemberExprRoot,
        )
      }) {
        *namespace_ref.flags_mut(commit.symbols) |=
          SymbolRefFlags::MustStartWithCapitalLetterForJSX;
      }
    }
    MatchImportKind::NoMatch => {
      emit_missing_export_diagnostic(matcher.modules, site, commit.cx);
    }
  }
}

fn bind_normal_module(
  matcher: &ImportMatcher<'_>,
  module_side_effects: &ModuleSideEffects,
  module_idx: ModuleIdx,
  module: &NormalModule,
  commit: &mut SerialBindingCommit<'_, '_>,
) {
  let mut matching_ctx = MatchingContext { tracker_stack: Vec::new() };
  for (imported_as_ref, named_import) in &module.named_imports {
    let match_import_span = tracing::trace_span!(
      "MATCH_IMPORT",
      module_id = module.stable_id.as_str(),
      imported_specifier = named_import.imported.to_string()
    );
    let _enter = match_import_span.enter();
    let record = &module.import_records[named_import.record_idx];
    let Some(importee_idx) = record.resolved_module else { continue };
    let site = NamedImportSite {
      importer_idx: module_idx,
      importer: module,
      imported_as: *imported_as_ref,
      named_import,
      record,
      importee_idx,
    };
    record_external_groups(matcher, &site, commit);
    matching_ctx.tracker_stack.clear();
    let result = {
      let mut effects = RecursiveMatchEffects {
        cx: commit.cx,
        symbols: commit.symbols,
        shimmed_missing_exports: commit.shimmed_missing_exports,
        included_commonjs_export_symbols: commit.included_commonjs_export_symbols,
      };
      matcher.match_import_with_export(
        &mut effects,
        &mut matching_ctx,
        ImportTracker {
          importer: module_idx,
          importee: importee_idx,
          imported: named_import.imported.clone(),
          imported_as: *imported_as_ref,
        },
      )
    };
    tracing::trace!("Got match result {:?}", result);
    commit_match_result(matcher, module_side_effects, &site, result, commit);
  }
}

fn bind_all_imports(
  matcher: &ImportMatcher<'_>,
  module_side_effects: &ModuleSideEffects,
  commit: &mut SerialBindingCommit<'_, '_>,
) {
  for (module_idx, module) in matcher.modules.iter_enumerated() {
    let Module::Normal(module) = module else { continue };
    std::assert_eq!(module_idx, module.idx, "normal module index must match its physical slot");
    bind_normal_module(matcher, module_side_effects, module_idx, module, commit);
  }
}

fn commit_external_facades(
  external_binding_groups: &ExternalBindingGroups,
  execution_orders: &ModuleExecutionOrders,
  symbols: &mut SymbolRefDb,
) {
  for (module_idx, map) in external_binding_groups {
    let mut entries = map.iter().collect::<Vec<_>>();
    if entries.len() > 1 {
      entries.sort_unstable_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
    }
    for (key, symbol_set) in entries {
      let name = if key.as_str() == "default" {
        let key = symbol_set
          .iter()
          .min_by_key(|&&symbol_ref| {
            (execution_orders.get(symbol_ref.owner), symbol_ref.name(symbols))
          })
          .map_or_else(|| key.clone(), |symbol_ref| symbol_ref.name(symbols).into());
        Cow::Owned(key)
      } else if is_validate_identifier_name(key.as_str()) {
        Cow::Borrowed(key)
      } else {
        Cow::Owned(legitimize_identifier_name(key).as_ref().into())
      };
      let target_symbol = symbols.create_facade_root_symbol_ref(*module_idx, &name);
      for symbol_ref in symbol_set {
        symbols.link(*symbol_ref, target_symbol);
      }
    }
  }
}

impl Pass for BindImportsPass {
  type InputRead<'a> = BindImportsInput<'a>;
  type InputOwned = BindImportsOwned;
  type OutputRead = ();
  type OutputOwned = BindImportsOutput;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let BindImportsInput {
      module_table,
      resolved_exports,
      module_formats,
      dynamic_exports,
      module_side_effects,
      execution_orders,
      output_format,
      shim_missing_exports,
    } = input;
    let BindImportsOwned { mut symbols, dependencies } = owned;
    let mut dependencies = dependencies.into_inner();

    let module_count = module_table.modules.len();
    std::assert_eq!(dependencies.len(), module_count, "dependency layout must match the modules");
    std::assert_eq!(
      resolved_exports.module_count(),
      module_count,
      "resolved-export layout must match the modules"
    );
    std::assert_eq!(
      module_formats.module_count(),
      module_count,
      "module-format layout must match the modules"
    );
    std::assert_eq!(
      module_side_effects.module_count(),
      module_count,
      "module-side-effect layout must match the modules"
    );

    let mut shimmed_missing_exports = module_table
      .modules
      .iter()
      .map(|module| module.as_normal().map(|_| FxHashMap::default()))
      .collect::<IndexVec<ModuleIdx, _>>();
    let mut included_commonjs_export_symbols = module_table
      .modules
      .iter()
      .map(|module| module.as_normal().map(|_| FxHashSet::default()))
      .collect::<IndexVec<ModuleIdx, _>>();
    let mut normal_export_chains = FxHashMap::default();
    let mut external_binding_groups = ExternalBindingGroups::default();
    let mut external_namespace_merges: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>> =
      FxHashMap::default();

    let matcher = ImportMatcher {
      modules: &module_table.modules,
      resolved_exports,
      module_formats,
      dynamic_exports,
      output_format,
      shim_missing_exports,
    };
    {
      let mut commit = SerialBindingCommit {
        cx,
        symbols: &mut symbols,
        dependencies: &mut dependencies,
        shimmed_missing_exports: &mut shimmed_missing_exports,
        included_commonjs_export_symbols: &mut included_commonjs_export_symbols,
        normal_export_chains: &mut normal_export_chains,
        external_binding_groups: &mut external_binding_groups,
        external_namespace_merges: &mut external_namespace_merges,
      };
      bind_all_imports(&matcher, module_side_effects, &mut commit);
    }
    commit_external_facades(&external_binding_groups, execution_orders, &mut symbols);

    Ok(token.finish(
      (),
      BindImportsOutput {
        symbols,
        dependencies: ModuleDependenciesDraft::from_inner(dependencies),
        shimmed_missing_exports: ShimmedMissingExports { slots: shimmed_missing_exports },
        included_commonjs_export_symbols: IncludedCommonJsExportSymbols {
          slots: included_commonjs_export_symbols,
        },
        normal_export_chains: NormalExportChains { chains: normal_export_chains },
        external_namespace_merges: ExternalImportNamespaceMerges {
          merges: external_namespace_merges,
        },
      },
    ))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::Scoping, span::Span};
  use rolldown_common::{
    EntryPointKind, ExportsKind, ImportKind, ImportRecordIdx, LocalExport, Module, ModuleTable,
    NamedImport, OutputFormat, Specifier, SymbolRef, SymbolRefDb, SymbolRefDbForModule, WrapKind,
    side_effects::DeterminedSideEffects,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CanonicalizeEntriesPass, CollectInitialDependenciesPass, CollectResolvedExportsPass,
    ComputeModuleExecutionOrderInput, ComputeModuleExecutionOrderPass, DetermineModuleFormatsInput,
    DetermineModuleFormatsPass, DetermineModuleSideEffectsInput, DetermineModuleSideEffectsPass,
    ResolvedExportsDraft,
    collect_resolved_exports::test_support::set_conflicts,
    compute_dynamic_exports::test_support::dynamic_exports,
    create_wrapper_declarations::test_support::module_wrappers,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };
  use super::{BindImportsInput, BindImportsOutput, BindImportsOwned, BindImportsPass};

  fn symbols_for(modules: &ModuleTable) -> SymbolRefDb {
    let mut symbols = SymbolRefDb::new();
    for (module_idx, module) in modules.modules.iter_enumerated() {
      let scoping = Scoping::default();
      let root_scope_id = scoping.root_scope_id();
      symbols
        .store_local_db(module_idx, SymbolRefDbForModule::new(scoping, module_idx, root_scope_id));
      let namespace_ref = symbols.create_facade_root_symbol_ref(module_idx, "namespace");
      let expected_namespace_ref = match module {
        Module::Normal(module) => module.namespace_object_ref,
        Module::External(module) => module.namespace_ref,
      };
      assert_eq!(namespace_ref, expected_namespace_ref);
    }
    symbols
  }

  fn add_named_import(
    modules: &mut ModuleTable,
    importer: usize,
    record: usize,
    imported_as: SymbolRef,
    imported: Specifier,
  ) {
    let span_start = 32 + u32::try_from(record).expect("fixture record index fits in a span");
    let record_idx = ImportRecordIdx::from_usize(record);
    modules[module_idx(importer)].as_normal_mut().expect("normal importer").named_imports.insert(
      imported_as,
      NamedImport {
        imported,
        span_imported: Span::new(span_start, span_start + 1),
        imported_as,
        record_idx,
      },
    );
  }

  fn set_record_namespace(
    modules: &mut ModuleTable,
    importer: usize,
    record: usize,
    namespace_ref: SymbolRef,
  ) {
    modules[module_idx(importer)].as_normal_mut().expect("normal importer").import_records
      [ImportRecordIdx::from_usize(record)]
    .namespace_ref = namespace_ref;
  }

  fn insert_export(
    modules: &mut ModuleTable,
    owner: usize,
    name: &str,
    referenced: SymbolRef,
    came_from_commonjs: bool,
  ) {
    let span_start =
      96 + u32::try_from(referenced.symbol.index()).expect("fixture symbol fits in a span");
    modules[module_idx(owner)].as_normal_mut().expect("normal export owner").named_exports.insert(
      name.into(),
      LocalExport { span: Span::new(span_start, span_start + 1), referenced, came_from_commonjs },
    );
  }

  fn set_exports_kind(modules: &mut ModuleTable, module: usize, kind: ExportsKind) {
    modules[module_idx(module)].as_normal_mut().expect("normal module").exports_kind = kind;
  }

  fn set_side_effectful(modules: &mut ModuleTable, module: usize) {
    modules[module_idx(module)].as_normal_mut().expect("normal module").side_effects =
      DeterminedSideEffects::Analyzed(true);
  }

  fn bind(
    modules: &ModuleTable,
    symbols: SymbolRefDb,
    output_format: OutputFormat,
    shim_missing_exports: bool,
    dynamic: &[usize],
    prepare_resolved_exports: impl FnOnce(&mut ResolvedExportsDraft),
  ) -> (BindImportsOutput, Vec<String>) {
    let entries = modules
      .modules
      .iter_enumerated()
      .filter_map(|(module_idx, module)| {
        module.as_normal().map(|_| entry_point(module_idx.index(), EntryPointKind::UserDefined))
      })
      .collect::<Vec<_>>();
    let runtime = modules
      .modules
      .iter_enumerated()
      .find_map(|(module_idx, module)| {
        module.as_normal().filter(|module| module.import_records.is_empty()).map(|_| module_idx)
      })
      .unwrap_or_else(|| entries.first().expect("at least one normal fixture module").idx);
    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) =
      run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, modules, entries);
    let (_, (formats, _wrapper_seeds)) = run_infallible_pass(
      DetermineModuleFormatsPass,
      &mut pipeline,
      DetermineModuleFormatsInput {
        module_table: modules,
        entry_plan: &entry_plan,
        output_format,
        code_splitting_disabled: false,
      },
      (),
    );
    let dynamic_exports =
      dynamic_exports(modules.modules.len(), dynamic.iter().copied().map(module_idx));
    let wrap_kinds = vec![WrapKind::None; modules.modules.len()];
    let wrappers = module_wrappers(&wrap_kinds);
    let (module_side_effects, ()) = run_infallible_pass(
      DetermineModuleSideEffectsPass,
      &mut pipeline,
      DetermineModuleSideEffectsInput {
        module_table: modules,
        dynamic_exports: &dynamic_exports,
        module_wrappers: &wrappers,
      },
      (),
    );
    let (_, mut resolved_exports) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, modules, ());
    prepare_resolved_exports(&mut resolved_exports);
    let (_, dependencies) =
      run_infallible_pass(CollectInitialDependenciesPass, &mut pipeline, modules, ());
    let (execution_orders, _sorted_modules) = run_infallible_pass(
      ComputeModuleExecutionOrderPass,
      &mut pipeline,
      ComputeModuleExecutionOrderInput {
        module_table: modules,
        entry_plan: &entry_plan,
        runtime,
        code_splitting_disabled: false,
        check_circular_dependencies: false,
      },
      (),
    );
    let module_formats = formats.finalize();
    let (_, output) = run_infallible_pass(
      BindImportsPass,
      &mut pipeline,
      BindImportsInput {
        module_table: modules,
        resolved_exports: &resolved_exports,
        module_formats: &module_formats,
        dynamic_exports: &dynamic_exports,
        module_side_effects: &module_side_effects,
        execution_orders: &execution_orders,
        output_format,
        shim_missing_exports,
      },
      BindImportsOwned { symbols, dependencies },
    );
    let diagnostics =
      pipeline.into_diagnostics().into_iter().map(|diagnostic| diagnostic.to_string()).collect();
    (output, diagnostics)
  }

  #[test]
  fn links_direct_import_and_preserves_owned_dependency_and_physical_slot_shapes() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(2), Span::new(1, 2))]),
      external_module(1, "external"),
      normal_module(2, false, Vec::new()),
    ]);
    let mut symbols = symbols_for(&modules);
    let imported_as = symbols.create_facade_root_symbol_ref(module_idx(0), "local_value");
    let exported = symbols.create_facade_root_symbol_ref(module_idx(2), "exported_value");
    add_named_import(&mut modules, 0, 0, imported_as, Specifier::Literal("value".into()));
    insert_export(&mut modules, 2, "value", exported, false);

    let (output, diagnostics) = bind(&modules, symbols, OutputFormat::Esm, false, &[], |_| {});
    assert!(diagnostics.is_empty());
    let BindImportsOutput {
      symbols,
      dependencies,
      shimmed_missing_exports,
      included_commonjs_export_symbols,
      normal_export_chains,
      external_namespace_merges,
    } = output;

    assert_eq!(symbols.canonical_ref_for(imported_as), exported);
    let dependencies = dependencies.into_inner();
    assert_eq!(dependencies.len(), 3);
    assert_eq!(dependencies[module_idx(0)].iter().copied().collect::<Vec<_>>(), [module_idx(2)]);
    assert!(dependencies[module_idx(1)].is_empty());
    assert!(dependencies[module_idx(2)].is_empty());
    let shim_slots = shimmed_missing_exports.into_slots();
    let included_slots = included_commonjs_export_symbols.into_slots();
    assert_eq!(shim_slots.iter().map(Option::is_some).collect::<Vec<_>>(), [true, false, true]);
    assert_eq!(included_slots.iter().map(Option::is_some).collect::<Vec<_>>(), [true, false, true]);
    assert!(shim_slots.iter().filter_map(Option::as_ref).all(rustc_hash::FxHashMap::is_empty));
    assert!(included_slots.iter().filter_map(Option::as_ref).all(rustc_hash::FxHashSet::is_empty));
    assert_eq!(normal_export_chains.into_inner()[&imported_as], Vec::<SymbolRef>::new());
    assert!(external_namespace_merges.into_inner().is_empty());
  }

  #[test]
  fn links_reexport_chain_and_adds_side_effect_dependencies_in_chain_order() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(1, 2))]),
      normal_module(1, false, vec![(ImportKind::Import, Some(2), Span::new(3, 4))]),
      normal_module(2, false, vec![(ImportKind::Import, Some(3), Span::new(5, 6))]),
      normal_module(3, false, Vec::new()),
    ]);
    let mut symbols = symbols_for(&modules);
    let imported_as = symbols.create_facade_root_symbol_ref(module_idx(0), "from_first");
    let first_reexport = symbols.create_facade_root_symbol_ref(module_idx(1), "from_second");
    let second_reexport = symbols.create_facade_root_symbol_ref(module_idx(2), "from_third");
    let exported = symbols.create_facade_root_symbol_ref(module_idx(3), "value");
    add_named_import(&mut modules, 0, 0, imported_as, Specifier::Literal("value".into()));
    add_named_import(&mut modules, 1, 0, first_reexport, Specifier::Literal("value".into()));
    add_named_import(&mut modules, 2, 0, second_reexport, Specifier::Literal("value".into()));
    insert_export(&mut modules, 1, "value", first_reexport, false);
    insert_export(&mut modules, 2, "value", second_reexport, false);
    insert_export(&mut modules, 3, "value", exported, false);
    set_side_effectful(&mut modules, 2);

    let (output, diagnostics) = bind(&modules, symbols, OutputFormat::Esm, false, &[], |_| {});
    assert!(diagnostics.is_empty());
    let BindImportsOutput { symbols, dependencies, normal_export_chains, .. } = output;
    assert_eq!(symbols.canonical_ref_for(imported_as), exported);
    assert_eq!(normal_export_chains.into_inner()[&imported_as], [first_reexport, second_reexport]);
    assert_eq!(
      dependencies.into_inner()[module_idx(0)].iter().copied().collect::<Vec<_>>(),
      [module_idx(1), module_idx(2)]
    );
  }

  #[test]
  fn preserves_dynamic_and_commonjs_fallbacks_and_tracks_included_commonjs_exports() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(1, 2)),
          (ImportKind::Import, Some(2), Span::new(3, 4)),
          (ImportKind::Import, Some(3), Span::new(5, 6)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
    ]);
    set_exports_kind(&mut modules, 2, ExportsKind::CommonJs);
    let mut symbols = symbols_for(&modules);
    let dynamic_import = symbols.create_facade_root_symbol_ref(module_idx(0), "dynamic_import");
    let commonjs_import = symbols.create_facade_root_symbol_ref(module_idx(0), "commonjs_import");
    let commonjs_record_namespace =
      symbols.create_facade_root_symbol_ref(module_idx(0), "commonjs_record_namespace");
    let commonjs_export_import =
      symbols.create_facade_root_symbol_ref(module_idx(0), "commonjs_export_import");
    let commonjs_export = symbols.create_facade_root_symbol_ref(module_idx(3), "commonjs_export");
    add_named_import(&mut modules, 0, 0, dynamic_import, Specifier::Literal("dynamic_name".into()));
    add_named_import(&mut modules, 0, 1, commonjs_import, Specifier::Literal("cjs_name".into()));
    add_named_import(
      &mut modules,
      0,
      2,
      commonjs_export_import,
      Specifier::Literal("cjs_prop".into()),
    );
    set_record_namespace(&mut modules, 0, 1, commonjs_record_namespace);
    insert_export(&mut modules, 3, "cjs_prop", commonjs_export, true);

    let module_one_namespace =
      modules[module_idx(1)].as_normal().expect("normal dynamic module").namespace_object_ref;
    let module_three_namespace = modules[module_idx(3)]
      .as_normal()
      .expect("normal CommonJS-export owner")
      .namespace_object_ref;
    let (output, diagnostics) = bind(&modules, symbols, OutputFormat::Esm, false, &[1], |_| {});
    assert!(diagnostics.is_empty());
    let BindImportsOutput { symbols, included_commonjs_export_symbols, .. } = output;

    let dynamic_alias =
      symbols.get(dynamic_import).namespace_alias.as_ref().expect("dynamic namespace fallback");
    assert_eq!(dynamic_alias.property_name, "dynamic_name");
    assert_eq!(dynamic_alias.namespace_ref, module_one_namespace);
    let commonjs_alias =
      symbols.get(commonjs_import).namespace_alias.as_ref().expect("CommonJS namespace fallback");
    assert_eq!(commonjs_alias.property_name, "cjs_name");
    assert_eq!(commonjs_alias.namespace_ref, commonjs_record_namespace);
    let commonjs_export_alias = symbols
      .get(commonjs_export_import)
      .namespace_alias
      .as_ref()
      .expect("CommonJS-origin export namespace fallback");
    assert_eq!(commonjs_export_alias.property_name, "cjs_prop");
    assert_eq!(commonjs_export_alias.namespace_ref, module_three_namespace);
    let included = included_commonjs_export_symbols.into_slots();
    assert!(included[module_idx(1)].as_ref().expect("normal slot").is_empty());
    assert!(included[module_idx(2)].as_ref().expect("normal slot").is_empty());
    assert_eq!(
      included[module_idx(3)].as_ref().expect("normal slot").iter().copied().collect::<Vec<_>>(),
      [commonjs_export]
    );
  }

  #[test]
  fn reuses_one_shimmed_missing_export_across_importers_without_a_diagnostic() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(1, 2))]),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, vec![(ImportKind::Import, Some(1), Span::new(3, 4))]),
    ]);
    let mut symbols = symbols_for(&modules);
    let first_import = symbols.create_facade_root_symbol_ref(module_idx(0), "first_missing");
    let second_import = symbols.create_facade_root_symbol_ref(module_idx(2), "second_missing");
    add_named_import(&mut modules, 0, 0, first_import, Specifier::Literal("missing".into()));
    add_named_import(&mut modules, 2, 0, second_import, Specifier::Literal("missing".into()));

    let (output, diagnostics) = bind(&modules, symbols, OutputFormat::Esm, true, &[], |_| {});
    assert!(diagnostics.is_empty());
    let BindImportsOutput { symbols, shimmed_missing_exports, .. } = output;
    let slots = shimmed_missing_exports.into_slots();
    let shims = slots[module_idx(1)].as_ref().expect("normal shim slot");
    assert_eq!(shims.len(), 1);
    let shim = shims["missing"];
    assert_eq!(shim.name(&symbols), "missing");
    assert_eq!(symbols.canonical_ref_for(first_import), shim);
    assert_eq!(symbols.canonical_ref_for(second_import), shim);
  }

  #[test]
  fn emits_missing_then_ambiguous_diagnostics_with_legacy_content() {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(1, 2)),
          (ImportKind::Import, Some(1), Span::new(3, 4)),
        ],
      ),
      normal_module(1, false, Vec::new()),
      normal_module(2, false, Vec::new()),
    ]);
    let mut symbols = symbols_for(&modules);
    let missing_import = symbols.create_facade_root_symbol_ref(module_idx(0), "missing_import");
    let ambiguous_import = symbols.create_facade_root_symbol_ref(module_idx(0), "ambiguous_import");
    let primary = symbols.create_facade_root_symbol_ref(module_idx(1), "primary");
    let conflict = symbols.create_facade_root_symbol_ref(module_idx(2), "conflict");
    add_named_import(&mut modules, 0, 0, missing_import, Specifier::Literal("missing".into()));
    add_named_import(&mut modules, 0, 1, ambiguous_import, Specifier::Literal("ambiguous".into()));
    insert_export(&mut modules, 1, "ambiguous", primary, false);
    insert_export(&mut modules, 2, "ambiguous", conflict, false);

    let (_output, diagnostics) =
      bind(&modules, symbols, OutputFormat::Esm, false, &[], |resolved_exports| {
        set_conflicts(resolved_exports, module_idx(1), "ambiguous", Some(vec![conflict]), None);
      });
    assert_eq!(
      diagnostics,
      [
        "\"missing\" is not exported by \"m1.js\", imported by \"m0.js\".",
        "\"m1.js\" re-exports \"ambiguous\" from one of the modules \"m1.js\" and \"m2.js\" (will be ignored).",
      ]
    );
  }

  struct ExternalSummary {
    default_target: SymbolRef,
    default_name: String,
    named_target: SymbolRef,
    named_name: String,
    invalid_target: SymbolRef,
    invalid_name: String,
    namespace_merges: Vec<SymbolRef>,
    chain_count: usize,
  }

  fn bind_external_groups() -> ExternalSummary {
    let imports = vec![
      (ImportKind::Import, Some(2), Span::new(1, 2)),
      (ImportKind::Import, Some(2), Span::new(3, 4)),
      (ImportKind::Import, Some(2), Span::new(5, 6)),
      (ImportKind::Import, Some(2), Span::new(7, 8)),
    ];
    let mut modules = module_table(vec![
      normal_module(0, false, imports.clone()),
      normal_module(1, false, imports),
      external_module(2, "external"),
    ]);
    let mut symbols = symbols_for(&modules);
    let default_zero = symbols.create_facade_root_symbol_ref(module_idx(0), "z_default");
    let named_zero = symbols.create_facade_root_symbol_ref(module_idx(0), "named_zero");
    let invalid_zero = symbols.create_facade_root_symbol_ref(module_idx(0), "invalid_zero");
    let namespace_zero = symbols.create_facade_root_symbol_ref(module_idx(0), "namespace_zero");
    let default_one = symbols.create_facade_root_symbol_ref(module_idx(1), "a_default");
    let named_one = symbols.create_facade_root_symbol_ref(module_idx(1), "named_one");
    let invalid_one = symbols.create_facade_root_symbol_ref(module_idx(1), "invalid_one");
    let namespace_one = symbols.create_facade_root_symbol_ref(module_idx(1), "namespace_one");
    for (importer, refs) in [
      (0, [default_zero, named_zero, invalid_zero, namespace_zero]),
      (1, [default_one, named_one, invalid_one, namespace_one]),
    ] {
      add_named_import(&mut modules, importer, 0, refs[0], Specifier::Literal("default".into()));
      add_named_import(&mut modules, importer, 1, refs[1], Specifier::Literal("named".into()));
      add_named_import(&mut modules, importer, 2, refs[2], Specifier::Literal("not-valid".into()));
      add_named_import(&mut modules, importer, 3, refs[3], Specifier::Star);
    }

    let (output, diagnostics) = bind(&modules, symbols, OutputFormat::Esm, false, &[], |_| {});
    assert!(diagnostics.is_empty());
    let BindImportsOutput { symbols, normal_export_chains, external_namespace_merges, .. } = output;
    let default_target = symbols.canonical_ref_for(default_zero);
    assert_eq!(symbols.canonical_ref_for(default_one), default_target);
    let named_target = symbols.canonical_ref_for(named_zero);
    assert_eq!(symbols.canonical_ref_for(named_one), named_target);
    let invalid_target = symbols.canonical_ref_for(invalid_zero);
    assert_eq!(symbols.canonical_ref_for(invalid_one), invalid_target);
    let namespace_merges = external_namespace_merges.into_inner();
    let namespace_merges = namespace_merges[&module_idx(2)].iter().copied().collect::<Vec<_>>();
    assert_eq!(namespace_merges, [namespace_zero, namespace_one]);
    let normal_export_chains = normal_export_chains.into_inner();
    assert_eq!(normal_export_chains.len(), 8);
    assert!(normal_export_chains.values().all(Vec::is_empty));
    ExternalSummary {
      default_target,
      default_name: default_target.name(&symbols).to_string(),
      named_target,
      named_name: named_target.name(&symbols).to_string(),
      invalid_target,
      invalid_name: invalid_target.name(&symbols).to_string(),
      namespace_merges,
      chain_count: normal_export_chains.len(),
    }
  }

  #[test]
  fn groups_external_esm_bindings_and_commits_facades_in_deterministic_name_order() {
    let first = bind_external_groups();
    let second = bind_external_groups();
    for summary in [&first, &second] {
      assert_eq!(summary.default_target.owner, module_idx(2));
      assert_eq!(summary.default_target.symbol.index(), 1);
      assert_eq!(summary.default_name, "z_default");
      assert_eq!(summary.named_target.owner, module_idx(2));
      assert_eq!(summary.named_target.symbol.index(), 2);
      assert_eq!(summary.named_name, "named");
      assert_eq!(summary.invalid_target.owner, module_idx(2));
      assert_eq!(summary.invalid_target.symbol.index(), 3);
      assert_eq!(summary.invalid_name, "not_valid");
      assert_eq!(summary.namespace_merges.len(), 2);
      assert_eq!(summary.namespace_merges[0].owner, module_idx(0));
      assert_eq!(summary.namespace_merges[1].owner, module_idx(1));
      assert_eq!(summary.chain_count, 8);
    }
    assert_eq!(first.default_target, second.default_target);
    assert_eq!(first.named_target, second.named_target);
    assert_eq!(first.invalid_target, second.invalid_target);
  }
}
