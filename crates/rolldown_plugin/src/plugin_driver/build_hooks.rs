use crate::{
  HookBuildEndArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookRenderChunkArgs,
  HookResolveIdArgs, HookResolveIdReturn, HookTransformArgs, PluginDriver,
};
use rolldown_error::BuildError;
use rolldown_sourcemap::SourceMap;

impl PluginDriver {
  pub async fn build_start(&self) -> HookNoopReturn {
    // TODO should call `build_start` of all plugins in parallel
    for (plugin, ctx) in &self.plugins {
      let results = plugin.run_all(self.worker_manager.as_ref(), |p| p.build_start(ctx)).await;
      for result in results {
        result?;
      }
    }
    Ok(())
  }

  pub async fn resolve_id(&self, args: &HookResolveIdArgs<'_>) -> HookResolveIdReturn {
    for (plugin, ctx) in &self.plugins {
      let result =
        plugin.run_single(self.worker_manager.as_ref(), |p| p.resolve_id(ctx, args)).await?;
      if let Some(r) = result {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn load(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    for (plugin, ctx) in &self.plugins {
      let result = plugin.run_single(self.worker_manager.as_ref(), |p| p.load(ctx, args)).await?;
      if let Some(r) = result {
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
    for (plugin, ctx) in &self.plugins {
      let args = HookTransformArgs { id: args.id, code: &code };
      let result =
        plugin.run_single(self.worker_manager.as_ref(), |p| p.transform(ctx, &args)).await?;

      if let Some(r) = result {
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
    for (plugin, ctx) in &self.plugins {
      let results = plugin.run_all(self.worker_manager.as_ref(), |p| p.build_end(ctx, args)).await;
      for result in results {
        result?;
      }
    }
    Ok(())
  }

  pub async fn render_chunk(
    &self,
    mut args: HookRenderChunkArgs<'_>,
  ) -> Result<(String, Vec<SourceMap>), BuildError> {
    let mut sourcemap_chain = vec![];
    for (plugin, ctx) in &self.plugins {
      let result =
        plugin.run_single(self.worker_manager.as_ref(), |p| p.render_chunk(ctx, &args)).await?;
      if let Some(r) = result {
        args.code = r.code;
        if let Some(map) = r.map {
          sourcemap_chain.push(map);
        }
      }
    }
    Ok((args.code, sourcemap_chain))
  }
}
