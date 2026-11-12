use std::sync::Arc;

use arcstr::ArcStr;
use oxc::span::Span;

use rolldown_common::{
  ExportsKind, FlatOptions, ImportKind, ModuleIdx, ModuleInfo, ModuleLoaderMsg, ModuleType,
  NormalModule, NormalModuleTaskResult, ResolvedId, SourceMapGenMsg, SourcemapChainElement,
  StrOrBytes, WatchPath, try_extract_lazy_barrel_info,
};
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosticOptions, EventKindSwitcher,
  ExternalModuleConflictImporter, UnloadableDependencyContext, downcast_napi_error_diagnostics,
};
use rolldown_plugin::{
  ModuleCacheRecorder, RecordedAction, RecordedEffects, SharedModuleCacheRecorder,
};
use rolldown_utils::{ecmascript::legitimize_identifier_name, indexmap::FxIndexSet};

use rolldown_fs::FileSystem;

use crate::{
  ecmascript::ecma_module_view_factory::{CreateEcmaViewReturn, create_ecma_view},
  persistent_cache::{
    ModuleKey, ScanCacheSession,
    saved_result::{SavedResolve, SavedResult},
  },
  types::module_factory::{CreateModuleContext, CreateModuleViewArgs},
  utils::{load_source::load_source, transform_source::transform_source},
};

use super::{
  resolve_utils::{DependencyCache, resolve_dependencies, restore_resolved_ids},
  task_context::{TaskContext, TaskMessageGuard},
};

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

  pub fn to_external_conflict_importer(&self) -> ExternalModuleConflictImporter {
    ExternalModuleConflictImporter {
      source: self.source.clone(),
      importer_id: self.importer_id.clone(),
      importee_span: self.importee_span,
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
  magic_string_tx: Option<std::sync::mpsc::Sender<SourceMapGenMsg>>,
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
    magic_string_tx: Option<std::sync::mpsc::Sender<SourceMapGenMsg>>,
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

  #[tracing::instrument(name="NormalModuleTask::run", level = "trace", skip_all, fields(module_id = %self.resolved_id.id))]
  pub async fn run(mut self) {
    let guard = TaskMessageGuard::new(&self.ctx, self.resolved_id.id.clone());
    if let Err(errs) = self.run_inner().await {
      self.ctx.plugin_driver.mark_context_load_modules_loaded(self.resolved_id.id.clone());
      // The send fails only when the build was dropped before this task finished.
      // Then nothing reads the result, so ignore the error.
      let _ = self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(errs.into_vec().into_boxed_slice()));
    }
    guard.mark_sent();
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

    let cache = self.ctx.persistent_cache.clone();
    let cache_key = match &cache {
      Some(cache) => {
        cache
          .module_key(&self.ctx.fs, &self.resolved_id.id, self.asserted_module_type.as_ref())
          .await
      }
      None => None,
    };
    let (cache_key, preloaded) = match cache_key {
      Some(ModuleKey { key, contents }) => (Some(key), contents),
      None => (None, None),
    };
    let saved_result = match (&cache, cache_key) {
      (Some(cache), Some(key)) if cache.reads_saved_results() => {
        cache.lookup(&self.ctx.fs, key).await.and_then(|mut saved| {
          // A saved result is reused as a whole or not at all.
          let resolved =
            restore_resolved_ids(std::mem::take(&mut saved.resolved), &self.ctx.resolver)?;
          Some((saved, resolved))
        })
      }
      _ => None,
    };
    let cache_recorder = (cache_key.is_some() && saved_result.is_none())
      .then(|| Arc::new(ModuleCacheRecorder::default()));

    let mut sourcemap_chain = vec![];
    let mut hook_side_effects = self.resolved_id.side_effects.take();
    let mut saved_resolves = None;
    let (source, module_type, is_read_from_disk) = if let Some((saved, resolved)) = saved_result {
      // The saved result replaces `load`, `transform` and `resolveId`.
      self.ctx.plugin_driver.replay_cache_actions(self.module_idx, saved.actions).await?;
      if saved.hook_side_effects.is_some() {
        hook_side_effects = saved.hook_side_effects;
      }
      sourcemap_chain = saved.sourcemap_chain;
      saved_resolves = Some(resolved);
      self.watch_module_files(saved.is_read_from_disk);
      (saved.source, saved.module_type, saved.is_read_from_disk)
    } else {
      self
        .load_source(
          &mut sourcemap_chain,
          &mut hook_side_effects,
          self.magic_string_tx.clone(),
          cache_recorder.as_ref(),
          preloaded,
        )
        .await?
    };

    // A copy of the state after `transform`, to save once the module task succeeds.
    let state_to_save = cache_recorder
      .as_ref()
      .map(|_| (source.clone(), module_type.clone(), sourcemap_chain.clone(), is_read_from_disk));

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

    let dependency_cache = match (&saved_resolves, &cache_recorder) {
      (Some(saved), _) => DependencyCache::Reuse(saved),
      (None, Some(recorder)) => DependencyCache::Record(recorder),
      (None, None) => DependencyCache::Off,
    };
    let (resolved_deps, resolves_to_save) = resolve_dependencies(
      &self.resolved_id,
      &self.ctx.options,
      &self.ctx.resolver,
      &self.ctx.plugin_driver,
      &raw_import_records,
      ecma_view.source.clone(),
      &mut warnings,
      dependency_cache,
    )
    .await?;

    if let (Some(cache), Some(key), Some(recorder), Some(state)) =
      (&cache, cache_key, &cache_recorder, state_to_save)
    {
      if recorder.is_cacheable() {
        self.save_to_cache(cache, key, recorder, state, resolves_to_save).await;
      }
    }

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

    let repr_name = self.resolved_id.id.representative_name();
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
                message: event.to_stderr_string(),
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

    // The send fails only when the build was dropped before this task finished.
    // Then nothing reads the result, so ignore the error.
    let _ = self.ctx.tx.send(result);

    Ok(())
  }

  /// Watches the module file when Rolldown read it from disk, and the tsconfig that governs the
  /// module. The tsconfig is also a transform dependency: it affects `resolveId` and the
  /// `transform` hooks, which a restart with the persistent cache skips.
  fn watch_module_files(&self, is_read_from_disk: bool) {
    let raw_id = self.resolved_id.id.raw_id();
    if is_read_from_disk {
      self.ctx.plugin_driver.watch_files.insert(raw_id.into());
    }
    if let Some(tsconfig_path) = self.governing_tsconfig() {
      let tsconfig_path: ArcStr = tsconfig_path.to_string_lossy().as_ref().into();
      self.ctx.plugin_driver.watch_files.insert(tsconfig_path.clone());
      self.ctx.plugin_driver.add_transform_dependency(
        self.module_idx,
        WatchPath::new(tsconfig_path.as_str(), &self.ctx.options.cwd),
      );
    }
  }

  fn governing_tsconfig(&self) -> Option<std::path::PathBuf> {
    let path = self.resolved_id.id.as_path()?;
    self.ctx.options.transform_options.discover_tsconfig_file(path)
  }

  async fn save_to_cache(
    &self,
    cache: &ScanCacheSession,
    key: u128,
    recorder: &ModuleCacheRecorder,
    (source, module_type, sourcemap_chain, is_read_from_disk): (
      StrOrBytes,
      ModuleType,
      Vec<SourcemapChainElement>,
      bool,
    ),
    resolved: Vec<SavedResolve>,
  ) {
    let RecordedEffects { actions, hook_side_effects } = recorder.take_effects();
    let mut dependencies: Vec<String> = actions
      .iter()
      .filter_map(|action| match action {
        RecordedAction::TransformDependency(path) => Some(path.to_string()),
        _ => None,
      })
      .collect();
    if let Some(tsconfig_path) = self.governing_tsconfig() {
      dependencies.push(tsconfig_path.to_string_lossy().into_owned());
    }
    let saved = SavedResult {
      source,
      module_type,
      hook_side_effects,
      sourcemap_chain,
      is_read_from_disk,
      resolved,
      actions,
    };
    cache.save(&self.ctx.fs, key, dependencies, &saved).await;
  }

  #[tracing::instrument(level = "debug", skip_all)]
  async fn load_source(
    &self,
    sourcemap_chain: &mut Vec<SourcemapChainElement>,
    hook_side_effects: &mut Option<rolldown_common::side_effects::HookSideEffects>,
    magic_string_tx: Option<std::sync::mpsc::Sender<SourceMapGenMsg>>,
    cache_recorder: Option<&SharedModuleCacheRecorder>,
    preloaded: Option<Vec<u8>>,
  ) -> BuildResult<(StrOrBytes, ModuleType, bool)> {
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
      cache_recorder,
      preloaded,
    )
    .await;
    // Add watch files as early as possible, so that a rebuild can recover from build errors.
    self.watch_module_files(is_read_from_disk);
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
          cache_recorder,
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
        "`{}` is not specified module type,  rolldown can't handle this asset correctly. Please use the load/transform hook to transform the resource",
        self.resolved_id.id
      ))?;
    }
    Ok((source, module_type, is_read_from_disk))
  }
}
