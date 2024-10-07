use arcstr::ArcStr;
use futures::future::join_all;
use oxc::index::IndexVec;
use oxc::minifier::ReplaceGlobalDefinesConfig;
use oxc::span::Span;
use rolldown_common::{
  side_effects::HookSideEffects, ImportKind, ImportRecordIdx, ModuleDefFormat, ModuleIdx,
  ModuleType, RawImportRecord, ResolvedId, StrOrBytes,
};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_plugin::{SharedPluginDriver, __inner::resolve_id_check_external};
use rolldown_resolver::ResolveError;
use rolldown_sourcemap::SourceMap;
use std::sync::Arc;

use crate::{runtime::RUNTIME_MODULE_ID, SharedOptions, SharedResolver};

pub struct CreateModuleContext<'a> {
  pub module_index: ModuleIdx,
  pub plugin_driver: &'a SharedPluginDriver,
  pub resolved_id: &'a ResolvedId,
  pub options: &'a SharedOptions,
  pub module_type: ModuleType,
  pub warnings: &'a mut Vec<BuildDiagnostic>,
  pub resolver: &'a SharedResolver,
  pub replace_global_define_config: Option<ReplaceGlobalDefinesConfig>,
}

impl<'a> CreateModuleContext<'a> {
  pub(crate) async fn resolve_id(
    bundle_options: &SharedOptions,
    resolver: &SharedResolver,
    plugin_driver: &SharedPluginDriver,
    importer: &str,
    specifier: &str,
    kind: ImportKind,
  ) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
    // Check runtime module
    if specifier == RUNTIME_MODULE_ID {
      return Ok(Ok(ResolvedId {
        id: specifier.to_string().into(),
        ignored: false,
        module_def_format: ModuleDefFormat::EsmMjs,
        is_external: false,
        package_json: None,
        side_effects: None,
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

  pub async fn resolve_dependencies(
    &mut self,
    dependencies: &IndexVec<ImportRecordIdx, RawImportRecord>,
    source: ArcStr,
  ) -> anyhow::Result<DiagnosableResult<IndexVec<ImportRecordIdx, ResolvedId>>> {
    let jobs = dependencies.iter_enumerated().map(|(idx, item)| {
      let specifier = item.module_request.clone();
      let bundle_options = Arc::clone(self.options);
      // FIXME(hyf0): should not use `Arc<Resolver>` here
      let resolver = Arc::clone(self.resolver);
      let plugin_driver = Arc::clone(self.plugin_driver);
      let importer = &self.resolved_id.id;
      let kind = item.kind;
      async move {
        Self::resolve_id(&bundle_options, &resolver, &plugin_driver, importer, &specifier, kind)
          .await
          .map(|id| (specifier, idx, id))
      }
    });

    let resolved_ids = join_all(jobs).await;

    let mut ret = IndexVec::with_capacity(dependencies.len());
    let mut build_errors = vec![];
    for resolved_id in resolved_ids {
      let (specifier, idx, resolved_id) = resolved_id?;

      match resolved_id {
        Ok(info) => {
          ret.push(info);
        }
        Err(e) => match &e {
          ResolveError::NotFound(..) => {
            self.warnings.push(
              BuildDiagnostic::unresolved_import_treated_as_external(
                specifier.to_string(),
                self.resolved_id.id.to_string(),
                Some(e),
              )
              .with_severity_warning(),
            );
            ret.push(ResolvedId {
              id: specifier.to_string().into(),
              ignored: false,
              module_def_format: ModuleDefFormat::Unknown,
              is_external: true,
              package_json: None,
              side_effects: None,
            });
          }
          e => {
            let reason = rolldown_resolver::error::oxc_resolve_error_to_reason(e);
            let dep = &dependencies[idx];
            build_errors.push(BuildDiagnostic::diagnosable_resolve_error(
              source.clone(),
              self.resolved_id.id.clone(),
              Span::new(dep.module_request_start, dep.module_request_end()),
              reason,
            ));
          }
        },
      }
    }

    if build_errors.is_empty() {
      Ok(Ok(ret))
    } else {
      Ok(Err(build_errors))
    }
  }
}

pub struct CreateModuleViewArgs {
  pub source: StrOrBytes,
  pub sourcemap_chain: Vec<SourceMap>,
  pub hook_side_effects: Option<HookSideEffects>,
}
