use std::{
  path::PathBuf,
  sync::{Arc, Weak},
};

use arcstr::ArcStr;
use derive_more::Debug;
use rolldown_common::{
  LogWithoutPlugin, ModuleDefFormat, ResolvedId,
  side_effects::{DeterminedSideEffects, HookSideEffects},
};

use crate::{
  PluginContextResolveOptions,
  plugin_context::PluginContextMeta,
  types::{
    hook_resolve_id_skipped::HookResolveIdSkipped,
    module_cache_recorder::{RecordedAction, SharedModuleCacheRecorder},
  },
};

use super::NativePluginContextImpl;

#[derive(Debug)]
pub struct NapiPluginContextImpl;

#[derive(Debug, Clone)]
pub enum PluginContext {
  Napi(Arc<NapiPluginContextImpl>),
  Native(Arc<NativePluginContextImpl>),
}

macro_rules! call_native_only {
  ($self:expr, $method_name:literal, $ctx:ident => $native_expr:expr) => {
    match $self {
      PluginContext::Napi(_) => {
        unimplemented!(concat!("Can't call `", $method_name, "` on PluginContext::Napi"))
      }
      PluginContext::Native($ctx) => $native_expr,
    }
  };
}

impl PluginContext {
  #[must_use]
  pub fn new_napi_context() -> Self {
    Self::Napi(Arc::new(NapiPluginContextImpl))
  }

  /// A context for one hook call. Returns a clone of `self` when there is nothing to fork.
  #[must_use]
  pub fn fork_for_call(
    &self,
    skipped_resolve_calls: Option<&Vec<Arc<HookResolveIdSkipped>>>,
    cache_recorder: Option<&SharedModuleCacheRecorder>,
  ) -> Self {
    match self {
      PluginContext::Napi(_) => self.clone(),
      PluginContext::Native(_) if skipped_resolve_calls.is_none() && cache_recorder.is_none() => {
        self.clone()
      }
      PluginContext::Native(ctx) => Self::Native(Arc::new(NativePluginContextImpl {
        plugin_name: ctx.plugin_name.clone(),
        skipped_resolve_calls: skipped_resolve_calls
          .map_or_else(|| ctx.skipped_resolve_calls.clone(), Clone::clone),
        plugin_idx: ctx.plugin_idx,
        plugin_driver: Weak::clone(&ctx.plugin_driver),
        meta: Arc::clone(&ctx.meta),
        resolver: Arc::clone(&ctx.resolver),
        file_emitter: Arc::clone(&ctx.file_emitter),
        options: Arc::clone(&ctx.options),
        watch_files: Arc::clone(&ctx.watch_files),
        module_infos: Arc::clone(&ctx.module_infos),
        module_side_effects_overrides: Arc::clone(&ctx.module_side_effects_overrides),
        tx: Arc::clone(&ctx.tx),
        session: ctx.session.clone(),
        log_hook_tasks: Arc::clone(&ctx.log_hook_tasks),
        bundle_span: ctx.bundle_span.clone(),
        manual_resolve_span: ctx.manual_resolve_span.clone(),
        cache_recorder: cache_recorder.cloned().or_else(|| ctx.cache_recorder.clone()),
        module_meta: Arc::clone(&ctx.module_meta),
        module_meta_build: ctx.module_meta_build,
      })),
    }
  }

  /// The recorder of the module task that this hook call belongs to, if the persistent cache
  /// records this call.
  pub fn cache_recorder(&self) -> Option<&SharedModuleCacheRecorder> {
    match self {
      PluginContext::Napi(_) => None,
      PluginContext::Native(ctx) => ctx.cache_recorder.as_ref(),
    }
  }

  /// The module task that this hook call belongs to gets no saved result.
  pub fn mark_not_cacheable(&self, reason: impl FnOnce() -> String) {
    if let Some(recorder) = self.cache_recorder() {
      recorder.mark_not_cacheable(reason);
    }
  }

  /// Records a log that a JavaScript plugin sent to `onLog` during this hook call.
  pub fn record_js_log(&self, level: rolldown_common::LogLevel, json: String) {
    if let Some(recorder) = self.cache_recorder() {
      recorder.record(RecordedAction::JsLog { level, json });
    }
  }

  pub async fn load(
    &self,
    raw_id: &str,
    attributes: rolldown_common::ImportAttributes,
    side_effects: Option<HookSideEffects>,
    module_def_format: ModuleDefFormat,
    resolve_dependencies: bool,
    module_options_receipt: Option<u32>,
  ) -> anyhow::Result<()> {
    call_native_only!(self, "load", ctx => ctx.load(raw_id, attributes, side_effects, module_def_format, resolve_dependencies, module_options_receipt).await)
  }

  pub async fn resolve(
    &self,
    specifier: &str,
    importer: Option<&str>,
    extra_options: Option<PluginContextResolveOptions>,
  ) -> anyhow::Result<Result<ResolvedId, rolldown_resolver::ResolveError>> {
    call_native_only!(self, "resolve", ctx => ctx.resolve(specifier, importer, extra_options).await)
  }

  pub fn emit_chunk(&self, chunk: rolldown_common::EmittedChunk) -> anyhow::Result<ArcStr> {
    call_native_only!(self, "emit_chunk", ctx => ctx.emit_chunk(chunk))
  }

  pub fn emit_file(
    &self,
    file: rolldown_common::EmittedAsset,
    fn_asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  ) -> anyhow::Result<ArcStr> {
    call_native_only!(self, "emit_file", ctx => ctx.emit_file(file, fn_asset_filename, fn_sanitized_file_name))
  }

  pub async fn emit_file_async(
    &self,
    file: rolldown_common::EmittedAsset,
  ) -> anyhow::Result<ArcStr> {
    call_native_only!(self, "emit_file_async", ctx => ctx.emit_file_async(file).await)
  }

  pub fn get_file_name(&self, reference_id: &str) -> anyhow::Result<ArcStr> {
    call_native_only!(self, "get_file_name", ctx => ctx.get_file_name(reference_id))
  }

  pub fn associate_module_with_file_ref(&self, module_id: &str, reference_id: &str) {
    call_native_only!(self, "associate_module_with_file_ref", ctx => ctx.associate_module_with_file_ref(module_id, reference_id));
  }

  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<rolldown_common::ModuleInfo>> {
    call_native_only!(self, "get_module_info", ctx => ctx.get_module_info(module_id))
  }

  /// `getModuleInfo(module_id)?.moduleSideEffects`. It runs a `treeshake.moduleSideEffects`
  /// function when the value needs it.
  pub async fn get_module_side_effects(
    &self,
    module_id: &str,
  ) -> anyhow::Result<Option<DeterminedSideEffects>> {
    call_native_only!(self, "get_module_side_effects", ctx => ctx.get_module_side_effects(module_id).await)
  }

  /// Like a JS write to `getModuleInfo(module_id).moduleSideEffects`. `None` is a write of `null`.
  /// A write after `buildEnd` is ignored, and the first one in a build logs a warning.
  pub fn set_module_side_effects(&self, module_id: &str, side_effects: Option<HookSideEffects>) {
    call_native_only!(self, "set_module_side_effects", ctx => ctx.set_module_side_effects(module_id, side_effects));
  }

  /// Like `set_module_side_effects`, but returns `true` for the first ignored write after
  /// `buildEnd` instead of warning, so the caller can warn.
  pub fn set_module_side_effects_without_warning(
    &self,
    module_id: &str,
    side_effects: Option<HookSideEffects>,
  ) -> bool {
    call_native_only!(self, "set_module_side_effects_without_warning", ctx => ctx.set_module_side_effects_without_warning(module_id, side_effects))
  }

  /// Sets the value under `key` in the meta that Rust plugins keep for the module `module_id`.
  /// The module can be the module of this hook call or another one. By convention, `key` is
  /// the name of the plugin. A new value replaces the value under the same key of that module.
  ///
  /// With the persistent cache, a call in a `resolve_id`, `resolve_dynamic_import`, `load` or
  /// `transform` hook is saved with the module whose task ran the hook, and is done again when
  /// that module reuses its saved result. A call in another hook is not saved.
  ///
  /// A context without a bundler has no module meta: the call does nothing.
  pub fn set_module_meta(&self, module_id: &str, key: &str, value: impl Into<Arc<[u8]>>) {
    match self {
      PluginContext::Napi(_) => {}
      PluginContext::Native(ctx) => ctx.set_module_meta(module_id, key, value.into()),
    }
  }

  /// The value that a Rust plugin set under `key` for the module `module_id`.
  ///
  /// The meta is not cleared between the builds of one bundler, so the value can come from an
  /// earlier build.
  pub fn get_module_meta(&self, module_id: &str, key: &str) -> Option<Arc<[u8]>> {
    match self {
      PluginContext::Napi(_) => None,
      PluginContext::Native(ctx) => ctx.get_module_meta(module_id, key),
    }
  }

  /// Whether the value under `key` for the module `module_id` was set in this build. A hook
  /// set it, or Rolldown set it again from a saved result. A value of an earlier build gives
  /// `false`.
  pub fn has_module_meta_in_this_build(&self, module_id: &str, key: &str) -> bool {
    match self {
      PluginContext::Napi(_) => false,
      PluginContext::Native(ctx) => ctx.has_module_meta_in_this_build(module_id, key),
    }
  }

  /// The modules whose value under `key` was set in this build, in no fixed order. A hook set
  /// it, or Rolldown set it again from a saved result. A module of an earlier build that this
  /// build did not set again is left out.
  pub fn module_ids_with_meta(&self, key: &str) -> Vec<ArcStr> {
    match self {
      PluginContext::Napi(_) => Vec::new(),
      PluginContext::Native(ctx) => ctx.module_ids_with_meta(key),
    }
  }

  /// The length of `module_ids_with_meta`, without a copy of the ids.
  pub fn count_modules_with_meta(&self, key: &str) -> usize {
    match self {
      PluginContext::Napi(_) => 0,
      PluginContext::Native(ctx) => ctx.count_modules_with_meta(key),
    }
  }

  pub fn cwd(&self) -> &PathBuf {
    call_native_only!(self, "cwd", ctx => ctx.cwd())
  }

  /// Add a file or a directory as a watch dependency.
  pub fn add_watch_file(&self, file: &str) {
    call_native_only!(self, "add_watch_file", ctx => ctx.add_watch_file(file));
  }

  pub fn meta(&self) -> &PluginContextMeta {
    call_native_only!(self, "meta", ctx => &ctx.meta)
  }

  pub fn options(&self) -> &rolldown_common::NormalizedBundlerOptions {
    call_native_only!(self, "options", ctx => &ctx.options)
  }

  pub fn file_emitter(&self) -> &rolldown_common::SharedFileEmitter {
    call_native_only!(self, "file_emitter", ctx => &ctx.file_emitter)
  }

  #[inline]
  pub fn info(&self, log: LogWithoutPlugin) {
    call_native_only!(self, "info", ctx => ctx.info(log));
  }

  #[inline]
  pub fn warn(&self, log: LogWithoutPlugin) {
    call_native_only!(self, "warn", ctx => ctx.warn(log));
  }

  #[inline]
  pub fn debug(&self, log: LogWithoutPlugin) {
    call_native_only!(self, "debug", ctx => ctx.debug(log));
  }
}
