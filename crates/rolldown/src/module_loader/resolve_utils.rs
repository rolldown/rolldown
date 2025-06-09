use arcstr::ArcStr;
use futures::future::join_all;
use oxc_index::IndexVec;
use rolldown_plugin::{__inner::resolve_id_check_external, SharedPluginDriver};
use rolldown_resolver::ResolveError;
use rolldown_utils::{
  concat_string,
  ecmascript::{self},
};
use std::sync::Arc;

use rolldown_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleDefFormat, ModuleType, RUNTIME_MODULE_KEY,
  RawImportRecord, ResolvedId,
};
use rolldown_error::{BuildDiagnostic, BuildResult, DiagnosableArcstr, EventKind};

use crate::{SharedOptions, SharedResolver};

#[tracing::instrument(skip_all, fields(CONTEXT_hook_resolve_id_trigger = "automatic"))]
pub async fn resolve_id(
  bundle_options: &SharedOptions,
  resolver: &SharedResolver,
  plugin_driver: &SharedPluginDriver,
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

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
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
  let jobs = dependencies.iter_enumerated().map(|(idx, item)| {
    let specifier = item.module_request.clone();
    let bundle_options = Arc::clone(options);
    // FIXME(hyf0): should not use `Arc<Resolver>` here
    let resolver = Arc::clone(resolver);
    let plugin_driver = Arc::clone(plugin_driver);
    let importer = &self_resolved_id.id;
    let kind = item.kind;
    async move {
      // TODO: We should early return when `async closure is stable`
      resolve_id(&bundle_options, &resolver, &plugin_driver, importer, &specifier, kind)
        .await
        .map(|id| (specifier, idx, id))
    }
  });

  let resolved_ids = join_all(jobs).await;
  // FIXME: if the import records came from css view, but source from ecma view,
  // the span will not matched.
  let is_css_module = matches!(module_type, ModuleType::Css);
  let mut ret = IndexVec::with_capacity(dependencies.len());
  let mut build_errors = vec![];
  for resolved_id in resolved_ids {
    let (specifier, idx, resolved_id) = resolved_id?;

    match resolved_id {
      Ok(info) => {
        ret.push(info);
      }
      Err(e) => {
        let dep = &dependencies[idx];
        match &e {
          ResolveError::NotFound(..) => {
            // NOTE: IN_TRY_CATCH_BLOCK meta if it is a `require` import
            // record
            if !dep.meta.contains(ImportRecordMeta::IN_TRY_CATCH_BLOCK) {
              // https://github.com/rollup/rollup/blob/49b57c2b30d55178a7316f23cc9ccc457e1a2ee7/src/ModuleLoader.ts#L643-L646
              if ecmascript::is_path_like_specifier(&specifier) {
                // Unlike rollup, we also emit errors for absolute path
                build_errors.push(BuildDiagnostic::resolve_error(
                  source.clone(),
                  self_resolved_id.id.clone(),
                  if dep.is_unspanned() || is_css_module {
                    DiagnosableArcstr::String(concat_string!("'", specifier.as_str(), "'").into())
                  } else {
                    DiagnosableArcstr::Span(dep.state.span)
                  },
                  "Module not found.".into(),
                  EventKind::UnresolvedImport,
                  None,
                ));
              } else {
                warnings.push(
                  BuildDiagnostic::resolve_error(
                    source.clone(),
                    self_resolved_id.id.clone(),
                    if dep.is_unspanned() || is_css_module {
                      DiagnosableArcstr::String(concat_string!("'", specifier.as_str(), "'").into())
                    } else {
                      DiagnosableArcstr::Span(dep.state.span)
                    },
                    "Module not found, treating it as an external dependency".into(),
                    EventKind::UnresolvedImport,
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
                Some("May be you expected `resolve.alias` to call other plugins resolveId hook? see the docs https://rolldown.rs/reference/config-options#resolve-alias for more details".to_string()),
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
