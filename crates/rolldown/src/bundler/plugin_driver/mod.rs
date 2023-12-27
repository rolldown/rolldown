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
  plugins: Vec<(BoxPlugin, PluginContext)>,
}

impl PluginDriver {
  pub fn new(plugins: Vec<BoxPlugin>) -> Self {
    Self {
      plugins: plugins.into_iter().map(|plugin| (plugin, PluginContext::new())).collect::<Vec<_>>(),
    }
  }

  pub async fn build_start(&self) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      plugin.build_start(ctx).await?;
    }
    Ok(())
  }

  pub async fn resolve_id(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
    for (plugin, ctx) in &self.plugins {
      if let Some(r) = plugin.resolve_id(ctx, args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn load(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    for (plugin, ctx) in &self.plugins {
      if let Some(r) = plugin.load(ctx, args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn transform(&self, args: &HookTransformArgs<'_>) -> HookTransformReturn {
    for (plugin, ctx) in &self.plugins {
      if let Some(r) = plugin.transform(ctx, args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn build_end(&self, args: Option<&HookBuildEndArgs>) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      plugin.build_end(ctx, args).await?;
    }
    Ok(())
  }
}
