use std::sync::Arc;

use crate::{
  pluginable::HookTransformAstReturn,
  types::{
    hook_resolve_id_skipped::HookResolveIdSkipped, hook_transform_ast_args::HookTransformAstArgs,
    plugin_idx::PluginIdx,
  },
  HookBuildEndArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookResolveIdArgs,
  HookResolveIdReturn, HookTransformArgs, PluginContext, PluginDriver, TransformPluginContext,
};
use anyhow::Result;
use rolldown_common::{side_effects::HookSideEffects, ModuleInfo, ModuleType};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::futures::block_on_spawn_all;

use super::hook_filter::{filter_load, filter_resolve_id, filter_transform};

impl PluginDriver {
  #[tracing::instrument(level = "trace", skip_all)]
  pub async fn build_start(&self) -> HookNoopReturn {
    let ret = {
      #[cfg(not(target_arch = "wasm32"))]
      {
        block_on_spawn_all(
          self
            .iter_plugin_with_context_by_order(&self.order_by_build_start_meta)
            .map(|(_, plugin, ctx)| plugin.call_build_start(ctx)),
        )
        .await
      }
      #[cfg(target_arch = "wasm32")]
      {
        // FIXME(hyf0): This is a workaround for wasm32 target, it's wired that
        // `block_on_spawn_all(self.plugins.iter().map(|(plugin, ctx)| plugin.build_start(ctx))).await;` will emit compile errors like
        // `implementation of `std::marker::Send` is not general enough`. It seems to be the problem related to HRTB, async and iterator.
        // I guess we need some rust experts here.
        let mut futures = vec![];
        for (_, plugin, ctx) in
          self.iter_plugin_with_context_by_order(&self.order_by_build_start_meta)
        {
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

  #[inline]
  fn get_resolve_call_skipped_plugins(
    specifier: &str,
    importer: Option<&str>,
    skipped_resolve_calls: Option<&Vec<Arc<HookResolveIdSkipped>>>,
  ) -> Vec<PluginIdx> {
    let mut skipped_plugins = vec![];
    if let Some(skipped_resolve_calls) = skipped_resolve_calls {
      for skip_resolve_call in skipped_resolve_calls {
        if skip_resolve_call.specifier == specifier
          && skip_resolve_call.importer.as_deref() == importer
        {
          skipped_plugins.push(skip_resolve_call.plugin_idx);
        }
      }
    }
    skipped_plugins
  }

  pub async fn resolve_id(
    &self,
    args: &HookResolveIdArgs<'_>,
    skipped_resolve_calls: Option<&Vec<Arc<HookResolveIdSkipped>>>,
  ) -> HookResolveIdReturn {
    let skipped_plugins =
      Self::get_resolve_call_skipped_plugins(args.specifier, args.importer, skipped_resolve_calls);
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_resolve_id_meta)
    {
      if skipped_plugins.iter().any(|p| *p == plugin_idx) {
        continue;
      }
      let filter_option = &self.index_plugin_filters[plugin_idx];
      if filter_resolve_id(filter_option, args.specifier, ctx.cwd()) == Some(false) {
        continue;
      }
      if let Some(r) = plugin
        .call_resolve_id(
          &skipped_resolve_calls.map_or_else(
            || ctx.clone(),
            |skipped_resolve_calls| {
              PluginContext::new_shared_with_skipped_resolve_calls(
                ctx,
                skipped_resolve_calls.clone(),
              )
            },
          ),
          args,
        )
        .await?
      {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  #[allow(deprecated)]
  // Only for rollup compatibility
  pub async fn resolve_dynamic_import(
    &self,
    args: &HookResolveIdArgs<'_>,
    skipped_resolve_calls: Option<&Vec<Arc<HookResolveIdSkipped>>>,
  ) -> HookResolveIdReturn {
    let skipped_plugins =
      Self::get_resolve_call_skipped_plugins(args.specifier, args.importer, skipped_resolve_calls);
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_resolve_dynamic_import_meta)
    {
      if skipped_plugins.iter().any(|p| *p == plugin_idx) {
        continue;
      }
      if let Some(r) = plugin
        .call_resolve_dynamic_import(
          &skipped_resolve_calls.map_or_else(
            || ctx.clone(),
            |skipped_resolve_calls| {
              PluginContext::new_shared_with_skipped_resolve_calls(
                ctx,
                skipped_resolve_calls.clone(),
              )
            },
          ),
          args,
        )
        .await?
      {
        return Ok(Some(r));
      }
    }
    Ok(None)
  }

  pub async fn load(&self, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_load_meta)
    {
      let filter_option = &self.index_plugin_filters[plugin_idx];
      if filter_load(filter_option, args.id, ctx.cwd()) == Some(false) {
        continue;
      }
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
    module_type: &mut ModuleType,
  ) -> Result<String> {
    let mut code = args.code.to_string();
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_transform_meta)
    {
      let filter_option = &self.index_plugin_filters[plugin_idx];
      if !filter_transform(filter_option, args.id, ctx.cwd(), module_type, &code) {
        continue;
      }
      if let Some(r) = plugin
        .call_transform(
          &TransformPluginContext::new(ctx.clone(), sourcemap_chain, original_code, args.id),
          &HookTransformArgs { id: args.id, code: &code, module_type: &*module_type },
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
        if let Some(ty) = r.module_type {
          *module_type = ty;
        }
      }
    }
    Ok(code)
  }

  pub fn transform_ast(&self, mut args: HookTransformAstArgs) -> HookTransformAstReturn {
    for (_, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_transform_ast_meta)
    {
      args.ast = plugin.call_transform_ast(
        ctx,
        HookTransformAstArgs { cwd: args.cwd, ast: args.ast, id: args.id },
      )?;
    }
    Ok(args.ast)
  }

  pub async fn module_parsed(&self, module_info: Arc<ModuleInfo>) -> HookNoopReturn {
    for (_, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_module_parsed_meta)
    {
      plugin.call_module_parsed(ctx, Arc::clone(&module_info)).await?;
    }
    Ok(())
  }

  pub async fn build_end(&self, args: Option<&HookBuildEndArgs>) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_build_end_meta) {
      plugin.call_build_end(ctx, args).await?;
    }
    Ok(())
  }
}
