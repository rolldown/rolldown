use std::sync::{Arc, Weak};

use rolldown_error::BuildError;
use rolldown_fs::FileSystem;

use rolldown_error::BuildError;

use crate::{
  plugin::{
    args::{HookBuildEndArgs, RenderChunkArgs},
    context::TransformPluginContext,
    plugin::{BoxPlugin, HookNoopReturn},
  },
  HookLoadArgs, HookLoadReturn, HookResolveIdArgs, HookResolveIdReturn, HookTransformArgs,
  HookTransformReturn, PluginContext,
};

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: Vec<BoxPlugin>,
}

impl PluginDriver {
  pub fn new(plugins: Vec<BoxPlugin>) -> Self {
    Self { plugins }
  }

  pub async fn build_start(&self) -> HookNoopReturn {
    for plugin in &self.plugins {
      plugin.build_start(&mut PluginContext::new()).await?;
    }
    Ok(())
  }

  pub async fn resolve_id(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
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

  pub async fn build_end(&self, args: Option<&HookBuildEndArgs>) -> HookNoopReturn {
    for plugin in &self.plugins {
      plugin.build_end(&mut PluginContext::new(), args).await?;
    }
    Ok(())
  }

  pub async fn render_chunk(&self, mut args: RenderChunkArgs) -> Result<String, BuildError> {
    for (plugin, ctx) in &self.plugins {
      if let Some(r) = plugin.render_chunk(ctx, &args).await? {
        args.code = r.code;
      }
    }
    Ok(args.code)
  }
}
