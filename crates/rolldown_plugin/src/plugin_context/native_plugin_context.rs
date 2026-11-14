use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  sync::{Arc, Mutex, Weak},
};

use anyhow::Context;
use arcstr::ArcStr;
use derive_more::Debug;
use rolldown_common::{
  FilenameTemplate, ImportAttributes, LogLevel, LogWithoutPlugin, ModuleDefFormat, ModuleLoaderMsg,
  PackageJson, PluginIdx, ResolvedId, SharedFileEmitter, SharedModuleInfoDashMap,
  SharedNormalizedBundlerOptions, SharedSavedJsModuleOptions, WatchPath,
  side_effects::HookSideEffects,
};
use rolldown_resolver::{ResolveError, Resolver};
use rolldown_utils::dashmap::FxDashSet;
use tracing::Instrument;

use crate::{
  PluginDriver,
  plugin_context::PluginContextMeta,
  types::{
    hook_resolve_id_skipped::HookResolveIdSkipped,
    module_cache_recorder::{RecordedAction, SharedModuleCacheRecorder},
    module_meta_store::SharedModuleMetaStore,
    plugin_context_resolve_options::PluginContextResolveOptions,
  },
  utils::resolve_id_check_external::resolve_id_check_external,
};

pub type SharedNativePluginContext = Arc<NativePluginContextImpl>;

#[derive(Debug)]
pub struct NativePluginContextImpl {
  pub(crate) plugin_name: Cow<'static, str>,
  pub(crate) skipped_resolve_calls: Vec<Arc<HookResolveIdSkipped>>,
  pub(crate) plugin_idx: PluginIdx,
  pub(crate) resolver: Arc<Resolver>,
  pub(crate) meta: Arc<PluginContextMeta>,
  pub(crate) plugin_driver: Weak<PluginDriver>,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) options: SharedNormalizedBundlerOptions,
  pub(crate) watch_files: Arc<FxDashSet<ArcStr>>,
  pub(crate) module_infos: SharedModuleInfoDashMap,
  pub(crate) tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>>>>,
  pub(crate) session: rolldown_devtools::Session,
  pub(crate) bundle_span: tracing::Span,
  // `resolve_id` hook not only will be triggered by the rolldown's resolve process, but also could be triggered
  // by manual calls of `PluginContext.resolve()`. We use a dedicated span here to distinguish whether the call is
  // - automatic (by rolldown) or manual (by `PluginContext.resolve()`)
  pub(crate) manual_resolve_span: tracing::Span,
  /// Set on the contexts that the plugin driver forks for the hook calls of a module task
  /// while the persistent cache records that task.
  pub(crate) cache_recorder: Option<SharedModuleCacheRecorder>,
  /// `None` without the persistent cache.
  pub(crate) saved_js_module_options: Option<SharedSavedJsModuleOptions>,
  pub(crate) module_meta: SharedModuleMetaStore,
  /// The number of this build in `module_meta`.
  pub(crate) module_meta_build: u64,
}

impl NativePluginContextImpl {
  pub async fn load(
    &self,
    raw_id: &str,
    attributes: ImportAttributes,
    side_effects: Option<HookSideEffects>,
    module_def_format: ModuleDefFormat,
  ) -> anyhow::Result<()> {
    // The loaded module is an input that the persistent cache does not track.
    self.mark_not_cacheable(|| format!("plugin `{}` called `this.load`", self.plugin_name));
    // Clone out the sender under the lock, then drop the lock before awaiting.
    let sender = {
      let guard = self.tx.lock().ok().context("Failed to acquire PluginDriver tx lock")?.clone();
      guard.context("The `PluginContext.load` only work at `resolveId/load/transform/moduleParsed` hooks. If you using it at resolveId hook, please make sure it could not load the entry module.")?
    };
    let resolved_id = ResolvedId {
      id: raw_id.into(),
      side_effects,
      module_def_format,
      attributes,
      ..Default::default()
    }
    .finalize_attributes(self.options.experimental.is_import_attributes_enabled());
    let id = resolved_id.id.as_arc_str().clone();
    sender
      .send(ModuleLoaderMsg::FetchModule(Box::new(resolved_id)))
      .map_err(|e| anyhow::Error::new(e).context("PluginContext: failed to send FetchModule message - module loader shut down during plugin execution"))?;
    let plugin_driver = self
      .plugin_driver
      .upgrade()
      .ok_or_else(|| anyhow::anyhow!("Plugin driver is already dropped."))?;
    plugin_driver.wait_for_module_load_completion(&id).await;
    Ok(())
  }

  pub fn try_get_package_json_or_create(&self, path: &Path) -> anyhow::Result<Arc<PackageJson>> {
    self.resolver.try_get_package_json_or_create(path)
  }

  pub async fn resolve(
    &self,
    specifier: &str,
    importer: Option<&str>,
    extra_options: Option<PluginContextResolveOptions>,
  ) -> anyhow::Result<Result<ResolvedId, ResolveError>> {
    let plugin_driver = self
      .plugin_driver
      .upgrade()
      .ok_or_else(|| anyhow::anyhow!("Plugin driver is already dropped."))?;

    let normalized_extra_options = extra_options.unwrap_or_default();
    if normalized_extra_options.skip_self
      && self.skipped_resolve_calls.iter().any(|call| {
        call.plugin_idx == self.plugin_idx
          && call.specifier == specifier
          && call.importer.as_deref() == importer
      })
    {
      // This means that the plugin recursively called itself
      return Ok(Err(ResolveError::NotFound(specifier.to_string())));
    }

    let skipped_resolve_calls = if normalized_extra_options.skip_self {
      let mut skipped_resolve_calls = Vec::with_capacity(self.skipped_resolve_calls.len() + 1);
      skipped_resolve_calls.extend(self.skipped_resolve_calls.clone());
      skipped_resolve_calls.push(Arc::new(HookResolveIdSkipped {
        plugin_idx: self.plugin_idx,
        importer: importer.map(Into::into),
        specifier: specifier.into(),
      }));
      Some(skipped_resolve_calls)
    } else if !self.skipped_resolve_calls.is_empty() {
      Some(self.skipped_resolve_calls.clone())
    } else {
      None
    };

    // A forwarded `importer` carries the importer's attributes in its id.
    let (importer_raw_id, importer_attributes) =
      match importer.and_then(ImportAttributes::parse_module_id) {
        Some((raw_id, attributes)) => (Some(raw_id), attributes),
        None => (importer, ImportAttributes::default()),
      };

    // Use the pre-created manual resolve span.
    // When PluginContext.resolve() is called from JavaScript via NAPI, the tracing span
    // context is lost across the async boundary. By making the resolve span a child of
    // the bundle span (which is a child of the session span), we ensure that both
    // CONTEXT_session_id and CONTEXT_bundle_id are inherited automatically.
    // The plugin contexts are recreated at the start of each write/generate with the new bundle span.
    async {
      resolve_id_check_external(
        &self.resolver,
        &plugin_driver,
        specifier,
        importer,
        normalized_extra_options.is_entry,
        normalized_extra_options.import_kind,
        skipped_resolve_calls,
        normalized_extra_options.custom,
        false,
        &self.options,
        &normalized_extra_options.attributes,
        importer_raw_id,
        &importer_attributes,
        self.cache_recorder.as_ref(),
      )
      .await
    }
    .instrument(self.manual_resolve_span.clone())
    .await
  }

  pub fn mark_not_cacheable(&self, reason: impl FnOnce() -> String) {
    if let Some(recorder) = &self.cache_recorder {
      recorder.mark_not_cacheable(reason);
    }
  }

  /// Records a log that a JavaScript plugin sent to `onLog` during this hook call.
  pub fn record_js_log(&self, level: LogLevel, json: String) {
    self.record(|| RecordedAction::JsLog { level, json });
  }

  /// Whether the persistent cache records this hook call, and the module task can still get a
  /// saved result.
  pub fn is_cache_recording(&self) -> bool {
    self.cache_recorder.as_ref().is_some_and(|recorder| recorder.is_cacheable())
  }

  /// The module of the task that this hook call belongs to, when the persistent cache records
  /// the call. In a nested `this.resolve` call, it is not the importer of the call.
  pub fn cache_module_id(&self) -> Option<&ArcStr> {
    self.cache_recorder.as_ref().map(|recorder| recorder.module_id())
  }

  /// Records one top-level key of the `meta` that a JavaScript hook returned for the module
  /// `module_id` during this hook call. `json` is `None` for the value `undefined`.
  pub fn record_js_module_meta(&self, module_id: ArcStr, key: String, json: Option<String>) {
    self.record(|| RecordedAction::JsModuleMeta { module_id, key, json });
  }

  /// Records the `moduleSideEffects` that a JavaScript hook returned for the module `module_id`
  /// during this hook call.
  pub fn record_js_module_side_effects(&self, module_id: ArcStr, value: HookSideEffects) {
    self.record(|| RecordedAction::JsModuleSideEffects { module_id, value });
  }

  /// See `ModuleCacheRecorder::ignore_next_hook_side_effects`.
  pub fn ignore_next_hook_side_effects(&self) {
    if let Some(recorder) = &self.cache_recorder {
      recorder.ignore_next_hook_side_effects();
    }
  }

  /// The JavaScript-side values of the modules whose saved result the persistent cache reused,
  /// or `None` when there are none.
  pub fn saved_js_module_options(&self) -> Option<&SharedSavedJsModuleOptions> {
    self.saved_js_module_options.as_ref().filter(|saved| !saved.is_empty())
  }

  pub fn set_module_meta(&self, module_id: &str, key: &str, value: Arc<[u8]>) {
    self.record(|| RecordedAction::RustModuleMeta {
      module_id: module_id.into(),
      key: key.into(),
      value: Arc::clone(&value),
    });
    self.module_meta.set(module_id, key, value, self.module_meta_build);
  }

  pub fn get_module_meta(&self, module_id: &str, key: &str) -> Option<Arc<[u8]>> {
    self.module_meta.get(module_id, key)
  }

  pub fn has_module_meta_in_this_build(&self, module_id: &str, key: &str) -> bool {
    self.module_meta.has_key_in_build(module_id, key, self.module_meta_build)
  }

  pub fn module_ids_with_meta(&self, key: &str) -> Vec<ArcStr> {
    self.module_meta.module_ids_with_key(key, self.module_meta_build)
  }

  pub fn count_modules_with_meta(&self, key: &str) -> usize {
    self.module_meta.count_modules_with_key(key, self.module_meta_build)
  }

  fn record(&self, action: impl FnOnce() -> RecordedAction) {
    if let Some(recorder) = &self.cache_recorder {
      recorder.record(action());
    }
  }

  /// A copy of `value` when this call is recorded. Emits are recorded only after they succeed:
  /// a failed emit that the plugin catches must not fail the build when it is replayed.
  fn copy_for_record<T: Clone>(&self, value: &T) -> Option<T> {
    self.cache_recorder.as_ref().map(|_| value.clone())
  }

  pub fn emit_chunk(&self, chunk: rolldown_common::EmittedChunk) -> anyhow::Result<ArcStr> {
    let copy = self.copy_for_record(&chunk);
    let reference_id = self.file_emitter.emit_chunk(Arc::new(chunk))?;
    if let Some(chunk) = copy {
      self.record(|| RecordedAction::EmitChunk(chunk));
    }
    Ok(reference_id)
  }

  pub fn emit_prebuilt_chunk(&self, chunk: rolldown_common::EmittedPrebuiltChunk) -> ArcStr {
    self.record(|| RecordedAction::EmitPrebuiltChunk(chunk.clone()));
    self.file_emitter.emit_prebuilt_chunk(chunk)
  }

  pub fn emit_file(
    &self,
    file: rolldown_common::EmittedAsset,
    fn_asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  ) -> anyhow::Result<ArcStr> {
    let copy = self.copy_for_record(&(
      file.clone(),
      fn_asset_filename.clone(),
      fn_sanitized_file_name.clone(),
    ));
    let reference_id = self.emit_file_inner(file, fn_asset_filename, fn_sanitized_file_name)?;
    if let Some((file, fn_asset_filename, fn_sanitized_file_name)) = copy {
      self.record(|| RecordedAction::EmitFile { file, fn_asset_filename, fn_sanitized_file_name });
    }
    Ok(reference_id)
  }

  fn emit_file_inner(
    &self,
    file: rolldown_common::EmittedAsset,
    fn_asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  ) -> anyhow::Result<ArcStr> {
    let file_name_is_none = file.file_name.is_none();
    let asset_filename_template = file_name_is_none.then(|| {
      FilenameTemplate::new(
        self.options.asset_filenames.value(fn_asset_filename),
        "output.assetFileNames",
      )
    });
    let sanitized_file_name = file_name_is_none.then(|| {
      self.options.sanitize_filename.value(file.name_for_sanitize(), fn_sanitized_file_name)
    });

    self.file_emitter.emit_file(file, asset_filename_template, sanitized_file_name)
  }

  pub async fn emit_file_async(
    &self,
    file: rolldown_common::EmittedAsset,
  ) -> anyhow::Result<ArcStr> {
    let copy = self.copy_for_record(&file);
    let asset_filename = self.options.asset_filename_with_file(&file).await?;
    let sanitized_file_name = self.options.sanitize_file_name_with_file(&file).await?;
    let reference_id = self.file_emitter.emit_file(file, asset_filename, sanitized_file_name)?;
    if let Some(file) = copy {
      self.record(|| RecordedAction::EmitFileAsync(file));
    }
    Ok(reference_id)
  }

  pub fn get_file_name(&self, reference_id: &str) -> anyhow::Result<ArcStr> {
    self.file_emitter.get_file_name(reference_id)
  }

  pub fn associate_module_with_file_ref(&self, module_id: &str, reference_id: &str) {
    self.record(|| RecordedAction::AssociateModuleWithFileRef {
      module_id: module_id.into(),
      reference_id: reference_id.into(),
    });
    self.file_emitter.associate_module_with_file_ref(module_id, reference_id);
  }

  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<rolldown_common::ModuleInfo>> {
    self.module_infos.get(module_id).map(|v| Arc::<rolldown_common::ModuleInfo>::clone(v.value()))
  }

  pub fn get_module_ids(&self) -> Vec<ArcStr> {
    self.module_infos.iter().map(|v| v.key().clone()).collect()
  }

  pub fn cwd(&self) -> &PathBuf {
    self.resolver.cwd()
  }

  /// The path is resolved against `cwd`, so the watch files are absolute and normalized.
  pub fn add_watch_file(&self, file: &str) {
    let path: ArcStr = WatchPath::new(file, &self.options.cwd).to_string().into();
    self.record(|| RecordedAction::WatchFile(path.clone()));
    self.watch_files.insert(path);
  }

  fn log(&self, level: LogLevel, log: LogWithoutPlugin) {
    if let Some(on_log) = &self.options.on_log {
      let log = log.into_log(Some(self.plugin_name.to_string()));
      self.record(|| RecordedAction::Log { level, log: log.clone() });
      // Call now so the log is enqueued before any later work; only its result is awaited.
      let result = on_log.call(level, log);
      rolldown_utils::futures::spawn(async move {
        // FIXME: should collect error happened here and cause the build to fail later
        let _ = result.await;
      });
    }
  }

  #[inline]
  pub fn info(&self, log: LogWithoutPlugin) {
    self.log(LogLevel::Info, log);
  }

  #[inline]
  pub fn warn(&self, log: LogWithoutPlugin) {
    self.log(LogLevel::Warn, log);
  }

  #[inline]
  pub fn debug(&self, log: LogWithoutPlugin) {
    self.log(LogLevel::Debug, log);
  }
}
