use std::sync::Arc;

use crate::{
  HookBuildEndArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookResolveIdArgs,
  HookResolveIdReturn, HookTransformArgs, PluginDriver,
};
use anyhow::Result;
use rolldown_common::ModuleInfo;
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

  #[allow(clippy::unnecessary_cast)]
  pub async fn transform(&self, args: &HookTransformArgs<'_>) -> Result<(String, Vec<SourceMap>)> {
    let mut sourcemap_chain = vec![];
    let mut code = args.code.to_string();
    for (plugin, ctx) in &self.plugins {
      if let Some(r) =
        plugin.transform(ctx, &HookTransformArgs { id: args.id, code: &code }).await?
      {
        code = r.code;
        if let Some(mut map) = r.map {
          // If sourcemap  hasn't `sources`, using original id to fill it.
          if map.get_source(0 as u32).map_or(true, str::is_empty) {
            map.set_sources(vec![args.id]);
          }
          // If sourcemap hasn't `sourcesContent`, using original code to fill it.
          if map.get_source_content(0 as u32).map_or(true, str::is_empty) {
            map.set_source_contents(vec![args.code]);
          }
          sourcemap_chain.push(map);
        }
      }
    }
    Ok((code, sourcemap_chain))
  }

  pub async fn module_parsed(&self, module_info: Arc<ModuleInfo>) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      plugin.module_parsed(ctx, Arc::clone(&module_info)).await?;
    }
    Ok(())
  }

  pub async fn build_end(&self, args: Option<&HookBuildEndArgs>) -> HookNoopReturn {
    tracing::info!("PluginDriver::build_end");
    for (plugin, ctx) in &self.plugins {
      plugin.build_end(ctx, args).await?;
    }
    Ok(())
  }
}
