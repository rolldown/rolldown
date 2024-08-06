use std::sync::Arc;
use std::time::Instant;

use crate::{
  pluginable::HookTransformAstReturn, types::hook_transform_ast_args::HookTransformAstArgs,
  HookBuildEndArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookResolveDynamicImportArgs,
  HookResolveIdArgs, HookResolveIdReturn, HookTransformArgs, PluginDriver, TransformPluginContext,
};
use anyhow::Result;
use rolldown_common::{side_effects::HookSideEffects, ModuleInfo};
use rolldown_sourcemap::SourceMap;
use rolldown_stats::hook_metric::MetricType;
use rolldown_utils::futures::block_on_spawn_all;

impl PluginDriver {
  #[tracing::instrument(level = "trace", skip_all)]
  pub async fn build_start(&self) -> HookNoopReturn {
    let ret = {
      #[cfg(not(target_arch = "wasm32"))]
      {
        block_on_spawn_all(self.plugins.iter().map(|(plugin, ctx)| plugin.call_build_start(ctx)))
          .await
      }
      #[cfg(target_arch = "wasm32")]
      {
        // FIXME(hyf0): This is a workaround for wasm32 target, it's wired that
        // `block_on_spawn_all(self.plugins.iter().map(|(plugin, ctx)| plugin.build_start(ctx))).await;` will emit compile errors like
        // `implementation of `std::marker::Send` is not general enough`. It seems to be the problem related to HRTB, async and iterator.
        // I guess we need some rust experts here.
        let mut futures = vec![];
        for (plugin, ctx) in &self.plugins {
          futures.push(plugin.call_build_start(ctx));
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
    for (i, (plugin, ctx)) in self.plugins.iter().enumerate() {
      let _guard = ctx.stats.hook_metric[i].guard(MetricType::Resolve);
      if let Some(r) = plugin.call_resolve_id(ctx, args).await? {
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
      if let Some(r) = plugin.call_resolve_dynamic_import(ctx, args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn load(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    for (i, (plugin, ctx)) in self.plugins.iter().enumerate() {
      let _guard = ctx.stats.hook_metric[i].guard(MetricType::Load);
      if let Some(r) = plugin.call_load(ctx, args).await? {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn transform(
    &self,
    args: &HookTransformArgs<'_>,
    sourcemap_chain: &mut Vec<SourceMap>,
    side_effects: &mut Option<HookSideEffects>,
    original_code: &str,
  ) -> Result<String> {
    let mut code = args.code.to_string();
    for (i, (plugin, ctx)) in self.plugins.iter().enumerate() {
      let _guard = ctx.stats.hook_metric[i].guard(MetricType::Transform);
      if let Some(r) = plugin
        .call_transform(
          &TransformPluginContext::new(Arc::clone(ctx), sourcemap_chain, original_code, args.id),
          &HookTransformArgs { id: args.id, code: &code },
        )
        .await?
      {
        if let Some(mut map) = r.map {
          // If sourcemap  hasn't `sources`, using original id to fill it.
          if map.get_source(0).map_or(true, str::is_empty) {
            map.set_sources(vec![args.id]);
          }
          // If sourcemap hasn't `sourcesContent`, using original code to fill it.
          if map.get_source_content(0).map_or(true, str::is_empty) {
            map.set_source_contents(vec![&code]);
          }
          sourcemap_chain.push(map);
        }
        if let Some(v) = r.side_effects {
          *side_effects = Some(v);
        }
        if let Some(v) = r.code {
          code = v;
        }
      }
    }
    Ok(code)
  }

  pub fn transform_ast(&self, mut args: HookTransformAstArgs) -> HookTransformAstReturn {
    for (i, (plugin, ctx)) in self.plugins.iter().enumerate() {
      let _guard = ctx.stats.hook_metric[i].guard(MetricType::TransformAst);
      args.ast =
        plugin.call_transform_ast(ctx, HookTransformAstArgs { cwd: args.cwd, ast: args.ast })?;
    }
    Ok(args.ast)
  }

  pub async fn module_parsed(&self, module_info: Arc<ModuleInfo>) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      plugin.call_module_parsed(ctx, Arc::clone(&module_info)).await?;
    }
    Ok(())
  }

  pub async fn build_end(&self, args: Option<&HookBuildEndArgs>) -> HookNoopReturn {
    for (plugin, ctx) in &self.plugins {
      plugin.call_build_end(ctx, args).await?;
    }
    Ok(())
  }
}
