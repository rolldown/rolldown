use crate::{HookNoopReturn, PluginDriver};
use futures::future::join_all;
use rolldown_common::Output;

impl PluginDriver {
  pub async fn generate_bundle(&self, bundle: &Vec<Output>, is_write: bool) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      let results = plugin
        .run_all(self.worker_manager.as_ref(), |plugin| {
          plugin.generate_bundle(ctx, bundle, is_write)
        })
        .await;

      for result in results {
        result?;
      }
    }
    Ok(())
  }

  pub async fn write_bundle(&self, bundle: &Vec<Output>) -> HookNoopReturn {
    let results = join_all(self.plugins.iter().map(|(plugin, ctx)| {
      plugin.run_all(self.worker_manager.as_ref(), |plugin| plugin.write_bundle(ctx, bundle))
    }))
    .await;

    for result in results.into_iter().flatten() {
      result?;
    }

    Ok(())
  }
}
