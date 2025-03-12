use arcstr::ArcStr;
use futures::future::join_all;
use oxc::span::Span;
use oxc_index::IndexVec;
use rolldown_plugin::{__inner::resolve_id_check_external, SharedPluginDriver};
use rolldown_resolver::ResolveError;
use rolldown_rstr::Rstr;
use rolldown_std_utils::PathExt;
use rolldown_utils::{
  concat_string,
  ecmascript::{self, legitimize_identifier_name},
  indexmap::FxIndexSet,
};
use std::sync::Arc;
use sugar_path::SugarPath;

use rolldown_common::{
  ImportKind, ImportRecordIdx, ImportRecordMeta, ModuleDefFormat, ModuleId, ModuleIdx, ModuleInfo,
  ModuleLoaderMsg, ModuleType, NormalModule, NormalModuleTaskResult, RUNTIME_MODULE_ID,
  RawImportRecord, ResolvedId, StrOrBytes,
};
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosableArcstr, EventKind, UnloadableDependencyContext,
};

use super::task_context::TaskContext;
use crate::{
  SharedOptions, SharedResolver,
  asset::create_asset_view,
  css::create_css_view,
  ecmascript::ecma_module_view_factory::{CreateEcmaViewReturn, create_ecma_view},
  types::module_factory::{CreateModuleContext, CreateModuleViewArgs},
  utils::{load_source::load_source, transform_source::transform_source},
};

pub struct ModuleTaskOwner {
  source: ArcStr,
  importer_id: Rstr,
  importee_span: Span,
}

impl ModuleTaskOwner {
  pub fn new(source: ArcStr, importer_id: Rstr, importee_span: Span) -> Self {
    ModuleTaskOwner { source, importer_id, importee_span }
  }
}

pub struct ModuleTask {
  ctx: Arc<TaskContext>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  owner: Option<ModuleTaskOwner>,
  is_user_defined_entry: bool,
  /// The module is asserted to be this specific module type.
  asserted_module_type: Option<ModuleType>,
}

impl ModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
  ) -> Self {
    Self {
      ctx,
      module_idx: idx,
      resolved_id,
      owner,
      is_user_defined_entry,
      asserted_module_type: assert_module_type,
    }
  }

  #[tracing::instrument(name="NormalModuleTask::run", level = "trace", skip_all, fields(module_id = ?self.resolved_id.id))]
  pub async fn run(mut self) {
    if let Err(errs) = self.run_inner().await {
      self
        .ctx
        .tx
        .send(ModuleLoaderMsg::BuildErrors(errs.into_vec()))
        .await
        .expect("Send should not fail");
    }
  }

  #[expect(clippy::too_many_lines)]
  async fn run_inner(&mut self) -> BuildResult<()> {
    let id = ModuleId::new(&self.resolved_id.id);

    // Add watch files for watcher recover if build errors occurred.
    self.ctx.plugin_driver.watch_files.insert(self.resolved_id.id.clone());

    self.ctx.plugin_driver.set_module_info(
      &id,
      Arc::new(ModuleInfo {
        code: None,
        id: id.clone(),
        is_entry: self.is_user_defined_entry,
        importers: FxIndexSet::default(),
        dynamic_importers: FxIndexSet::default(),
        imported_ids: FxIndexSet::default(),
        dynamically_imported_ids: FxIndexSet::default(),
        exports: vec![],
      }),
    );

    let mut sourcemap_chain = vec![];
    let mut hook_side_effects = self.resolved_id.side_effects.take();

    let (mut source, module_type) =
      self.load_source_phase(&mut sourcemap_chain, &mut hook_side_effects).await?;

    let stable_id = id.stabilize(&self.ctx.options.cwd);
    let mut raw_import_records = IndexVec::default();

    let (asset_view, css_view) = match module_type {
      ModuleType::Asset => {
        let asset_source = source.into_bytes();
        let asset_view = create_asset_view(asset_source.into());
        source = StrOrBytes::Str(String::new());
        (Some(asset_view), None)
      }
      ModuleType::Css => {
        let css_source: ArcStr = source.try_into_string()?.into();
        // FIXME: This makes creating `EcmaView` rely on creating `CssView` first, while they should be done in parallel.
        let (css_view, css_raw_import_records) = create_css_view(&stable_id, &css_source);
        raw_import_records = css_raw_import_records;
        source = StrOrBytes::Str(String::new());
        (None, Some(css_view))
      }
      _ => (None, None),
    };

    let mut warnings = vec![];

    let ret = create_ecma_view(
      &mut CreateModuleContext {
        stable_id: &stable_id,
        module_index: self.module_idx,
        plugin_driver: &self.ctx.plugin_driver,
        resolved_id: &self.resolved_id,
        options: &self.ctx.options,
        warnings: &mut warnings,
        module_type: module_type.clone(),
        replace_global_define_config: self.ctx.meta.replace_global_define_config.clone(),
        is_user_defined_entry: self.is_user_defined_entry,
      },
      CreateModuleViewArgs { source, sourcemap_chain, hook_side_effects },
    )
    .await?;

    let CreateEcmaViewReturn {
      mut ecma_view,
      ecma_related,
      raw_import_records: ecma_raw_import_records,
    } = ret;

    if css_view.is_none() {
      raw_import_records = ecma_raw_import_records;
    }

    let resolved_deps = self
      .resolve_dependencies(
        &raw_import_records,
        ecma_view.source.clone(),
        &mut warnings,
        &module_type,
      )
      .await?;

    if css_view.is_none() {
      for (record, info) in raw_import_records.iter().zip(&resolved_deps) {
        match record.kind {
          ImportKind::Import | ImportKind::Require | ImportKind::NewUrl => {
            ecma_view.imported_ids.insert(ArcStr::clone(&info.id).into());
          }
          ImportKind::DynamicImport => {
            ecma_view.dynamically_imported_ids.insert(ArcStr::clone(&info.id).into());
          }
          // for a none css module, we should not have `at-import` or `url-import`
          ImportKind::AtImport | ImportKind::UrlImport | ImportKind::HotAccept => unreachable!(),
        }
      }
    }

    let repr_name = self.resolved_id.id.as_path().representative_file_name();
    let repr_name = legitimize_identifier_name(&repr_name).into_owned();

    let module = NormalModule {
      repr_name,
      stable_id,
      id,
      debug_id: self.resolved_id.debug_id(&self.ctx.options.cwd),
      idx: self.module_idx,
      exec_order: u32::MAX,
      is_user_defined_entry: self.is_user_defined_entry,
      module_type: module_type.clone(),
      ecma_view,
      css_view,
      asset_view,
    };

    let module_info = Arc::new(module.to_module_info(Some(&raw_import_records)));
    self.ctx.plugin_driver.set_module_info(&module.id, Arc::clone(&module_info));
    self.ctx.plugin_driver.module_parsed(Arc::clone(&module_info), &module).await?;
    self.ctx.plugin_driver.mark_context_load_modules_loaded(&module.id).await?;

    let result = ModuleLoaderMsg::NormalModuleDone(NormalModuleTaskResult {
      module: module.into(),
      ecma_related: Some(ecma_related),
      resolved_deps,
      raw_import_records,
      warnings,
    });

    // If the main thread is dead, nothing we can do to handle these send failures.
    let _ = self.ctx.tx.send(result).await;

    Ok(())
  }

  async fn load_source_without_cache(
    &self,
    sourcemap_chain: &mut Vec<rolldown_sourcemap::SourceMap>,
    hook_side_effects: &mut Option<rolldown_common::side_effects::HookSideEffects>,
  ) -> BuildResult<(StrOrBytes, ModuleType)> {
    let result = load_source(
      &self.ctx.plugin_driver,
      &self.resolved_id,
      &self.ctx.fs,
      sourcemap_chain,
      hook_side_effects,
      &self.ctx.options,
      self.asserted_module_type.as_ref(),
    )
    .await;
    let (source, mut module_type) = result.map_err(|err| {
      BuildDiagnostic::unloadable_dependency(
        self.resolved_id.debug_id(self.ctx.options.cwd.as_path()).into(),
        self.owner.as_ref().map(|owner| UnloadableDependencyContext {
          importer_id: owner.importer_id.as_str().into(),
          importee_span: owner.importee_span,
          source: owner.source.clone(),
        }),
        err,
      )
    })?;
    if let Some(asserted) = &self.asserted_module_type {
      module_type = asserted.clone();
    }
    let source = match source {
      StrOrBytes::Str(source) => {
        // Run plugin transform.
        let source = transform_source(
          &self.ctx.plugin_driver,
          &self.resolved_id,
          source,
          sourcemap_chain,
          hook_side_effects,
          &mut module_type,
        )
        .await?;
        source.into()
      }
      StrOrBytes::Bytes(_) => source,
    };
    if let ModuleType::Custom(_) = module_type {
      // TODO: should provide some diagnostics for user how they should handle the module type.
      // e.g.
      // sass -> recommended npm install `sass` etc
      Err(anyhow::anyhow!(
        "`{:?}` is not specified module type,  rolldown can't handle this asset correctly. Please use the load/transform hook to transform the resource",
        self.resolved_id.id
      ))?;
    };
    Ok((source, module_type))
  }

  // TODO: cache source_map_chain and hook_side_effects
  fn load_source_with_cache(&self) -> Option<(StrOrBytes, ModuleType)> {
    self
      .ctx
      .cache
      .get_raw_source_and_module_type(&self.resolved_id.id)
      .map(|item| item.value().clone())
  }

  async fn load_source_phase(
    &self,
    sourcemap_chain: &mut Vec<rolldown_sourcemap::SourceMap>,
    hook_side_effects: &mut Option<rolldown_common::side_effects::HookSideEffects>,
  ) -> BuildResult<(StrOrBytes, ModuleType)> {
    let incremental_build_enabled = self.ctx.options.experimental.is_incremental_build_enabled();
    if incremental_build_enabled {
      if let Some(value) = self.load_source_with_cache() {
        return Ok(value);
      }
    }
    let value = self.load_source_without_cache(sourcemap_chain, hook_side_effects).await?;
    if incremental_build_enabled {
      self.ctx.cache.insert_raw_source_and_module_type(self.resolved_id.id.clone(), value.clone());
    }
    Ok(value)
  }

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
        id: specifier.into(),
        ignored: false,
        module_def_format: ModuleDefFormat::EsmMjs,
        external: false.into(),
        normalize_external_id: None,
        package_json: None,
        side_effects: None,
        is_external_without_side_effects: false,
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

  #[allow(clippy::too_many_lines)]
  pub async fn resolve_dependencies(
    &self,
    dependencies: &IndexVec<ImportRecordIdx, RawImportRecord>,
    source: ArcStr,
    warnings: &mut Vec<BuildDiagnostic>,
    module_type: &ModuleType,
  ) -> BuildResult<IndexVec<ImportRecordIdx, ResolvedId>> {
    let jobs = dependencies.iter_enumerated().map(|(idx, item)| {
      let specifier = item.module_request.clone();
      let bundle_options = Arc::clone(&self.ctx.options);
      // FIXME(hyf0): should not use `Arc<Resolver>` here
      let resolver = Arc::clone(&self.ctx.resolver);
      let plugin_driver = Arc::clone(&self.ctx.plugin_driver);
      let importer = &self.resolved_id.id;
      let kind = item.kind;
      async move {
        // TODO: We should early return when `async closure is stable`
        // Can't use `module_request.is_empty()` to check, see https://github.com/rolldown/rolldown/actions/runs/12980744296/job/36198187669?pr=3428
        if item.meta.contains(ImportRecordMeta::IS_DUMMY) {
          return Ok((item.module_request.clone(), idx, Ok(ResolvedId::make_dummy())));
        }
        Self::resolve_id(&bundle_options, &resolver, &plugin_driver, importer, &specifier, kind)
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
                    self.resolved_id.id.clone(),
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
                      self.resolved_id.id.clone(),
                      if dep.is_unspanned() || is_css_module {
                        DiagnosableArcstr::String(
                          concat_string!("'", specifier.as_str(), "'").into(),
                        )
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
                ignored: false,
                module_def_format: ModuleDefFormat::Unknown,
                external: true.into(),
                normalize_external_id: None,
                package_json: None,
                side_effects: None,
                is_external_without_side_effects: false,
              });
            }
            ResolveError::MatchedAliasNotFound(..) => {
              build_errors.push(BuildDiagnostic::resolve_error(
                source.clone(),
                self.resolved_id.id.clone(),
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
                self.resolved_id.id.clone(),
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
          };
        }
      }
    }

    if build_errors.is_empty() { Ok(ret) } else { Err(build_errors.into()) }
  }
}
