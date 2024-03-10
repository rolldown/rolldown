use std::sync::Arc;

use rolldown_common::Output;
use rolldown_error::BuildError;
use rolldown_plugin::{
  BoxPlugin, HookBuildEndArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookResolveIdArgs,
  HookResolveIdReturn, HookTransformArgs, PluginContext, RenderChunkArgs,
};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::block_on_spawn_all;

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

  pub async fn transform(
    &self,
    args: &HookTransformArgs<'_>,
  ) -> Result<(String, Vec<SourceMap>), BuildError> {
    let mut sourcemap_chain = vec![];
    let mut code = args.code.to_string();
    for plugin in &self.plugins {
      if let Some(r) = plugin
        .transform(&mut PluginContext::new(), &HookTransformArgs { id: args.id, code: &code })
        .await?
      {
        code = r.code;
        if let Some(map) = r.map {
          sourcemap_chain.push(map);
        }
      }
    }
    Ok((code, sourcemap_chain))
  }

  pub async fn build_end(&self, args: Option<&HookBuildEndArgs>) -> HookNoopReturn {
    tracing::info!("PluginDriver::build_end");
    for plugin in &self.plugins {
      plugin.build_end(&mut PluginContext::new(), args).await?;
    }
    Ok(())
  }

  pub async fn render_chunk(&self, mut args: RenderChunkArgs<'_>) -> Result<String, BuildError> {
    for plugin in &self.plugins {
      if let Some(r) = plugin.render_chunk(&PluginContext::new(), &args).await? {
        args.code = r.code;
      }
    }
    Ok(args.code)
  }

  pub async fn generate_bundle(&self, bundle: &Vec<Output>, is_write: bool) -> HookNoopReturn {
    for plugin in &self.plugins {
      plugin.generate_bundle(&PluginContext::new(), bundle, is_write).await?;
    }
    Ok(())
  }

  #[allow(clippy::unused_async)]
  pub async fn write_bundle(&self, bundle: &Vec<Output>) -> HookNoopReturn {
    let result = block_on_spawn_all(self.plugins.iter().map(|plugin| async move {
      match plugin.write_bundle(&PluginContext::new(), bundle).await {
        Ok(()) => Ok(()),
        Err(e) => Err(e),
      }
    }));

    for value in result {
      value?;
    }

    Ok(())
  }
}
