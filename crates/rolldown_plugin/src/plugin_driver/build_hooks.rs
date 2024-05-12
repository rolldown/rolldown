use std::sync::Arc;

use crate::{
  HookBuildEndArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookResolveDynamicImportArgs,
  HookResolveIdArgs, HookResolveIdReturn, HookTransformArgs, PluginDriver,
};
use anyhow::Result;
use rolldown_common::ModuleInfo;
use rolldown_sourcemap::SourceMap;
use rolldown_utils::futures::block_on_spawn_all;

impl PluginDriver {
  #[tracing::instrument(level = "trace", skip_all)]
  pub async fn build_start(&self) -> HookNoopReturn {
    let ret = {
      #[cfg(not(target_arch = "wasm32"))]
      {
        block_on_spawn_all(self.plugins.iter().map(|(plugin, ctx)| plugin.build_start(ctx))).await
      }
      #[cfg(target_arch = "wasm32")]
      {
        // FIXME(hyf0): This is a workaround for wasm32 target, it's wired that
        // `block_on_spawn_all(self.plugins.iter().map(|(plugin, ctx)| plugin.build_start(ctx))).await;` will emit compile errors like
        // `implementation of `std::marker::Send` is not general enough`. It seems to be the problem related to HRTB, async and iterator.
        // I guess we need some rust experts here.
        let mut futures = vec![];
        for (plugin, ctx) in &self.plugins {
          futures.push(plugin.build_start(ctx));
        }
        block_on_spawn_all(futures.into_iter()).await
      }
    };

    for r in ret {
      r?;
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

  #[allow(deprecated)]
  // Only for rollup compatibility
  pub async fn resolve_dynamic_import(
    &self,
    args: &HookResolveDynamicImportArgs<'_>,
  ) -> HookResolveIdReturn {
    for (plugin, ctx) in &self.plugins {
      if let Some(r) = plugin.resolve_dynamic_import(ctx, args).await? {
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
    for (plugin, ctx) in &self.plugins {
      plugin.build_end(ctx, args).await?;
    }
    Ok(())
  }
}
