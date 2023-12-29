use std::sync::{Arc, Weak};

use rolldown_fs::FileSystem;

use crate::{
  plugin::{
    args::HookBuildEndArgs,
    context::TransformPluginContext,
    plugin::{BoxPlugin, HookNoopReturn},
  },
  HookLoadArgs, HookLoadReturn, HookResolveIdArgs, HookResolveIdReturn, HookTransformArgs,
  HookTransformReturn, PluginContext, SharedResolver,
};

use super::options::input_options::SharedInputOptions;

pub type SharedPluginDriver<T> = Arc<PluginDriver<T>>;

#[derive(Debug)]
pub struct PluginDriver<T: FileSystem + Default> {
  plugins: Vec<(BoxPlugin<T>, PluginContext<T>)>,
}

impl<T: FileSystem + Default + 'static> PluginDriver<T> {
  pub fn with_shared(
    plugins: Vec<BoxPlugin<T>>,
    input_options: &SharedInputOptions,
    resolver: &SharedResolver<T>,
  ) -> Arc<Self> {
    Arc::new_cyclic(|v| Self {
      plugins: plugins
        .into_iter()
        .map(|plugin| {
          (
            plugin,
            PluginContext::new(Weak::clone(v), Arc::clone(input_options), Arc::clone(resolver)),
          )
        })
        .collect::<Vec<_>>(),
    })
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
      let context = TransformPluginContext::new(ctx);
      if let Some(r) = plugin.transform(&context, args).await? {
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
