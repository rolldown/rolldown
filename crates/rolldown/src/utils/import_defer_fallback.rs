//! The decision, once for each build, whether the build keeps the deferred evaluation of
//! `import defer`. See internal-docs/import-defer/implementation.md ("Whole-build fallback").

use oxc::diagnostics::LabeledSpan;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, EntryPointKind, ExportsKind, ImportKind, ImportRecordMeta, IndexModules,
  Module, ModuleIdx, NormalizedBundlerOptions, OutputFormat, ResolvedImportRecord, StmtEvalFlags,
  side_effects::DeterminedSideEffects,
};
use rolldown_error::{BuildDiagnostic, EventKind};
use rolldown_utils::IndexBitSet;

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
  OutputFormat,
  InlineCommonChunks,
  NonRecursiveGroups,
  /// An `import.defer()` call targets this CommonJS module.
  DynamicImportOfCommonJs(ModuleIdx),
  /// A deferred closure has this external module.
  ExternalModule(ModuleIdx),
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
      Self::OutputFormat => "the output format is not `esm` or `cjs`".to_string(),
      Self::InlineCommonChunks => {
        "`output.codeSplitting.experimentalInlineCommonChunks` is on".to_string()
      }
      Self::NonRecursiveGroups => {
        "a code splitting group sets `includeDependenciesRecursively: false`, so the chunks can import each other in a cycle"
          .to_string()
      }
      Self::DynamicImportOfCommonJs(idx) => format!(
        "an `import.defer()` call targets the CommonJS module `{}`",
        modules[*idx].stable_id()
      ),
      Self::ExternalModule(idx) => {
        format!("a deferred import evaluates the external module `{}`", modules[*idx].stable_id())
      }
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
  if !matches!(options.format, OutputFormat::Esm | OutputFormat::Cjs) {
    return Some(UnsupportedReason::OutputFormat);
  }
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
  let closures = match deferred_closures(output) {
    Ok(closures) => closures,
    Err(external) => return Some(UnsupportedReason::ExternalModule(external)),
  };
  if let Some(module) = commonjs_module_run_at_load(output, options, &closures, &commonjs) {
    return Some(UnsupportedReason::CommonJsRunAtLoad(module));
  }
  import_after_async_deferred_import(output, options, &closures)
    .map(UnsupportedReason::ImportAfterAsyncDeferredImport)
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

/// The modules of the deferred closures, or `Err` with the first external module of a closure. A
/// closure is a deferred target and its static dependencies. The link stage wraps these modules
/// (`wrap_import_defer_closures`), and this walk also follows only the static records.
fn deferred_closures(
  output: &NormalizedScanStageOutput,
) -> Result<IndexBitSet<ModuleIdx>, ModuleIdx> {
  let modules = &output.module_table.modules;
  let runtime_idx = output.runtime.id();
  let mut stack = modules
    .iter()
    .filter_map(Module::as_normal)
    .flat_map(|module| module.import_records.iter())
    .filter(|rec| rec.is_defer_phase())
    .filter_map(|rec| rec.resolved_module)
    .collect::<Vec<_>>();
  let mut visited = IndexBitSet::new(modules.len());
  while let Some(idx) = stack.pop() {
    if idx == runtime_idx || !visited.set_bit(idx) {
      continue;
    }
    let Module::Normal(module) = &modules[idx] else {
      return Err(idx);
    };
    stack.extend(
      module
        .import_records
        .iter()
        .filter(|rec| rec.kind.is_static())
        .filter_map(|rec| rec.resolved_module),
    );
  }
  Ok(visited)
}

/// The first module outside the deferred closures with an import of a module outside the closures
/// after a deferred import of a module that reaches a top-level `await` (the early flag of
/// `mark_tla_before_tree_shaking`).
///
/// The preparation of the deferred import (`__esmGather`) waits for the modules with top-level
/// `await` in the body of the importer. A module outside the closures has no wrapper, so the code
/// of its imports runs before its body: the later import runs before the modules with top-level
/// `await` start. In native order, they start first. A module of a closure has a wrapper that runs
/// its import statements in order, and so does a later import of a closure module. In a build with
/// one chunk, an external module runs first also without the deferral, so it does not count. In a
/// build with more chunks, a chunk imports an external module in the order of its imports.
fn import_after_async_deferred_import(
  output: &NormalizedScanStageOutput,
  options: &NormalizedBundlerOptions,
  closures: &IndexBitSet<ModuleIdx>,
) -> Option<ModuleIdx> {
  let modules = &output.module_table.modules;
  let normal_modules = || modules.iter().filter_map(Module::as_normal);
  let externals_count = can_have_more_chunks(output, options);
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
  normal_modules().filter(|module| !closures.has_bit(module.idx)).find_map(|module| {
    let mut after_async_deferred_import = false;
    for rec in &module.import_records {
      let Some(target) = rec.resolved_module else { continue };
      if !matches!(rec.kind, ImportKind::Import) {
        continue;
      }
      if rec.is_defer_phase() {
        after_async_deferred_import |= reaches_tla.has_bit(target);
      } else if after_async_deferred_import
        && !closures.has_bit(target)
        && target != output.runtime.id()
        && (externals_count || matches!(modules[target], Module::Normal(_)))
      {
        return Some(module.idx);
      }
    }
    None
  })
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
