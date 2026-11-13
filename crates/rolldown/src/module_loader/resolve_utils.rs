use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use futures::future::join_all;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ImportAttributes, ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleDefFormat, ModuleId,
  NormalizedBundlerOptions, RUNTIME_MODULE_KEY, RawImportRecord, ResolvedId,
};
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosableArcstr, DiagnosticOptions, EventKind,
};
use rolldown_fs::FileSystem;
use rolldown_plugin::{
  __inner::resolve_id_check_external, PluginDriver, SharedModuleCacheRecorder, SharedPluginDriver,
};
use rolldown_resolver::{ResolveError, Resolver};
use rolldown_utils::ecmascript;
use rustc_hash::FxHashMap;

use crate::{
  SharedOptions, SharedResolver,
  persistent_cache::saved_result::{SavedResolve, SavedResolvedId},
};

#[tracing::instrument(skip_all, fields(CONTEXT_hook_resolve_id_trigger = "automatic"))]
#[expect(clippy::too_many_arguments)]
pub async fn resolve_id<Fs: FileSystem>(
  bundle_options: &NormalizedBundlerOptions,
  resolver: &Resolver<Fs>,
  plugin_driver: &PluginDriver,
  importer: &ResolvedId,
  specifier: &str,
  kind: ImportKind,
  attributes: &ImportAttributes,
  cache_recorder: Option<&SharedModuleCacheRecorder>,
) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
  // Check runtime module
  if specifier == RUNTIME_MODULE_KEY {
    return Ok(Ok(ResolvedId {
      id: ModuleId::new(specifier),
      module_def_format: ModuleDefFormat::EsmMjs,
      ..Default::default()
    }));
  }

  resolve_id_check_external(
    resolver,
    plugin_driver,
    specifier,
    Some(&importer.id),
    false,
    kind,
    None,
    Arc::default(),
    false,
    bundle_options,
    attributes,
    Some(importer.id.raw_id()),
    &importer.attributes,
    cache_recorder,
  )
  .await
}

/// Saved resolve results, filled in again, by (specifier, kind, attributes). `None` stands for
/// `ResolveError::NotFound`.
pub type RestoredResolveMap = FxHashMap<(String, ImportKind, ImportAttributes), Option<ResolvedId>>;

/// How `resolve_dependencies` works with the persistent cache.
pub enum DependencyCache<'a> {
  Off,
  /// Runs the hooks and returns their results, so they can be saved.
  Record(&'a SharedModuleCacheRecorder),
  /// Takes the results from a saved result instead of running the hooks.
  Reuse(&'a RestoredResolveMap),
}

/// Fills in the fields that saved resolve results do not hold. `None` when a `package.json` that
/// one of them names cannot be read any more: then the whole saved result is not used.
pub fn restore_resolved_ids<Fs: FileSystem>(
  saved: Vec<SavedResolve>,
  resolver: &Resolver<Fs>,
) -> Option<RestoredResolveMap> {
  saved
    .into_iter()
    .map(|saved| {
      let resolved_id = match saved.resolved {
        Some(resolved) => Some(restore_resolved_id(resolved, resolver)?),
        None => None,
      };
      Some(((saved.specifier, saved.kind, saved.attributes), resolved_id))
    })
    .collect()
}

fn restore_resolved_id<Fs: FileSystem>(
  saved: SavedResolvedId,
  resolver: &Resolver<Fs>,
) -> Option<ResolvedId> {
  let package_json = match &saved.package_json_path {
    Some(path) => Some(resolver.try_get_package_json_or_create(Path::new(path)).ok()?),
    None => None,
  };
  Some(ResolvedId {
    module_def_format: saved.module_def_format.infer(saved.id.raw_id(), package_json.as_ref()),
    id: saved.id,
    attributes: saved.attributes,
    external: saved.external,
    normalize_external_id: saved.normalize_external_id,
    package_json,
    side_effects: saved.side_effects,
    is_external_without_side_effects: saved.is_external_without_side_effects,
  })
}

#[expect(clippy::too_many_arguments)]
pub async fn resolve_dependencies<Fs: FileSystem>(
  self_resolved_id: &ResolvedId,
  options: &SharedOptions,
  resolver: &SharedResolver<Fs>,
  plugin_driver: &SharedPluginDriver,
  dependencies: &IndexVec<ImportRecordIdx, RawImportRecord>,
  source: ArcStr,
  warnings: &mut Vec<BuildDiagnostic>,
  cache: DependencyCache<'_>,
) -> BuildResult<(IndexVec<ImportRecordIdx, ResolvedId>, Vec<SavedResolve>)> {
  // NOTE: this dedupes the identical (specifier, kind, attributes) resolve calls
  let dedup_map: FxHashMap<(&str, ImportKind, &ImportAttributes), ImportRecordIdx> = dependencies
    .iter_enumerated()
    .map(|(idx, item)| ((item.module_request.as_str(), item.kind, &item.attributes), idx))
    .collect();

  let cache = &cache;
  let jobs = dedup_map.values().map(|&idx| async move {
    let item = &dependencies[idx];
    let specifier = &item.module_request;
    if let DependencyCache::Reuse(saved) = cache {
      let key = (specifier.to_string(), item.kind, item.attributes.clone());
      match saved.get(&key) {
        Some(None) => return Ok((idx, Err(ResolveError::NotFound(specifier.to_string())))),
        Some(Some(resolved_id)) => return Ok((idx, Ok(resolved_id.clone()))),
        // The runtime module, which is resolved without hooks and never saved.
        None => {}
      }
    }
    let cache_recorder = match cache {
      DependencyCache::Record(recorder) => Some(*recorder),
      _ => None,
    };
    resolve_id(
      options,
      resolver,
      plugin_driver,
      self_resolved_id,
      specifier,
      item.kind,
      &item.attributes,
      cache_recorder,
    )
    .await
    .map(|id| (idx, id))
  });
  let mut sparse_results: IndexVec<ImportRecordIdx, Option<Result<ResolvedId, ResolveError>>> =
    index_vec![None; dependencies.len()];
  for result in join_all(jobs).await {
    let (idx, resolved) = result?;
    sparse_results[idx] = Some(resolved);
  }

  let mut saved_resolves = vec![];
  if let DependencyCache::Record(_) = cache {
    for &idx in dedup_map.values() {
      let item = &dependencies[idx];
      // The runtime module is resolved without hooks, and with a format that the saved fields
      // cannot give back.
      if item.module_request.as_str() == RUNTIME_MODULE_KEY {
        continue;
      }
      let resolved = match sparse_results[idx].as_ref() {
        Some(Ok(resolved_id)) => Some(SavedResolvedId::from_resolved_id(resolved_id)),
        Some(Err(ResolveError::NotFound(..))) => None,
        // Any other error fails the module task, so nothing is saved.
        _ => continue,
      };
      saved_resolves.push(SavedResolve {
        specifier: item.module_request.to_string(),
        kind: item.kind,
        attributes: item.attributes.clone(),
        resolved,
      });
    }
  }
  let resolved_results = dependencies.iter().map(|dep| {
    let repr_idx = dedup_map[&(dep.module_request.as_str(), dep.kind, &dep.attributes)];
    sparse_results[repr_idx].as_ref().expect("dedup representative should be resolved")
  });

  let mut ret = IndexVec::with_capacity(dependencies.len());
  let mut build_errors = vec![];
  for (dep, resolved_id) in dependencies.iter().zip(resolved_results) {
    match resolved_id {
      Ok(info) => {
        ret.push(info.clone());
      }
      Err(e) => {
        let specifier = &dep.module_request;
        match e {
          ResolveError::NotFound(..) => {
            // NOTE: IN_TRY_CATCH_BLOCK meta if it is a `require` import
            // record
            if !dep.meta.contains(ImportRecordMeta::InTryCatchBlock) {
              // https://github.com/rollup/rollup/blob/49b57c2b30d55178a7316f23cc9ccc457e1a2ee7/src/ModuleLoader.ts#L643-L646
              if ecmascript::is_path_like_specifier(specifier) {
                // Unlike rollup, we also emit errors for absolute path
                build_errors.push(BuildDiagnostic::resolve_error(
                  source.clone(),
                  self_resolved_id.id.as_arc_str().clone(),
                  diagnosable_importee(dep),
                  "Module not found.".into(),
                  EventKind::UnresolvedImport,
                  None,
                ));
              } else {
                let help = if specifier.starts_with("virtual:") {
                  Some(r#"The id starts with "virtual:", which by convention denotes a virtual module that is expected to be resolved by a plugin. Make sure the plugin that should provide it is registered and resolves the id in its `resolveId` hook."#.to_string())
                } else {
                  matches!(options.platform, rolldown_common::Platform::Neutral).then(|| {
                    r#"The "main" field here was ignored. Main fields must be configured explicitly when using the "neutral" platform."#.to_string()
                  })
                };
                warnings.push(
                  BuildDiagnostic::resolve_error(
                    source.clone(),
                    self_resolved_id.id.as_arc_str().clone(),
                    diagnosable_importee(dep),
                    "Module not found, treating it as an external dependency".into(),
                    EventKind::UnresolvedImport,
                    help,
                  )
                  .with_severity_warning(),
                );
              }
            }
            ret.push(
              ResolvedId {
                id: ModuleId::new(specifier.as_str()),
                external: true.into(),
                attributes: dep.attributes.clone(),
                ..Default::default()
              }
              .finalize_attributes(options.experimental.is_import_attributes_enabled()),
            );
          }
          ResolveError::MatchedAliasNotFound(..) => {
            build_errors.push(BuildDiagnostic::resolve_error(
                source.clone(),
                self_resolved_id.id.as_arc_str().clone(),
                diagnosable_importee(dep),
                format!("Matched alias not found for '{specifier}'"),
                    EventKind::ResolveError,
                Some("Maybe you expected `resolve.alias` to call other plugins resolveId hook? see the docs https://rolldown.rs/reference/InputOptions.resolve#alias for more details".to_string()),
              ));
          }
          e => {
            let diagnostic_opts = DiagnosticOptions { cwd: options.cwd.clone() };
            build_errors.push(BuildDiagnostic::resolve_error(
              source.clone(),
              self_resolved_id.id.as_arc_str().clone(),
              diagnosable_importee(dep),
              rolldown_error::resolve_error_to_message(e, &diagnostic_opts),
              EventKind::ResolveError,
              None,
            ));
          }
        }
      }
    }
  }

  if build_errors.is_empty() { Ok((ret, saved_resolves)) } else { Err(build_errors.into()) }
}

/// The message names `module_request`. The span only underlines the import in source.
fn diagnosable_importee(dep: &RawImportRecord) -> DiagnosableArcstr {
  let text = ArcStr::from(dep.module_request.as_str());
  if dep.is_unspanned() {
    DiagnosableArcstr::String(text)
  } else {
    DiagnosableArcstr::Span { span: dep.state.span, text }
  }
}
