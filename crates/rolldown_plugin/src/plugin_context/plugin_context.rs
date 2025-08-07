use std::{
  path::PathBuf,
  sync::{Arc, Weak},
};

use arcstr::ArcStr;
use derive_more::Debug;
use rolldown_common::{LogWithoutPlugin, ResolvedId, side_effects::HookSideEffects};

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
  ) -> anyhow::Result<()> {
    match self {
      PluginContext::Napi(_) => unimplemented!("Can't call `load` on PluginContext::Napi"),
      PluginContext::Native(ctx) => ctx.load(specifier, side_effects).await,
    }
  }

  pub async fn resolve(
    &self,
    specifier: &str,
    importer: Option<&str>,
    extra_options: Option<PluginContextResolveOptions>,
  ) -> anyhow::Result<Result<ResolvedId, rolldown_resolver::ResolveError>> {
    match self {
      PluginContext::Napi(_) => unimplemented!("Can't call `resolve` on PluginContext::Napi"),
      PluginContext::Native(ctx) => ctx.resolve(specifier, importer, extra_options).await,
    }
  }

  pub async fn emit_chunk(&self, chunk: rolldown_common::EmittedChunk) -> anyhow::Result<ArcStr> {
    match self {
      PluginContext::Napi(_) => unimplemented!("Can't call `emit_chunk` on PluginContext::Napi"),
      PluginContext::Native(ctx) => ctx.emit_chunk(chunk).await,
    }
  }

  pub fn emit_file(
    &self,
    file: rolldown_common::EmittedAsset,
    fn_asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  ) -> ArcStr {
    match self {
      PluginContext::Napi(_) => unimplemented!("Can't call `emit_file` on PluginContext::Napi"),
      PluginContext::Native(ctx) => ctx.emit_file(file, fn_asset_filename, fn_sanitized_file_name),
    }
  }

  pub async fn emit_file_async(
    &self,
    file: rolldown_common::EmittedAsset,
  ) -> anyhow::Result<ArcStr> {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `emit_file_async` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => ctx.emit_file_async(file).await,
    }
  }

  pub fn get_file_name(&self, reference_id: &str) -> anyhow::Result<ArcStr> {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `get_file_name` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => ctx.get_file_name(reference_id),
    }
  }

  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<rolldown_common::ModuleInfo>> {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `get_module_info` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => ctx.get_module_info(module_id),
    }
  }

  pub fn get_module_ids(&self) -> Vec<String> {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `get_module_ids` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => ctx.get_module_ids(),
    }
  }

  pub fn cwd(&self) -> &PathBuf {
    match self {
      PluginContext::Napi(_) => unimplemented!("Can't call `cwd` on PluginContext::Napi"),
      PluginContext::Native(ctx) => ctx.cwd(),
    }
  }

  pub fn add_watch_file(&self, file: &str) {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `add_watch_file` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => ctx.add_watch_file(file),
    }
  }

  pub fn meta(&self) -> &PluginContextMeta {
    match self {
      PluginContext::Napi(_) => unimplemented!("Can't call `meta` on PluginContext::Napi"),
      PluginContext::Native(ctx) => &ctx.meta,
    }
  }

  pub fn options(&self) -> &rolldown_common::NormalizedBundlerOptions {
    match self {
      PluginContext::Napi(_) => unimplemented!("Can't call `options` on PluginContext::Napi"),
      PluginContext::Native(ctx) => &ctx.options,
    }
  }

  pub fn resolver(&self) -> &rolldown_resolver::Resolver {
    match self {
      PluginContext::Napi(_) => unimplemented!("Can't call `resolver` on PluginContext::Napi"),
      PluginContext::Native(ctx) => &ctx.resolver,
    }
  }

  pub fn file_emitter(&self) -> &rolldown_common::SharedFileEmitter {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `file_emitter` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => &ctx.file_emitter,
    }
  }

  #[inline]
  pub fn info(&self, log: LogWithoutPlugin) {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `info` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => ctx.info(log),
    }
  }

  #[inline]
  pub fn warn(&self, log: LogWithoutPlugin) {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `warn` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => ctx.warn(log),
    }
  }

  #[inline]
  pub fn debug(&self, log: LogWithoutPlugin) {
    match self {
      PluginContext::Napi(_) => {
        unimplemented!("Can't call `debug` on PluginContext::Napi")
      }
      PluginContext::Native(ctx) => ctx.debug(log),
    }
  }
}
