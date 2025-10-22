use std::{
  path::PathBuf,
  sync::{Arc, Weak},
};

use arcstr::ArcStr;
use derive_more::Debug;
use rolldown_common::{
  LogWithoutPlugin, ModuleDefFormat, ResolvedId, side_effects::HookSideEffects,
};
use rolldown_error::SingleBuildResult;

use crate::{
  PluginContextResolveOptions, plugin_context::PluginContextMeta,
  types::hook_resolve_id_skipped::HookResolveIdSkipped,
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

  #[must_use]
  pub fn new_shared_with_skipped_resolve_calls(
    &self,
    skipped_resolve_calls: Vec<Arc<HookResolveIdSkipped>>,
  ) -> Self {
    match self {
      PluginContext::Napi(_) => self.clone(),
      PluginContext::Native(ctx) => Self::Native(Arc::new(NativePluginContextImpl {
        plugin_name: ctx.plugin_name.clone(),
        skipped_resolve_calls,
        plugin_idx: ctx.plugin_idx,
        plugin_driver: Weak::clone(&ctx.plugin_driver),
        meta: Arc::clone(&ctx.meta),
        resolver: Arc::clone(&ctx.resolver),
        file_emitter: Arc::clone(&ctx.file_emitter),
        options: Arc::clone(&ctx.options),
        watch_files: Arc::clone(&ctx.watch_files),
        modules: Arc::clone(&ctx.modules),
        tx: Arc::clone(&ctx.tx),
      })),
    }
  }

  pub async fn load(
    &self,
    specifier: &str,
    side_effects: Option<HookSideEffects>,
    module_def_format: ModuleDefFormat,
  ) -> SingleBuildResult<()> {
    call_native_only!(self, "load", ctx => ctx.load(specifier, side_effects, module_def_format).await)
  }

  pub async fn resolve(
    &self,
    specifier: &str,
    importer: Option<&str>,
    extra_options: Option<PluginContextResolveOptions>,
  ) -> SingleBuildResult<Result<ResolvedId, rolldown_resolver::ResolveError>> {
    call_native_only!(self, "resolve", ctx => ctx.resolve(specifier, importer, extra_options).await)
  }

  pub async fn emit_chunk(&self, chunk: rolldown_common::EmittedChunk) -> anyhow::Result<ArcStr> {
    call_native_only!(self, "emit_chunk", ctx => ctx.emit_chunk(chunk).await)
  }

  pub fn emit_file(
    &self,
    file: rolldown_common::EmittedAsset,
    fn_asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  ) -> ArcStr {
    call_native_only!(self, "emit_file", ctx => ctx.emit_file(file, fn_asset_filename, fn_sanitized_file_name))
  }

  pub async fn emit_file_async(
    &self,
    file: rolldown_common::EmittedAsset,
  ) -> SingleBuildResult<ArcStr> {
    call_native_only!(self, "emit_file_async", ctx => ctx.emit_file_async(file).await)
  }

  pub fn get_file_name(&self, reference_id: &str) -> anyhow::Result<ArcStr> {
    call_native_only!(self, "get_file_name", ctx => ctx.get_file_name(reference_id))
  }

  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<rolldown_common::ModuleInfo>> {
    call_native_only!(self, "get_module_info", ctx => ctx.get_module_info(module_id))
  }

  pub fn get_module_ids(&self) -> Vec<ArcStr> {
    call_native_only!(self, "get_module_ids", ctx => ctx.get_module_ids())
  }

  pub fn cwd(&self) -> &PathBuf {
    call_native_only!(self, "cwd", ctx => ctx.cwd())
  }

  /// Add a file as a dependency.
  ///
  /// * file - The file to add as a watch dependency. This should be a normalized absolute path.
  pub fn add_watch_file(&self, file: &str) {
    call_native_only!(self, "add_watch_file", ctx => ctx.add_watch_file(file));
  }

  pub fn meta(&self) -> &PluginContextMeta {
    call_native_only!(self, "meta", ctx => &ctx.meta)
  }

  pub fn options(&self) -> &rolldown_common::NormalizedBundlerOptions {
    call_native_only!(self, "options", ctx => &ctx.options)
  }

  pub fn resolver(&self) -> &rolldown_resolver::Resolver {
    call_native_only!(self, "resolver", ctx => &ctx.resolver)
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
