use std::sync::Arc;

use arcstr::ArcStr;
use futures::future::join_all;
use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleDefFormat, ModuleType,
  NormalizedBundlerOptions, RUNTIME_MODULE_KEY, RawImportRecord, ResolvedId,
};
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosableArcstr, EventKind, SingleBuildResult,
};
use rolldown_plugin::{__inner::resolve_id_check_external, PluginDriver, SharedPluginDriver};
use rolldown_resolver::{ResolveError, Resolver};
use rolldown_utils::ecmascript::{self};

use crate::{SharedOptions, SharedResolver};

#[tracing::instrument(skip_all, fields(CONTEXT_hook_resolve_id_trigger = "automatic"))]
pub async fn resolve_id(
  bundle_options: &NormalizedBundlerOptions,
  resolver: &Resolver,
  plugin_driver: &PluginDriver,
  importer: &str,
  specifier: &str,
  kind: ImportKind,
) -> SingleBuildResult<Result<ResolvedId, ResolveError>> {
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
                ));
              } else {
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
              ));
          }
          e => {
            let reason = rolldown_resolver::error::oxc_resolve_error_to_reason(e);
            build_errors.push(BuildDiagnostic::resolve_error(
              source.clone(),
              self_resolved_id.id.clone(),
              if dep.is_unspanned() || is_css_module {
                DiagnosableArcstr::String(specifier.as_str().into())
              } else {
                DiagnosableArcstr::Span(dep.state.span)
              },
              reason,
              EventKind::ResolveError,
              None,
            ));
          }
        }
      }
    }
  }

  if build_errors.is_empty() { Ok(ret) } else { Err(build_errors.into()) }
}
