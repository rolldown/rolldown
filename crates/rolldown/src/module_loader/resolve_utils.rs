use std::sync::Arc;

use arcstr::ArcStr;
use futures::future::join_all;
use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleDefFormat, ModuleIdx, ModuleType,
  NormalizedBundlerOptions, RUNTIME_MODULE_KEY, RawImportRecord, ResolvedId, ImporterRecord,
};
use rolldown_error::{BuildDiagnostic, BuildResult, DiagnosableArcstr, EventKind};
use rolldown_plugin::{__inner::resolve_id_check_external, PluginDriver, SharedPluginDriver};
use rolldown_resolver::{ResolveError, Resolver};
use rolldown_utils::ecmascript::{self};
use rustc_hash::FxHashSet;

use crate::{SharedOptions, SharedResolver};

/// Build import chain from the given module back to entry points
/// Returns a list of module paths from entry to the given module
pub fn build_import_chain(
  module_idx: ModuleIdx,
  importers: &IndexVec<ModuleIdx, Vec<ImporterRecord>>,
  modules: &[Option<String>],
) -> Option<Vec<String>> {
  let mut chain = Vec::new();
  let mut visited = FxHashSet::default();
  let current = module_idx;
  
  // Trace back through importers to find a path to an entry point
  // We'll do a simple depth-first search to find one path
  fn trace_to_entry(
    current: ModuleIdx,
    importers: &IndexVec<ModuleIdx, Vec<ImporterRecord>>,
    modules: &[Option<String>],
    visited: &mut FxHashSet<ModuleIdx>,
    chain: &mut Vec<String>,
  ) -> bool {
    // Prevent infinite loops
    if !visited.insert(current) {
      return false;
    }
    
    // Add current module to chain
    if let Some(Some(module_id)) = modules.get(current.index()) {
      chain.push(module_id.clone());
    } else {
      return false;
    }
    
    // If this module has no importers, it's an entry point
    if importers[current].is_empty() {
      return true;
    }
    
    // Try to trace through the first importer
    // (We could make this more sophisticated to find the "best" path)
    if let Some(importer) = importers[current].first() {
      if trace_to_entry(importer.importer_idx, importers, modules, visited, chain) {
        return true;
      }
    }
    
    // Backtrack if we didn't find a path
    chain.pop();
    visited.remove(&current);
    false
  }
  
  if trace_to_entry(current, importers, modules, &mut visited, &mut chain) {
    // Reverse the chain so it goes from entry to current module
    chain.reverse();
    Some(chain)
  } else {
    None
  }
}

#[tracing::instrument(skip_all, fields(CONTEXT_hook_resolve_id_trigger = "automatic"))]
pub async fn resolve_id(
  bundle_options: &NormalizedBundlerOptions,
  resolver: &Resolver,
  plugin_driver: &PluginDriver,
  importer: &str,
  specifier: &str,
  kind: ImportKind,
) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
  // Check runtime module
  if specifier == RUNTIME_MODULE_KEY {
    return Ok(Ok(ResolvedId {
      id: specifier.into(),
      module_def_format: ModuleDefFormat::EsmMjs,
      ..Default::default()
    }));
  }

  resolve_id_check_external(
    resolver,
    plugin_driver,
    specifier,
    Some(importer),
    false,
    kind,
    None,
    Arc::default(),
    false,
    bundle_options,
  )
  .await
}

#[expect(clippy::too_many_arguments)]
pub async fn resolve_dependencies(
  self_resolved_id: &ResolvedId,
  options: &SharedOptions,
  resolver: &SharedResolver,
  plugin_driver: &SharedPluginDriver,
  dependencies: &IndexVec<ImportRecordIdx, RawImportRecord>,
  source: ArcStr,
  warnings: &mut Vec<BuildDiagnostic>,
  module_type: &ModuleType,
) -> BuildResult<IndexVec<ImportRecordIdx, ResolvedId>> {
  let jobs = dependencies.iter_enumerated().map(async |(idx, item)| {
    let importer = &self_resolved_id.id;
    let specifier = &item.module_request;
    resolve_id(options, resolver, plugin_driver, importer, specifier, item.kind)
      .await
      .map(|id| (idx, id))
  });

  // FIXME: if the import records came from css view, but source from ecma view,
  // the span will not matched.
  let is_css_module = matches!(module_type, ModuleType::Css);
  let mut ret = IndexVec::with_capacity(dependencies.len());
  let mut build_errors = vec![];
  for resolved_id in join_all(jobs).await {
    let (idx, resolved_id) = resolved_id?;

    match resolved_id {
      Ok(info) => {
        ret.push(info);
      }
      Err(e) => {
        let dep = &dependencies[idx];
        let specifier = &dep.module_request;
        match &e {
          ResolveError::NotFound(..) => {
            // NOTE: IN_TRY_CATCH_BLOCK meta if it is a `require` import
            // record
            if !dep.meta.contains(ImportRecordMeta::InTryCatchBlock) {
              // https://github.com/rollup/rollup/blob/49b57c2b30d55178a7316f23cc9ccc457e1a2ee7/src/ModuleLoader.ts#L643-L646
              if ecmascript::is_path_like_specifier(specifier) {
                // TODO: Build import chain for UNRESOLVED_IMPORT error
                // For now, pass None - we'll implement this after understanding the data flow better
                // Unlike rollup, we also emit errors for absolute path
                build_errors.push(BuildDiagnostic::resolve_error(
                  source.clone(),
                  self_resolved_id.id.clone(),
                  if dep.is_unspanned() || is_css_module {
                    DiagnosableArcstr::String(specifier.as_str().into())
                  } else {
                    DiagnosableArcstr::Span(dep.state.span)
                  },
                  "Module not found.".into(),
                  EventKind::UnresolvedImport,
                  None,
                  None,
                ));
              } else {
                // TODO: Build import chain for UNRESOLVED_IMPORT warning
                // For now, pass None - we'll implement this after understanding the data flow better
                let help = matches!(options.platform, rolldown_common::Platform::Neutral).then(|| {
                  r#"The "main" field here was ignored. Main fields must be configured explicitly when using the "neutral" platform."#.to_string()
                });
                warnings.push(
                  BuildDiagnostic::resolve_error(
                    source.clone(),
                    self_resolved_id.id.clone(),
                    if dep.is_unspanned() || is_css_module {
                      DiagnosableArcstr::String(specifier.as_str().into())
                    } else {
                      DiagnosableArcstr::Span(dep.state.span)
                    },
                    "Module not found, treating it as an external dependency".into(),
                    EventKind::UnresolvedImport,
                    help,
                    None,
                  )
                  .with_severity_warning(),
                );
              }
            }
            ret.push(ResolvedId {
              id: specifier.as_str().into(),
              external: true.into(),
              ..Default::default()
            });
          }
          ResolveError::MatchedAliasNotFound(..) => {
            build_errors.push(BuildDiagnostic::resolve_error(
                source.clone(),
                self_resolved_id.id.clone(),
                if dep.is_unspanned() || is_css_module {
                  DiagnosableArcstr::String(specifier.as_str().into())
                } else {
                  DiagnosableArcstr::Span(dep.state.span)
                },
                format!("Matched alias not found for '{specifier}'"),
                    EventKind::ResolveError,
                Some("May be you expected `resolve.alias` to call other plugins resolveId hook? see the docs https://rolldown.rs/apis/config-options#resolve-alias for more details".to_string()),
                None,
              ));
          }
          e => {
            build_errors.push(BuildDiagnostic::resolve_error(
              source.clone(),
              self_resolved_id.id.clone(),
              if dep.is_unspanned() || is_css_module {
                DiagnosableArcstr::String(specifier.as_str().into())
              } else {
                DiagnosableArcstr::Span(dep.state.span)
              },
              rolldown_resolver::error::resolve_error_to_message(e),
              EventKind::ResolveError,
              None,
              None,
            ));
          }
        }
      }
    }
  }

  if build_errors.is_empty() { Ok(ret) } else { Err(build_errors.into()) }
}
