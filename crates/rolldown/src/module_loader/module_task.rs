use std::sync::Arc;

use arcstr::ArcStr;
use oxc::span::Span;
use sugar_path::SugarPath as _;

use rolldown_common::{
  ExportsKind, FlatOptions, ImportKind, ModuleIdx, ModuleInfo, ModuleLoaderMsg, ModuleType,
  NormalModule, NormalModuleTaskResult, ResolvedId, SourceMapGenMsg, SourcemapChainElement,
  StrOrBytes, try_extract_lazy_barrel_info,
};
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosticOptions, EventKindSwitcher, UnloadableDependencyContext,
  downcast_napi_error_diagnostics,
};
use rolldown_std_utils::PathExt as _;
use rolldown_utils::{ecmascript::legitimize_identifier_name, indexmap::FxIndexSet};

use rolldown_fs::FileSystem;

use crate::{
  ecmascript::ecma_module_view_factory::{CreateEcmaViewReturn, create_ecma_view},
  types::module_factory::{CreateModuleContext, CreateModuleViewArgs},
  utils::{load_source::load_source, transform_source::transform_source},
};

use super::{resolve_utils::resolve_dependencies, task_context::TaskContext};

pub struct ModuleTaskOwner {
  source: ArcStr,
  importer_id: ArcStr,
  importee_span: Span,
}

impl ModuleTaskOwner {
  pub fn new(normal_module: &NormalModule, importee_span: Span) -> Self {
    Self {
      source: normal_module.source.clone(),
      importer_id: normal_module.stable_id.as_arc_str().clone(),
      importee_span,
    }
  }
}

pub struct ModuleTask<Fs: FileSystem + Clone + 'static> {
  ctx: Arc<TaskContext<Fs>>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  owner: Option<ModuleTaskOwner>,
  is_user_defined_entry: bool,
  /// The module is asserted to be this specific module type.
  asserted_module_type: Option<ModuleType>,
  flat_options: FlatOptions,
  magic_string_tx: Option<std::sync::Arc<std::sync::mpsc::Sender<SourceMapGenMsg>>>,
}

impl<Fs: FileSystem + Clone + 'static> ModuleTask<Fs> {
  #[expect(clippy::too_many_arguments)]
  pub fn new(
    ctx: Arc<TaskContext<Fs>>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
    flat_options: FlatOptions,
    magic_string_tx: Option<std::sync::Arc<std::sync::mpsc::Sender<SourceMapGenMsg>>>,
  ) -> Self {
    Self {
      ctx,
      module_idx: idx,
      resolved_id,
      owner,
      is_user_defined_entry,
      asserted_module_type: assert_module_type,
      flat_options,
      magic_string_tx,
    }
  }

  #[tracing::instrument(name="NormalModuleTask::run", level = "trace", skip_all, fields(module_id = ?self.resolved_id.id))]
  pub async fn run(mut self) {
    if let Err(errs) = self.run_inner().await {
      self.ctx.plugin_driver.mark_context_load_modules_loaded(self.resolved_id.id.clone());
      self
        .ctx
        .tx
        .send(ModuleLoaderMsg::BuildErrors(errs.into_vec().into_boxed_slice()))
        .expect("ModuleLoader: failed to send build errors - main thread terminated while processing module errors");
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    let id = self.resolved_id.id.clone();

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
        input_format: ExportsKind::None,
      }),
    );

    let mut sourcemap_chain = vec![];
    let mut hook_side_effects = self.resolved_id.side_effects.take();
    let (source, module_type) = self
      .load_source(&mut sourcemap_chain, &mut hook_side_effects, self.magic_string_tx.clone())
      .await?;

    let stable_id = id.stabilize(&self.ctx.options.cwd);

    if matches!(module_type, ModuleType::Css) {
      Err(BuildDiagnostic::unsupported_feature(
        match &source { StrOrBytes::Bytes(_) => ArcStr::new(), StrOrBytes::Str(s) => s.into() },
        id.as_arc_str().clone(),
        Span::empty(0),
        "Bundling CSS is no longer supported (experimental support has been removed). See https://github.com/rolldown/rolldown/issues/4271 for details.".to_string())
      )?;
    }

    let mut warnings = vec![];

    let CreateEcmaViewReturn { mut ecma_view, ecma_related, raw_import_records, tla_keyword_span } =
      create_ecma_view(
        &mut CreateModuleContext {
          stable_id: &stable_id,
          module_idx: self.module_idx,
          plugin_driver: &self.ctx.plugin_driver,
          resolved_id: &self.resolved_id,
          options: &self.ctx.options,
          warnings: &mut warnings,
          module_type: module_type.clone(),
          replace_global_define_config: self.ctx.meta.replace_global_define_config.clone(),
          is_user_defined_entry: self.is_user_defined_entry,
          flat_options: self.flat_options,
        },
        CreateModuleViewArgs { source, sourcemap_chain, hook_side_effects },
      )
      .await?;

    let resolved_deps = resolve_dependencies(
      &self.resolved_id,
      &self.ctx.options,
      &self.ctx.resolver,
      &self.ctx.plugin_driver,
      &raw_import_records,
      ecma_view.source.clone(),
      &mut warnings,
      &module_type,
    )
    .await?;

    for (record, info) in raw_import_records.iter().zip(&resolved_deps) {
      match record.kind {
        ImportKind::Import | ImportKind::Require | ImportKind::NewUrl => {
          ecma_view.imported_ids.insert(info.id.clone());
        }
        ImportKind::DynamicImport => {
          ecma_view.dynamically_imported_ids.insert(info.id.clone());
        }
        ImportKind::HotAccept => {
          ecma_view.hmr_info.deps.insert(info.id.clone());
        }
        // for a none css module, we should not have `at-import` or `url-import`
        ImportKind::AtImport | ImportKind::UrlImport => unreachable!(),
      }
    }

    let repr_name = self.resolved_id.id.as_path().representative_file_name();
    let repr_name = legitimize_identifier_name(&repr_name).into_owned();

    // Build lazy barrel info if the experimental flag is enabled
    let barrel_info = if self.flat_options.is_lazy_barrel_enabled() {
      try_extract_lazy_barrel_info(&ecma_view, &raw_import_records)
    } else {
      None
    };

    // Eagerly resolving every import in a giant barrel is a known bottleneck.
    // The threshold targets only the real outliers (large icon packs); normal
    // component and utility barrels stay well below it.
    if barrel_info.is_some()
      && self.ctx.options.checks.contains(EventKindSwitcher::LargeBarrelModules)
    {
      const LARGE_BARREL_IMPORT_THRESHOLD: usize = 5000;
      let import_record_count = raw_import_records.len();
      if import_record_count > LARGE_BARREL_IMPORT_THRESHOLD {
        if let Some(on_log) = self.ctx.options.on_log.as_ref() {
          let event = BuildDiagnostic::large_barrel_modules(id.to_string(), import_record_count)
            .with_severity(rolldown_error::Severity::Info)
            .to_diagnostic_with(&DiagnosticOptions { cwd: self.ctx.options.cwd.clone() });
          on_log
            .call(
              rolldown_common::LogLevel::Info,
              rolldown_common::Log {
                message: event.to_color_string(),
                id: Some(id.to_string()),
                code: Some(event.kind()),
                ..Default::default()
              },
            )
            .await?;
        }
      }
    }

    let module = NormalModule {
      repr_name,
      stable_id,
      id,
      debug_id: self.resolved_id.debug_id(&self.ctx.options.cwd),
      idx: self.module_idx,
      exec_order: u32::MAX,
      module_type: module_type.clone(),
      ecma_view,
      originative_resolved_id: self.resolved_id.clone(),
    };

    let module_info =
      Arc::new(module.to_module_info(Some(&raw_import_records), self.is_user_defined_entry));
    self.ctx.plugin_driver.set_module_info(&module.id, Arc::clone(&module_info));
    self.ctx.plugin_driver.module_parsed(Arc::clone(&module_info), &module).await?;
    self.ctx.plugin_driver.mark_context_load_modules_loaded(module.id.clone());

    let result = ModuleLoaderMsg::NormalModuleDone(Box::new(NormalModuleTaskResult {
      module: module.into(),
      ecma_related,
      resolved_deps,
      raw_import_records,
      warnings,
      barrel_info,
      tla_keyword_span,
    }));

    self.ctx.tx.send(result).expect(
      "ModuleLoader channel closed while sending module completion - main thread terminated unexpectedly"
    );

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  async fn load_source(
    &self,
    sourcemap_chain: &mut Vec<SourcemapChainElement>,
    hook_side_effects: &mut Option<rolldown_common::side_effects::HookSideEffects>,
    magic_string_tx: Option<std::sync::Arc<std::sync::mpsc::Sender<SourceMapGenMsg>>>,
  ) -> BuildResult<(StrOrBytes, ModuleType)> {
    let mut is_read_from_disk = true;
    let result = load_source(
      &self.ctx.plugin_driver,
      &self.resolved_id,
      self.ctx.fs.clone(),
      sourcemap_chain,
      hook_side_effects,
      &self.ctx.options,
      self.asserted_module_type.as_ref(),
      &mut is_read_from_disk,
      self.module_idx,
    )
    .await;
    if is_read_from_disk {
      // - Only add watch files for files read from disk.
      // - Add watch files as early as possible for we might be able to recover from build errors.
      self.ctx.plugin_driver.watch_files.insert(self.resolved_id.id.as_arc_str().clone());
    }
    let (source, mut module_type) = result.map_err(|err| {
      downcast_napi_error_diagnostics(err).unwrap_or_else(|e| {
        BuildDiagnostic::unloadable_dependency(
          self.resolved_id.debug_id(self.ctx.options.cwd.as_path()).into(),
          self.owner.as_ref().map(|owner| UnloadableDependencyContext {
            source: owner.source.clone(),
            importer_id: owner.importer_id.clone(),
            importee_span: owner.importee_span,
          }),
          e.to_string().into(),
        )
      })
    })?;
    let source = match source {
      _ if self.resolved_id.id.starts_with("rolldown:") => source,
      StrOrBytes::Str(source) => {
        // Run plugin transform.
        let source = transform_source(
          &self.ctx.plugin_driver,
          &self.resolved_id,
          self.module_idx,
          source,
          sourcemap_chain,
          hook_side_effects,
          &mut module_type,
          magic_string_tx,
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
    }
    Ok((source, module_type))
  }
}
