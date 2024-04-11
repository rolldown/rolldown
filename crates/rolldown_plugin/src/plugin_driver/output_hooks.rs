use crate::{HookNoopReturn, PluginDriver};
use futures::future::join_all;
use rolldown_common::Output;

impl PluginDriver {
  pub async fn render_start(&self) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      plugin.render_start(ctx).await?;
    }
    Ok(())
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
