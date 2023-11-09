use std::sync::Arc;

use crate::{
  plugin::plugin::BoxPlugin, HookLoadArgs, HookLoadReturn, HookResolveIdArgs, HookResolveIdReturn,
  HookTransformArgs, HookTransformReturn, PluginContext,
};

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  _plugins: Vec<BoxPlugin>,
}

impl PluginDriver {
  pub fn new(plugins: Vec<BoxPlugin>) -> Self {
    Self { _plugins: plugins }
  }

  pub async fn _resolve_id(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
    for plugin in &self._plugins {
      match plugin.resolve_id(&mut PluginContext::new(), args).await {
        Err(e) => return Err(e),
        Ok(r) => {
          if let Some(r) = r {
            return Ok(Some(r));
          }
        }
      }
    }
    Ok(None)
  }

  pub async fn _load(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    for plugin in &self._plugins {
      match plugin.load(&mut PluginContext::new(), args).await {
        Err(e) => return Err(e),
        Ok(r) => {
          if let Some(r) = r {
            return Ok(Some(r));
          }
        }
      }
    }
    Ok(None)
  }

  pub async fn _transform(&self, args: &HookTransformArgs<'_>) -> HookTransformReturn {
    for plugin in &self._plugins {
      match plugin.transform(&mut PluginContext::new(), args).await {
        Err(e) => return Err(e),
        Ok(r) => {
          if let Some(r) = r {
            return Ok(Some(r));
          }
        }
      }
    }
    Ok(None)
  }
}
