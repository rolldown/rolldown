use std::sync::Arc;

use crate::{
  plugin::plugin::BoxPlugin, HookLoadArgs, HookLoadReturn, HookResolveIdArgs, HookResolveIdReturn,
  HookTransformArgs, HookTransformReturn, PluginContext,
};

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: Vec<BoxPlugin>,
}

impl PluginDriver {
  pub fn new(plugins: Vec<BoxPlugin>) -> Self {
    Self { plugins }
  }

  pub async fn _resolve_id(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
    for plugin in &self.plugins {
      if let Some(r) = plugin.resolve_id(&mut PluginContext::new(), args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn load(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    for plugin in &self.plugins {
      if let Some(r) = plugin.load(&mut PluginContext::new(), args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn transform(&self, args: &HookTransformArgs<'_>) -> HookTransformReturn {
    for plugin in &self.plugins {
      if let Some(r) = plugin.transform(&mut PluginContext::new(), args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }
}
