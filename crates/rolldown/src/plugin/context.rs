use std::sync::Weak;

use rolldown_common::ImportKind;
use rolldown_fs::FileSystem;

use crate::{
  bundler::{
    options::input_options::SharedInputOptions, plugin_driver::PluginDriver,
    utils::resolve_id::resolve_id_without_defaults,
  },
  error::BatchedResult,
  HookResolveIdArgsOptions, SharedResolver,
};

#[derive(Debug)]
pub struct PluginContext<T: FileSystem + Default> {
  pub plugin_driver: Weak<PluginDriver<T>>,
  pub input_options: SharedInputOptions,
  pub resolver: SharedResolver<T>,
}

impl<T: FileSystem + Default + 'static> PluginContext<T> {
  pub fn new(
    plugin_driver: Weak<PluginDriver<T>>,
    input_options: SharedInputOptions,
    resolver: SharedResolver<T>,
  ) -> Self {
    Self { plugin_driver, input_options, resolver }
  }

  pub fn load(&self) {}

  pub async fn resolve(
    &self,
    source: String,
    importer: Option<String>,
  ) -> BatchedResult<ResolveId> {
    let plugin_driver = self.plugin_driver.upgrade().expect("should have plugin_driver");
    let result = resolve_id_without_defaults(
      &self.input_options,
      &self.resolver,
      &plugin_driver,
      importer.map(std::convert::Into::into),
      &source,
      HookResolveIdArgsOptions { is_entry: false, kind: ImportKind::Import },
    )
    .await?;
    Ok(ResolveId { external: result.is_external, id: result.path.to_string() })
  }
}

pub struct ResolveId {
  pub external: bool,
  pub id: String,
}

#[derive(Debug)]
pub struct TransformPluginContext<'a, T: FileSystem + Default> {
  pub inner: &'a PluginContext<T>,
}

impl<'a, T: FileSystem + Default + 'static> TransformPluginContext<'a, T> {
  pub fn new(inner: &'a PluginContext<T>) -> Self {
    Self { inner }
  }
}
