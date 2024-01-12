use std::sync::Arc;

use crate::{
  plugin::{
    args::HookBuildEndArgs,
    plugin::{BoxPlugin, HookNoopReturn},
  },
  HookLoadArgs, HookLoadReturn, HookResolveIdArgs, HookResolveIdReturn, HookTransformArgs,
  HookTransformReturn, PluginContext,
};

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: Vec<BoxPlugin>,
  base: Option<SharedPluginDriver>,
}

impl PluginDriver {
  pub fn new(plugins: Vec<BoxPlugin>) -> Self {
    Self { plugins, base: None }
  }

  pub fn create_output_plugin_driver(
    plugins: Vec<BoxPlugin>,
    base_plugin_driver: SharedPluginDriver,
  ) -> Self {
    Self { plugins, base: Some(base_plugin_driver) }
  }

  pub fn plugins(&self) -> Vec<&BoxPlugin> {
    self.base.as_ref().map_or_else(
      || self.plugins.iter().collect::<Vec<_>>(),
      |base| base.plugins().into_iter().chain(self.plugins.iter()).collect::<Vec<_>>(),
    )
  }

  pub async fn build_start(&self) -> HookNoopReturn {
    for plugin in self.plugins() {
      plugin.build_start(&mut PluginContext::new()).await?;
    }
    Ok(())
  }

  pub async fn resolve_id(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
    for plugin in self.plugins() {
      if let Some(r) = plugin.resolve_id(&mut PluginContext::new(), args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn load(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    for plugin in self.plugins() {
      if let Some(r) = plugin.load(&mut PluginContext::new(), args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn transform(&self, args: &HookTransformArgs<'_>) -> HookTransformReturn {
    for plugin in self.plugins() {
      if let Some(r) = plugin.transform(&mut PluginContext::new(), args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn build_end(&self, args: Option<&HookBuildEndArgs>) -> HookNoopReturn {
    for plugin in self.plugins() {
      plugin.build_end(&mut PluginContext::new(), args).await?;
    }
    Ok(())
  }
}
