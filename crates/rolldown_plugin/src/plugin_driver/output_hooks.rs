use crate::HookRenderChunkArgs;
use crate::{HookNoopReturn, PluginDriver};
use futures::future::join_all;
use rolldown_common::Output;
use rolldown_error::Result;
use rolldown_sourcemap::SourceMap;

impl PluginDriver {
  pub async fn render_start(&self) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      plugin.render_start(ctx).await?;
    }
    Ok(())
  }

  pub async fn render_chunk(
    &self,
    mut args: HookRenderChunkArgs<'_>,
  ) -> Result<(String, Vec<SourceMap>)> {
    let mut sourcemap_chain = vec![];
    for (plugin, ctx) in &self.plugins {
      if let Some(r) = plugin.render_chunk(ctx, &args).await? {
        args.code = r.code;
        if let Some(map) = r.map {
          sourcemap_chain.push(map);
        }
      }
    }
    Ok((args.code, sourcemap_chain))
  }

  pub async fn generate_bundle(&self, bundle: &Vec<Output>, is_write: bool) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      plugin.generate_bundle(ctx, bundle, is_write).await?;
    }
    Ok(())
  }

  pub async fn write_bundle(&self, bundle: &Vec<Output>) -> HookNoopReturn {
    let results =
      join_all(self.plugins.iter().map(|(plugin, ctx)| plugin.write_bundle(ctx, bundle))).await;

    for result in results {
      result?;
    }

    Ok(())
  }
}
