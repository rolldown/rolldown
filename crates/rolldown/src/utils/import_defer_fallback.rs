//! The decision, once for each build, whether the build keeps the deferred evaluation of
//! `import defer`. See internal-docs/import-defer/implementation.md ("Whole-build fallback").

use oxc::diagnostics::LabeledSpan;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, ExportsKind, ImportKind, ImportRecordMeta, IndexModules, Module, ModuleIdx,
  ModuleType, NormalizedBundlerOptions, OutputFormat, ResolvedImportRecord, StmtEvalFlags,
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
  TopLevelAwait,
  ImportCycle,
  ClosureModule(ModuleIdx, ClosureModuleKind),
}

/// A module of a deferred closure that the deferral does not support yet.
#[derive(Debug)]
enum ClosureModuleKind {
  External,
  CommonJs,
  NotJavaScript,
  Required,
  OtherImportKind,
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
      Self::TopLevelAwait => "a module of the build has a top-level `await`".to_string(),
      Self::ImportCycle => "the build has a static import cycle".to_string(),
      Self::ClosureModule(idx, kind) => {
        let id = modules[*idx].stable_id();
        match kind {
          ClosureModuleKind::External => {
            format!("a deferred import evaluates the external module `{id}`")
          }
          ClosureModuleKind::CommonJs => {
            format!("a deferred import evaluates the CommonJS module `{id}`")
          }
          ClosureModuleKind::NotJavaScript => {
            format!("a deferred import evaluates `{id}`, which is not a JavaScript module")
          }
          ClosureModuleKind::Required => {
            format!("a deferred import evaluates `{id}`, which a `require()` call also loads")
          }
          ClosureModuleKind::OtherImportKind => format!(
            "a deferred import evaluates `{id}`, which has a `require()` call or another import that is not an `import` statement"
          ),
        }
      }
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
  if normal_modules().any(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait)) {
    return Some(UnsupportedReason::TopLevelAwait);
  }
  if has_static_import_cycle(modules) {
    return Some(UnsupportedReason::ImportCycle);
  }
  closure_reason(output)
}

/// Whether the static import records make a cycle. An iterative walk, for deep graphs.
fn has_static_import_cycle(modules: &IndexModules) -> bool {
  const NOT_VISITED: u8 = 0;
  const ON_PATH: u8 = 1;
  const DONE: u8 = 2;
  let mut state = IndexVec::<ModuleIdx, u8>::from_vec(vec![NOT_VISITED; modules.len()]);
  for root in modules.indices() {
    if state[root] != NOT_VISITED {
      continue;
    }
    state[root] = ON_PATH;
    // Each item is a module on the path and the position of its next record.
    let mut path = vec![(root, 0)];
    while let Some(&(idx, next)) = path.last() {
      let records = &modules[idx].import_records().raw;
      let dependency = records[next.min(records.len())..]
        .iter()
        .position(|rec| rec.kind.is_static() && rec.resolved_module.is_some())
        .map(|offset| next + offset);
      let Some(position) = dependency else {
        state[idx] = DONE;
        path.pop();
        continue;
      };
      if let Some(last) = path.last_mut() {
        last.1 = position + 1;
      }
      let dependency = records[position].resolved_module.expect("checked above");
      match state[dependency] {
        NOT_VISITED => {
          state[dependency] = ON_PATH;
          path.push((dependency, 0));
        }
        ON_PATH => return true,
        _ => {}
      }
    }
  }
  false
}

/// The first module of a deferred closure that the deferral does not support. A closure is a
/// deferred target and its static dependencies. This is the same walk as
/// `wrap_import_defer_closures` in the link stage.
fn closure_reason(output: &NormalizedScanStageOutput) -> Option<UnsupportedReason> {
  let modules = &output.module_table.modules;
  let runtime_idx = output.runtime.id();
  // A `require()` makes a module with `ExportsKind::None` a CommonJS module in the link stage.
  let mut required = IndexBitSet::new(modules.len());
  for module in modules.iter().filter_map(Module::as_normal) {
    for rec in module.import_records.iter().filter(|rec| rec.kind == ImportKind::Require) {
      if let Some(idx) = rec.resolved_module {
        required.set_bit(idx);
      }
    }
  }
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
      return Some(UnsupportedReason::ClosureModule(idx, ClosureModuleKind::External));
    };
    let kind = if matches!(module.exports_kind, ExportsKind::CommonJs) {
      ClosureModuleKind::CommonJs
    } else if !matches!(
      module.module_type,
      ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx
    ) {
      ClosureModuleKind::NotJavaScript
    } else if required.has_bit(idx) {
      ClosureModuleKind::Required
    } else if module.import_records.iter().any(|rec| {
      rec.kind.is_static() && rec.kind != ImportKind::Import && rec.resolved_module.is_some()
    }) {
      ClosureModuleKind::OtherImportKind
    } else {
      stack.extend(module.import_records.iter().filter_map(|rec| rec.resolved_module));
      continue;
    };
    return Some(UnsupportedReason::ClosureModule(idx, kind));
  }
  None
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
