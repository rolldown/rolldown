//! The decision, once for each build, whether the build keeps the deferred evaluation of
//! `import defer`. See internal-docs/import-defer/implementation.md ("Whole-build fallback").

use oxc::diagnostics::LabeledSpan;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, ImportKind, ImportRecordMeta, IndexModules, Module, ModuleIdx,
  NormalizedBundlerOptions, OutputFormat, ResolvedImportRecord, StmtEvalFlags,
  side_effects::DeterminedSideEffects,
};
use rolldown_error::{BuildDiagnostic, EventKind};
use rolldown_utils::IndexBitSet;

use crate::stages::scan_stage::NormalizedScanStageOutput;

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
  EntryCount,
  PreserveModules,
  ChunkGroups,
  InlineCommonChunks,
  DynamicImport,
  /// A deferred closure has this external module.
  ExternalModule(ModuleIdx),
  /// This module is outside the deferred closures. It has a deferred import of a module that
  /// reaches a top-level `await`, and a later import of a module outside the closures.
  ImportAfterAsyncDeferredImport(ModuleIdx),
}

impl UnsupportedReason {
  fn message(&self, modules: &IndexModules) -> String {
    let condition = match self {
      Self::OutputFormat => "the output format is not `esm` or `cjs`".to_string(),
      Self::EntryCount => "the build has more than one entry".to_string(),
      Self::PreserveModules => "`output.preserveModules` is on".to_string(),
      Self::ChunkGroups => "the build has code splitting groups".to_string(),
      Self::InlineCommonChunks => {
        "`output.codeSplitting.experimentalInlineCommonChunks` is on".to_string()
      }
      Self::DynamicImport => "the build has an `import()`".to_string(),
      Self::ExternalModule(idx) => {
        format!("a deferred import evaluates the external module `{}`", modules[*idx].stable_id())
      }
      Self::ImportAfterAsyncDeferredImport(idx) => format!(
        "an import in `{}` comes after a deferred import of a module that has a top-level `await` or imports one",
        modules[*idx].stable_id()
      ),
    };
    format!(
      "`import defer` is currently lowered to a normal import, because {condition}. This changes execution timing because side effects run immediately instead of when the deferred import is first used. `experimental.importDefer` does not keep the deferred evaluation in this build yet."
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
  // The build must make one chunk. Count each user-defined or emitted entry record, also several
  // records of one module. The check of `import()` below covers the entries of `import()`.
  if output.entry_points.iter().filter(|entry| entry.kind.is_user_defined()).count() != 1 {
    return Some(UnsupportedReason::EntryCount);
  }
  if options.preserve_modules {
    return Some(UnsupportedReason::PreserveModules);
  }
  if options.has_manual_code_splitting_groups() {
    return Some(UnsupportedReason::ChunkGroups);
  }
  // With one entry, inline common chunks can still put the runtime in its own chunk.
  if options.inline_common_chunks.is_some() {
    return Some(UnsupportedReason::InlineCommonChunks);
  }
  let modules = &output.module_table.modules;
  let normal_modules = || modules.iter().filter_map(Module::as_normal);
  if normal_modules().any(|module| module.import_records.iter().any(|rec| rec.kind.is_dynamic())) {
    return Some(UnsupportedReason::DynamicImport);
  }
  let closures = match deferred_closures(output) {
    Ok(closures) => closures,
    Err(external) => return Some(UnsupportedReason::ExternalModule(external)),
  };
  import_after_async_deferred_import(output, &closures)
    .map(UnsupportedReason::ImportAfterAsyncDeferredImport)
}

/// The modules of the deferred closures, or `Err` with the first external module of a closure. A
/// closure is a deferred target and its static dependencies. The link stage wraps these modules
/// (`wrap_import_defer_closures`). That walk follows only the static records, but here the build
/// has no `import()`, so the two walks find the same modules.
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
    stack.extend(module.import_records.iter().filter_map(|rec| rec.resolved_module));
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
/// its import statements in order, and so does a later import of a closure module. An external
/// module runs first also in a build without the deferral, so it does not count.
fn import_after_async_deferred_import(
  output: &NormalizedScanStageOutput,
  closures: &IndexBitSet<ModuleIdx>,
) -> Option<ModuleIdx> {
  let modules = &output.module_table.modules;
  let normal_modules = || modules.iter().filter_map(Module::as_normal);
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
        && matches!(modules[target], Module::Normal(_))
      {
        return Some(module.idx);
      }
    }
    None
  })
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
  let message = reason.message(&output.module_table.modules);
  let mut warnings = vec![];
  let mut errors = vec![];
  for module in output.module_table.modules.iter_mut().filter_map(Module::as_normal_mut) {
    for rec in module.ecma_view.import_records.iter_mut().filter(|rec| rec.is_defer_phase()) {
      rec.meta.remove(ImportRecordMeta::DeferPhase);
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
