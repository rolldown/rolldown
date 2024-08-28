use std::sync::Arc;

use futures::future::join_all;
use oxc::index::IndexVec;
use oxc::minifier::ReplaceGlobalDefinesConfig;
use rolldown_common::{
  side_effects::HookSideEffects, ImportKind, ImportRecordIdx, Module, ModuleDefFormat, ModuleIdx,
  ModuleType, RawImportRecord, ResolvedId, StrOrBytes,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_plugin::SharedPluginDriver;
use rolldown_resolver::ResolveError;
use rolldown_sourcemap::SourceMap;

use crate::{runtime::RUNTIME_MODULE_ID, utils::resolve_id, SharedOptions, SharedResolver};

use super::ast_symbols::AstSymbols;

pub struct CreateModuleContext<'a> {
  pub module_index: ModuleIdx,
  pub plugin_driver: &'a SharedPluginDriver,
  pub resolved_id: &'a ResolvedId,
  pub options: &'a SharedOptions,
  pub module_type: ModuleType,
  pub warnings: &'a mut Vec<BuildDiagnostic>,
  pub resolver: &'a SharedResolver,
  pub is_user_defined_entry: bool,
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
    // Check external with unresolved path
    if let Some(is_external) = bundle_options.external.as_ref() {
      if is_external(specifier, Some(importer), false).await? {
        return Ok(Ok(ResolvedId {
          id: specifier.to_string().into(),
          ignored: false,
          module_def_format: ModuleDefFormat::Unknown,
          is_external: true,
          package_json: None,
          side_effects: None,
        }));
      }
    }

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

    let resolved_id = resolve_id::resolve_id(
      resolver,
      plugin_driver,
      specifier,
      Some(importer),
      false,
      kind,
      None,
      Arc::default(),
    )
    .await?;

    match resolved_id {
      Ok(mut resolved_id) => {
        if !resolved_id.is_external {
          // Check external with resolved path
          if let Some(is_external) = bundle_options.external.as_ref() {
            resolved_id.is_external = is_external(specifier, Some(importer), true).await?;
          }
        }
        Ok(Ok(resolved_id))
      }
      Err(e) => Ok(Err(e)),
    }
  }

  pub async fn resolve_dependencies(
    &mut self,
    dependencies: &IndexVec<ImportRecordIdx, RawImportRecord>,
  ) -> anyhow::Result<DiagnosableResult<IndexVec<ImportRecordIdx, ResolvedId>>> {
    let jobs = dependencies.iter().map(|item| {
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
          .map(|id| (specifier, id))
      }
    });

    let resolved_ids = join_all(jobs).await;

    let mut ret = IndexVec::with_capacity(dependencies.len());
    let mut build_errors = vec![];
    for resolved_id in resolved_ids {
      let (specifier, resolved_id) = resolved_id?;

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
          _ => {
            build_errors.push(BuildDiagnostic::unresolved_import(
              specifier.to_string(),
              self.resolved_id.id.to_string(),
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

pub struct CreateModuleArgs {
  pub source: StrOrBytes,
  pub sourcemap_chain: Vec<SourceMap>,
  pub hook_side_effects: Option<HookSideEffects>,
}

pub struct CreateModuleReturn {
  pub module: Module,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub ecma_related: Option<(EcmaAst, AstSymbols)>,
}

pub trait ModuleFactory {
  async fn create_module(
    ctx: &mut CreateModuleContext,
    args: CreateModuleArgs,
  ) -> anyhow::Result<DiagnosableResult<CreateModuleReturn>>;
}
