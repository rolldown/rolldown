//! The decision, once for each build, whether the build keeps the deferred evaluation of
//! `import defer`. See internal-docs/import-defer/implementation.md ("Whole-build fallback").

use oxc::diagnostics::LabeledSpan;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, EntryPointKind, ExportsKind, ExternalModule, ImportKind, ImportRecordMeta,
  IndexModules, Module, ModuleIdx, NormalizedBundlerOptions, OutputFormat, ResolvedImportRecord,
  StmtEvalFlags, side_effects::DeterminedSideEffects,
};
use rolldown_error::{BuildDiagnostic, EventKind};
use rolldown_utils::{IndexBitSet, node_builtin::is_node_like_builtin};

use crate::{
  stages::scan_stage::NormalizedScanStageOutput,
  utils::prepare_build_context::has_non_recursive_dependency_capture,
};

/// Decides if the build keeps the deferred evaluation of each `import defer`. It runs after the
/// scan and before the link stage, on all modules of the build.
///
/// The scanner keeps the results of an eager import and saves the deferred results in
/// `EcmaView::import_defer_scan`. A build that keeps the deferral gets the saved results here. A
/// build that does not keep it lowers each deferred import to a normal import: it gives the output
/// of the build with `experimental.importDefer` off, and a warning for each deferred import.
///
/// Returns the `assign_to_import` errors of a build that lowers the deferred imports.
pub fn apply_import_defer_support(
  output: &mut NormalizedScanStageOutput,
  options: &NormalizedBundlerOptions,
) -> Result<(), Vec<BuildDiagnostic>> {
  if !options.experimental.is_import_defer_enabled() {
    return Ok(());
  }
  let has_deferred_import = output
    .module_table
    .modules
    .iter()
    .filter_map(Module::as_normal)
    .any(|module| module.import_records.iter().any(ResolvedImportRecord::is_defer_phase));
  if !has_deferred_import {
    return Ok(());
  }
  match unsupported_reason(output, options) {
    None => {
      keep_deferral(output);
      Ok(())
    }
    Some(reason) => lower_deferred_imports(output, &reason),
  }
}

/// Why a build does not keep the deferred evaluation. Each reason is a feature that the link and
/// generate stages do not support for `import defer` yet.
#[derive(Debug)]
enum UnsupportedReason {
  InlineCommonChunks,
  NonRecursiveGroups,
  SerialAsyncImports(ModuleIdx),
  PreservedImporterCycle(ModuleIdx),
  /// An `import.defer()` call targets this CommonJS module.
  DynamicImportOfCommonJs(ModuleIdx),
  /// A deferred import evaluates this external module, and the output format evaluates it before
  /// the first access (`deferred_closures`).
  ExternalModule(ModuleIdx, OutputFormat),
  /// A deferred closure has this CommonJS module, and its output file runs it at load.
  CommonJsRunAtLoad(ModuleIdx),
  /// This module is outside the deferred closures. It has a deferred import of a module that
  /// reaches a top-level `await`, and a later import of a module outside the closures.
  ImportAfterAsyncDeferredImport(ModuleIdx),
}

impl UnsupportedReason {
  /// The warning for a static deferred import, and the warning for an `import.defer()` call.
  fn messages(&self, modules: &IndexModules) -> (String, String) {
    let condition = match self {
      Self::InlineCommonChunks => {
        "`output.codeSplitting.experimentalInlineCommonChunks` is on".to_string()
      }
      Self::NonRecursiveGroups => {
        "a code splitting group sets `includeDependenciesRecursively: false`, so the chunks can import each other in a cycle"
          .to_string()
      }
      Self::SerialAsyncImports(idx) => format!(
        "the imports in `{}` can start a later dependency while a top-level `await` is pending, but deferred initialization would wait for them in sequence",
        modules[*idx].stable_id()
      ),
      Self::PreservedImporterCycle(idx) => format!(
        "`preserveModules` is on and a static import cycle can access the deferred namespace in `{}` before its runtime helpers load",
        modules[*idx].stable_id()
      ),
      Self::DynamicImportOfCommonJs(idx) => format!(
        "an `import.defer()` call targets the CommonJS module `{}`",
        modules[*idx].stable_id()
      ),
      Self::ExternalModule(idx, OutputFormat::Esm) => format!(
        "a deferred import evaluates the external module `{}`, and `esm` output imports that module at the start of the chunk",
        modules[*idx].stable_id()
      ),
      Self::ExternalModule(idx, OutputFormat::Cjs) => format!(
        "a deferred import evaluates the external module `{}` through `export *`, and the `cjs` output file of an entry module loads the external modules of its `export *` when the file loads",
        modules[*idx].stable_id()
      ),
      Self::ExternalModule(idx, format) => format!(
        "a deferred import evaluates the external module `{}`, and `{format}` output gets that module as a global or as a wrapper argument",
        modules[*idx].stable_id()
      ),
      Self::CommonJsRunAtLoad(idx) => format!(
        "a deferred import evaluates the CommonJS module `{}`, and the output file of that module runs it when the file loads",
        modules[*idx].stable_id()
      ),
      Self::ImportAfterAsyncDeferredImport(idx) => format!(
        "an import in `{}` comes after a deferred import of a module that has a top-level `await` or imports one",
        modules[*idx].stable_id()
      ),
    };
    (
      format!(
        "`import defer` is currently lowered to a normal import, because {condition}. This changes execution timing because side effects run immediately instead of when the deferred import is first used. `experimental.importDefer` does not keep the deferred evaluation in this build yet."
      ),
      format!(
        "`import.defer()` is currently lowered to `import()`, because {condition}. This changes execution timing because the module evaluates when it loads instead of when its namespace is first used. `experimental.importDefer` does not keep the deferred evaluation in this build yet."
      ),
    )
  }
}

fn unsupported_reason(
  output: &NormalizedScanStageOutput,
  options: &NormalizedBundlerOptions,
) -> Option<UnsupportedReason> {
  // Each eager importer of a deferred module in another chunk calls `init_x()` in its body, after
  // the later imports of the chunk run (`plan_lazy_host_inits` does not run here).
  if options.inline_common_chunks.is_some() {
    return Some(UnsupportedReason::InlineCommonChunks);
  }
  // In a cycle of chunk imports, a chunk can run before another chunk of the cycle exports its
  // init functions (`require_x.init_x is not a function` in `cjs` output) or, when the runtime is
  // in the cycle, before the runtime helpers exist (`__esmMin is not a function`).
  if options.manual_code_splitting.as_ref().is_some_and(has_non_recursive_dependency_capture) {
    return Some(UnsupportedReason::NonRecursiveGroups);
  }
  let commonjs = commonjs_modules(output, options);
  if let Some(module) = dynamic_import_of_commonjs(output, &commonjs) {
    return Some(UnsupportedReason::DynamicImportOfCommonJs(module));
  }
  let closures = match deferred_closures(output, options) {
    Ok(closures) => closures,
    Err(external) => return Some(UnsupportedReason::ExternalModule(external, options.format)),
  };
  if let Some(module) = commonjs_module_run_at_load(output, options, &closures, &commonjs) {
    return Some(UnsupportedReason::CommonJsRunAtLoad(module));
  }
  if options.preserve_modules
    && let Some(module) = preserved_importer_cycle(output)
  {
    return Some(UnsupportedReason::PreservedImporterCycle(module));
  }
  unsupported_async_import_order(output, options, &closures)
}

/// A sibling in an eager cycle can call an exported function or read a re-exported namespace
/// before the deferred host and its helpers load. Declarations in separate files cannot move
/// ahead of that call. See internal-docs/import-defer/implementation.md.
fn preserved_importer_cycle(output: &NormalizedScanStageOutput) -> Option<ModuleIdx> {
  let modules = &output.module_table.modules;
  let mut cycles = None;
  modules
    .iter()
    .filter_map(Module::as_normal)
    .filter(|module| {
      module.import_records.iter().any(|rec| {
        rec.kind == ImportKind::Import
          && rec.is_defer_phase()
          && rec.resolved_module.is_some_and(|idx| modules[idx].is_normal())
      })
    })
    .filter(|module| {
      module.named_exports.iter().any(|(name, export)| {
        // An anonymous default function has a synthetic export symbol.
        (name.as_str() == "default" && output.symbol_ref_db.is_facade_symbol(export.referenced))
          || export.referenced.is_declared_by_function(&output.symbol_ref_db)
          || module
            .named_imports
            .get(&export.referenced)
            .is_some_and(|import| module.import_records[import.record_idx].is_defer_phase())
      })
    })
    .filter(|module| {
      cycles
        .get_or_insert_with(|| crate::utils::eager_import_cycle_modules(modules))
        .contains(&module.idx)
    })
    .min_by_key(|module| &module.stable_id)
    .map(|module| module.idx)
}

/// The modules that the link stage makes CommonJS (`determine_module_exports_kind`): the scan gives
/// `ExportsKind::CommonJs`, or `ExportsKind::None` and a `require()` loads the module. With code
/// splitting off, an `import()` also counts as a `require()`.
fn commonjs_modules(
  output: &NormalizedScanStageOutput,
  options: &NormalizedBundlerOptions,
) -> IndexBitSet<ModuleIdx> {
  let modules = &output.module_table.modules;
  let code_splitting_disabled = options.code_splitting.is_disabled();
  let mut required = IndexBitSet::new(modules.len());
  for rec in modules.iter().filter_map(Module::as_normal).flat_map(|module| &module.import_records)
  {
    if rec.kind == ImportKind::Require
      || (rec.kind == ImportKind::DynamicImport && code_splitting_disabled)
    {
      if let Some(target) = rec.resolved_module {
        required.set_bit(target);
      }
    }
  }
  let mut commonjs = IndexBitSet::new(modules.len());
  for module in modules.iter().filter_map(Module::as_normal) {
    let is_commonjs = match module.exports_kind {
      ExportsKind::CommonJs => true,
      ExportsKind::None => required.has_bit(module.idx),
      ExportsKind::Esm => false,
    };
    if is_commonjs {
      commonjs.set_bit(module.idx);
    }
  }
  commonjs
}

/// The first CommonJS module that an `import.defer()` call targets. Native ESM defers the
/// evaluation of a CommonJS module, as for the static form, and `import()` evaluates it when it
/// loads. The deferred namespace of a CommonJS module (`create_deferred_cjs_namespaces`) exists
/// only for the static form.
fn dynamic_import_of_commonjs(
  output: &NormalizedScanStageOutput,
  commonjs: &IndexBitSet<ModuleIdx>,
) -> Option<ModuleIdx> {
  output
    .module_table
    .modules
    .iter()
    .filter_map(Module::as_normal)
    .flat_map(|module| &module.import_records)
    .filter(|rec| rec.kind == ImportKind::DynamicImport && rec.is_defer_phase())
    .filter_map(|rec| rec.resolved_module)
    .find(|&target| commonjs.has_bit(target))
}

/// The modules of the deferred closures, or `Err` with the first external module that a record
/// of a closure evaluates before the first access. A closure is a deferred target and its static
/// dependencies. The link stage wraps these modules (`wrap_import_defer_closures`), and this walk
/// also follows only the static records. The records of an external module that evaluate it
/// early:
/// - `cjs` output: a module of a closure loads an external module at the position of the import,
///   in its init (`lazy_external_import_stmt`). A deferred import of an external module loads it
///   at the first access (`__deferNsCached`). But the output file of an entry module loads the
///   external modules of its `export *` chain when the file loads (`render_chunk_exports`:
///   `var ext = require("ext")` and a copy of the keys of `ext`). A module of a closure can be the
///   entry module of a file: with `preserveModules`, or as an entry point (user-defined, emitted,
///   or the target of an `import()` or `import.defer()` call). The check does not know the chunks,
///   so it also counts an entry module that gets an entry facade.
/// - `esm` output: an eager import or re-export of an external module in a module of a closure.
///   The chunk imports the external module at its start. The output keeps a deferred import of
///   an external module (`import defer`, `import.defer()`). A `require()` call stays at its
///   position.
/// - `iife` and `umd` output: also a deferred import of an external module. The output gets an
///   external module as a global or as a wrapper argument.
///
/// A builtin module and an external module with no side effects do not count: their early
/// evaluation is not observable. See internal-docs/import-defer/implementation.md ("External
/// modules").
fn deferred_closures(
  output: &NormalizedScanStageOutput,
  options: &NormalizedBundlerOptions,
) -> Result<IndexBitSet<ModuleIdx>, ModuleIdx> {
  let modules = &output.module_table.modules;
  let runtime_idx = output.runtime.id();
  let format = options.format;
  let is_observable = |external: &ExternalModule| {
    external.side_effects.has_side_effects() && !is_node_like_builtin(external.id.as_str())
  };
  let evaluates_external_early = |rec: &ResolvedImportRecord, external: &ExternalModule| {
    let early = match format {
      OutputFormat::Cjs => false,
      OutputFormat::Esm => rec.kind == ImportKind::Import && !rec.is_defer_phase(),
      OutputFormat::Iife | OutputFormat::Umd => {
        rec.kind == ImportKind::Import || rec.is_defer_phase()
      }
    };
    early && is_observable(external)
  };
  let mut stack = vec![];
  // Adds the target of `rec` to the walk, or returns the external module that it evaluates early.
  let push = |stack: &mut Vec<ModuleIdx>, rec: &ResolvedImportRecord| {
    let Some(target) = rec.resolved_module else { return Ok(()) };
    match &modules[target] {
      Module::Normal(_) => stack.push(target),
      Module::External(external) if evaluates_external_early(rec, external) => {
        return Err(target);
      }
      Module::External(_) => {}
    }
    Ok(())
  };
  for module in modules.iter().filter_map(Module::as_normal) {
    for rec in module.import_records.iter().filter(|rec| rec.is_defer_phase()) {
      push(&mut stack, rec)?;
    }
  }
  let mut visited = IndexBitSet::new(modules.len());
  while let Some(idx) = stack.pop() {
    if idx == runtime_idx || !visited.set_bit(idx) {
      continue;
    }
    let Module::Normal(module) = &modules[idx] else { continue };
    for rec in module.import_records.iter().filter(|rec| rec.kind.is_static()) {
      push(&mut stack, rec)?;
    }
  }
  if matches!(format, OutputFormat::Cjs) {
    // The `export *` chains of the modules of the closures that can be entry modules.
    let mut stack = if options.preserve_modules {
      visited.index_of_one().collect::<Vec<_>>()
    } else {
      output
        .entry_points
        .iter()
        .map(|entry| entry.idx)
        .filter(|&idx| visited.has_bit(idx))
        .collect()
    };
    let mut star_visited = IndexBitSet::new(modules.len());
    while let Some(idx) = stack.pop() {
      if !star_visited.set_bit(idx) {
        continue;
      }
      let Module::Normal(module) = &modules[idx] else { continue };
      for rec in &module.import_records {
        if !rec.meta.contains(ImportRecordMeta::IsExportStar) {
          continue;
        }
        let Some(target) = rec.resolved_module else { continue };
        match &modules[target] {
          Module::Normal(_) => stack.push(target),
          Module::External(external) if is_observable(external) => return Err(target),
          Module::External(_) => {}
        }
      }
    }
  }
  Ok(visited)
}

/// Finds import lists where deferred preparation or an async wrapper changes when a sibling
/// starts. Uses the early top-level await flag of `mark_tla_before_tree_shaking`.
///
/// The preparation of the deferred import (`__esmGather`) waits for the modules with top-level
/// `await` in the body of the importer. A module outside the closures has no wrapper, so the code
/// of its imports runs before its body: the later import runs before the modules with top-level
/// `await` start. In native order, they start first. Async wrappers can also suspend before later
/// imports run, where native ESM would start the remaining siblings. In a build with
/// one chunk, an external module runs first also without the deferral, so it does not count. In a
/// build with more chunks, a chunk imports an external module in the order of its imports.
fn unsupported_async_import_order(
  output: &NormalizedScanStageOutput,
  options: &NormalizedBundlerOptions,
  closures: &IndexBitSet<ModuleIdx>,
) -> Option<UnsupportedReason> {
  let modules = &output.module_table.modules;
  let normal_modules = || modules.iter().filter_map(Module::as_normal);
  let multiple_chunks = can_have_more_chunks(output, options);
  let mut queue = normal_modules()
    .filter(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait))
    .map(|module| module.idx)
    .collect::<Vec<_>>();
  if queue.is_empty() {
    return None;
  }
  let mut importers = IndexVec::<ModuleIdx, Vec<ModuleIdx>>::from_vec(vec![vec![]; modules.len()]);
  for module in normal_modules() {
    for rec in &module.import_records {
      if matches!(rec.kind, ImportKind::Import)
        && let Some(dependency) = rec.resolved_module
      {
        importers[dependency].push(module.idx);
      }
    }
  }
  let mut reaches_tla = IndexBitSet::new(modules.len());
  for &idx in &queue {
    reaches_tla.set_bit(idx);
  }
  while let Some(dependency) = queue.pop() {
    for &importer in &importers[dependency] {
      if reaches_tla.set_bit(importer) {
        queue.push(importer);
      }
    }
  }
  normal_modules().find_map(|module| {
    let wrapped = closures.has_bit(module.idx);
    let own_tla = module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait);
    let mut after_async_import = false;
    let mut after_async_deferred_import = false;
    for (index, rec) in module.import_records.iter().enumerate() {
      let Some(target) = rec.resolved_module else { continue };
      if rec.kind != ImportKind::Import || target == output.runtime.id() {
        continue;
      }
      let is_async = reaches_tla.has_bit(target);
      if !wrapped
        && after_async_deferred_import
        && !rec.is_defer_phase()
        && !closures.has_bit(target)
        && (multiple_chunks || modules[target].is_normal())
      {
        return Some(UnsupportedReason::ImportAfterAsyncDeferredImport(module.idx));
      }
      // Native module loading starts siblings before waiting for their async evaluations.
      // A deferred preparation in the importer body cannot start while an earlier import
      // waits. An own-TLA wrapper likewise awaits an eager dependency before its next init.
      let later_init = if rec.is_defer_phase() {
        is_async
      } else {
        wrapped && own_tla && modules[target].side_effects().has_side_effects()
      };
      if multiple_chunks
        && after_async_import
        && later_init
        && needs_new_evaluation(
          module.import_records.iter().take(index),
          target,
          rec.is_defer_phase(),
          modules,
          module.idx,
        )
      {
        return Some(UnsupportedReason::SerialAsyncImports(module.idx));
      }
      after_async_import |= is_async;
      after_async_deferred_import |= rec.is_defer_phase() && is_async;
    }
    None
  })
}

/// An eager import needs its target's body. A deferred import only needs the modules with
/// their own top-level await in its static closure. An earlier request, eager or deferred,
/// already starts the async modules it reaches. Stop paths back through the importer so its
/// later requests cannot count as earlier ones.
fn needs_new_evaluation<'a>(
  earlier: impl Iterator<Item = &'a ResolvedImportRecord>,
  target: ModuleIdx,
  deferred: bool,
  modules: &IndexModules,
  importer: ModuleIdx,
) -> bool {
  let dependency = |rec: &ResolvedImportRecord| {
    if rec.kind == ImportKind::Import && (deferred || !rec.is_defer_phase()) {
      rec.resolved_module
    } else {
      None
    }
  };
  let mut stack: Vec<_> = earlier.filter_map(dependency).collect();
  let mut visited = rustc_hash::FxHashSet::default();
  visited.insert(importer);
  while let Some(idx) = stack.pop() {
    if visited.insert(idx) {
      stack.extend(modules[idx].import_records().iter().filter_map(dependency));
    }
  }
  if !deferred {
    return !visited.contains(&target);
  }
  stack.push(target);
  while let Some(idx) = stack.pop() {
    if !visited.insert(idx) {
      continue;
    }
    let Some(module) = modules[idx].as_normal() else { continue };
    if module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait) {
      return true;
    }
    stack.extend(module.import_records.iter().filter_map(dependency));
  }
  false
}

/// The first CommonJS module of the deferred closures whose output file runs the module at load.
/// A deferred importer imports that file, so the module runs before the first access:
/// - With `preserveModules` and `esm` output, the file of each CommonJS module ends with
///   `export default require_x();`. In `cjs` output, the file exports a `default` getter.
/// - In `cjs` output, the chunk of a user-defined entry resolves its own export mode
///   (`export_mode_module`), and it can end with `module.exports = require_x();`. In `esm` output
///   without `preserveModules`, such a chunk gets a facade (`preserve_strict_entry_signatures`).
fn commonjs_module_run_at_load(
  output: &NormalizedScanStageOutput,
  options: &NormalizedBundlerOptions,
  closures: &IndexBitSet<ModuleIdx>,
  commonjs: &IndexBitSet<ModuleIdx>,
) -> Option<ModuleIdx> {
  let each_file = options.preserve_modules && matches!(options.format, OutputFormat::Esm);
  if !each_file && !matches!(options.format, OutputFormat::Cjs) {
    return None;
  }
  if each_file {
    return closures.index_of_one().find(|&idx| commonjs.has_bit(idx));
  }
  output
    .entry_points
    .iter()
    .filter(|entry| matches!(entry.kind, EntryPointKind::UserDefined))
    .map(|entry| entry.idx)
    .find(|&idx| closures.has_bit(idx) && commonjs.has_bit(idx))
}

/// Whether the build can make more than one chunk. A build with one user-defined or emitted entry,
/// no `import()`, no `preserveModules` and no code splitting groups makes one chunk, and so does a
/// build with code splitting off.
fn can_have_more_chunks(
  output: &NormalizedScanStageOutput,
  options: &NormalizedBundlerOptions,
) -> bool {
  !options.code_splitting.is_disabled()
    && (options.preserve_modules
      || options.has_manual_code_splitting_groups()
      || output.entry_points.iter().filter(|entry| entry.kind.is_user_defined()).count() != 1
      || output
        .module_table
        .modules
        .iter()
        .filter_map(Module::as_normal)
        .any(|module| module.import_records.iter().any(|rec| rec.kind.is_dynamic())))
}

/// Writes the saved results of the deferral into the scan output.
fn keep_deferral(output: &mut NormalizedScanStageOutput) {
  for module in output.module_table.modules.iter_mut().filter_map(Module::as_normal_mut) {
    let Some(scan) = module.ecma_view.import_defer_scan.as_deref() else { continue };
    let stmt_infos = &mut output.stmt_infos[module.idx];
    let mut adds_side_effect = false;
    for &(stmt_idx, flags) in &scan.eval_flags {
      let eval_flags = &mut stmt_infos[stmt_idx].eval_flags;
      adds_side_effect |= flags.contains(StmtEvalFlags::UnknownSideEffect)
        && !eval_flags.contains(StmtEvalFlags::UnknownSideEffect);
      // The deferred analysis only adds side effects to the eager one, so the union keeps each
      // statement that one of them keeps.
      *eval_flags |= flags;
    }
    // `lazy_check_side_effects` computed the side effects of the module from the eager flags.
    if adds_side_effect
      && matches!(module.ecma_view.side_effects, DeterminedSideEffects::Analyzed(false))
    {
      module.ecma_view.side_effects = DeterminedSideEffects::Analyzed(true);
    }
  }
}

/// Lowers each deferred import to a normal import. The scan results of the modules are already
/// the results of an eager import, except the writes to a deferred namespace.
fn lower_deferred_imports(
  output: &mut NormalizedScanStageOutput,
  reason: &UnsupportedReason,
) -> Result<(), Vec<BuildDiagnostic>> {
  let (static_message, dynamic_message) = reason.messages(&output.module_table.modules);
  let mut warnings = vec![];
  let mut errors = vec![];
  for module in output.module_table.modules.iter_mut().filter_map(Module::as_normal_mut) {
    for rec in module.ecma_view.import_records.iter_mut().filter(|rec| rec.is_defer_phase()) {
      rec.meta.remove(ImportRecordMeta::DeferPhase);
      let message =
        if rec.kind == ImportKind::DynamicImport { &dynamic_message } else { &static_message };
      let span = rec.state.importer_span;
      warnings.push(
        BuildDiagnostic::oxc_error(
          module.ecma_view.source.clone(),
          module.id.as_str().to_string(),
          String::new(),
          message.clone(),
          vec![LabeledSpan::at(span.start..span.end, "The deferred phase is removed here.")],
          EventKind::UnsupportedFeatureError,
        )
        .with_severity_warning(),
      );
    }
    if let Some(scan) = module.ecma_view.import_defer_scan.take() {
      // With an eager import, the scanner reports these errors.
      errors.extend(scan.namespace_writes.into_iter().map(|write| {
        BuildDiagnostic::assign_to_import(
          module.id.as_arc_str().clone(),
          module.ecma_view.source.clone(),
          write.span,
          write.name,
          Some(write.import_decl_span),
          Some(write.imported_name),
        )
      }));
    }
  }
  if !errors.is_empty() {
    return Err(errors);
  }
  output.warnings.extend(warnings);
  Ok(())
}
