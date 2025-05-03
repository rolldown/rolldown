use std::{
  path::PathBuf,
  sync::{Arc, Weak},
};

use anyhow::Context;
use arcstr::ArcStr;
use derive_more::Debug;
use rolldown_common::{
  ModuleDefFormat, ModuleInfo, ModuleLoaderMsg, ResolvedId, SharedFileEmitter,
  SharedNormalizedBundlerOptions, side_effects::HookSideEffects,
};
use rolldown_resolver::{ResolveError, Resolver};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use tokio::sync::Mutex;

use crate::{
  PluginDriver,
  types::{
    hook_resolve_id_skipped::HookResolveIdSkipped,
    plugin_context_resolve_options::PluginContextResolveOptions, plugin_idx::PluginIdx,
  },
  utils::resolve_id_check_external::resolve_id_check_external,
};

#[derive(Debug)]
pub struct PluginContextImpl {
  pub(crate) skipped_resolve_calls: Vec<Arc<HookResolveIdSkipped>>,
  pub(crate) plugin_idx: PluginIdx,
  pub(crate) resolver: Arc<Resolver>,
  pub(crate) plugin_driver: Weak<PluginDriver>,
  pub(crate) file_emitter: SharedFileEmitter,
  pub(crate) options: SharedNormalizedBundlerOptions,
  pub(crate) watch_files: Arc<FxDashSet<ArcStr>>,
  pub(crate) modules: Arc<FxDashMap<ArcStr, Arc<ModuleInfo>>>,
  pub(crate) tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>>>,
}

impl PluginContextImpl {
  pub async fn load(
    &self,
    specifier: &str,
    side_effects: Option<HookSideEffects>,
  ) -> anyhow::Result<()> {
    // Clone out the sender under the lock, then drop the lock before awaiting.
    let sender = {
      let guard = self.tx.lock().await.clone();
      guard.context("The `PluginContext.load` only work at `resolveId/load/transform/moduleParsed` hooks. If you using it at resolveId hook, please make sure it could not load the entry module.")?
    };
    sender
      .send(ModuleLoaderMsg::FetchModule(ResolvedId {
        id: specifier.into(),
        ignored: false,
        module_def_format: ModuleDefFormat::Unknown,
        external: false.into(),
        normalize_external_id: None,
        package_json: None,
        side_effects,
        is_external_without_side_effects: false,
      }))
      .await?;
    Ok(())
  }

  #[tracing::instrument(skip_all, fields(CONTEXT_hook_resolve_id_trigger = "manual"))]
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

    resolve_id_check_external(
      &self.resolver,
      &plugin_driver,
      specifier,
      importer,
      false,
      normalized_extra_options.import_kind,
      skipped_resolve_calls,
      normalized_extra_options.custom,
      false,
      &self.options,
    )
    .await
  }

  pub async fn emit_chunk(&self, chunk: rolldown_common::EmittedChunk) -> anyhow::Result<ArcStr> {
    self.file_emitter.emit_chunk(Arc::new(chunk)).await
  }

  pub fn emit_file(
    &self,
    file: rolldown_common::EmittedAsset,
    fn_asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  ) -> ArcStr {
    let file_name_is_none = file.file_name.is_none();
    let asset_filename_template =
      file_name_is_none.then(|| self.options.asset_filenames.value(fn_asset_filename).into());
    let sanitized_file_name = file_name_is_none.then(|| {
      self.options.sanitize_filename.value(file.name_for_sanitize(), fn_sanitized_file_name)
    });

    self.file_emitter.emit_file(file, asset_filename_template, sanitized_file_name)
  }

  pub async fn emit_file_async(
    &self,
    file: rolldown_common::EmittedAsset,
  ) -> anyhow::Result<ArcStr> {
    let asset_filename = self.options.asset_filename_with_file(&file).await?;
    let sanitized_file_name = self.options.sanitize_file_name_with_file(&file).await?;
    Ok(self.file_emitter.emit_file(file, asset_filename.map(Into::into), sanitized_file_name))
  }

  pub fn get_file_name(&self, reference_id: &str) -> anyhow::Result<ArcStr> {
    self.file_emitter.get_file_name(reference_id)
  }

  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<rolldown_common::ModuleInfo>> {
    self.modules.get(module_id).map(|v| Arc::<rolldown_common::ModuleInfo>::clone(v.value()))
  }

  pub fn get_module_ids(&self) -> Vec<String> {
    self.modules.iter().map(|v| v.key().to_string()).collect()
  }

  pub fn cwd(&self) -> &PathBuf {
    self.resolver.cwd()
  }

  pub fn add_watch_file(&self, file: &str) {
    self.watch_files.insert(file.into());
  }
}
