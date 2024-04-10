use crate::{
  HookBuildEndArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookResolveIdArgs,
  HookResolveIdReturn, HookTransformArgs, PluginDriver,
};
use rolldown_error::Result;
use rolldown_sourcemap::SourceMap;

impl PluginDriver {
  pub async fn build_start(&self) -> HookNoopReturn {
    // TODO should call `build_start` of all plugins in parallel
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

  pub async fn transform(&self, args: &HookTransformArgs<'_>) -> Result<(String, Vec<SourceMap>)> {
    let mut sourcemap_chain = vec![];
    let mut code = args.code.to_string();
    for (plugin, ctx) in &self.plugins {
      if let Some(r) =
        plugin.transform(ctx, &HookTransformArgs { id: args.id, code: &code }).await?
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
    for (plugin, ctx) in &self.plugins {
      plugin.build_end(ctx, args).await?;
    }
    Ok(())
  }
}
