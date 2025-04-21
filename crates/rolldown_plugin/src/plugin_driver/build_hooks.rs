use std::sync::Arc;

use crate::{
  HookBuildEndArgs, HookLoadArgs, HookLoadReturn, HookNoopReturn, HookResolveIdArgs,
  HookResolveIdReturn, HookTransformArgs, HookUsage, PluginContext, PluginDriver,
  TransformPluginContext,
  pluginable::HookTransformAstReturn,
  types::{
    hook_resolve_id_skipped::HookResolveIdSkipped, hook_transform_ast_args::HookTransformAstArgs,
    plugin_idx::PluginIdx,
  },
};
use anyhow::Result;
use rolldown_common::{
  ModuleInfo, ModuleType, NormalModule, SharedNormalizedBundlerOptions,
  side_effects::HookSideEffects,
};
use rolldown_debug::{action, trace_action};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::unique_arc::UniqueArc;
use string_wizard::{MagicString, SourceMapOptions};
use tracing::{Instrument, debug_span};
use valuable::Valuable;

impl PluginDriver {
  #[tracing::instrument(level = "trace", skip_all)]
  pub async fn build_start(&self, opts: &SharedNormalizedBundlerOptions) -> HookNoopReturn {
    // let ret = {
    //   #[cfg(not(target_arch = "wasm32"))]
    //   {
    //     block_on_spawn_all(
    //       self
    //         .iter_plugin_with_context_by_order(&self.order_by_build_start_meta)
    //         .map(|(_, plugin, ctx)| plugin.call_build_start(ctx)),
    //     )
    //     .await
    //   }
    //   #[cfg(target_arch = "wasm32")]
    //   {
    //     // FIXME(hyf0): This is a workaround for wasm32 target, it's wired that
    //     // `block_on_spawn_all(self.plugins.iter().map(|(plugin, ctx)| plugin.build_start(ctx))).await;` will emit compile errors like
    //     // `implementation of `std::marker::Send` is not general enough`. It seems to be the problem related to HRTB, async and iterator.
    //     // I guess we need some rust experts here.
    //     let mut futures = vec![];
    //     for (_, plugin, ctx) in
    //       self.iter_plugin_with_context_by_order(&self.order_by_build_start_meta)
    //     {
    //       futures.push(plugin.call_build_start(ctx));
    //     }
    //     block_on_spawn_all(futures.into_iter()).await
    //   }
    // };

    // for r in ret {
    //   r?;
    // }

    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_build_start_meta)
    {
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::BuildStart) {
        continue;
      }
      plugin.call_build_start(ctx, &crate::HookBuildStartArgs { options: opts }).await?;
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
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::ResolveId) {
        continue;
      }
      // TODO: Maybe we could optimize this a little
      if skipped_plugins.iter().any(|p| *p == plugin_idx) {
        continue;
      }
      trace_action!(action::HookResolveIdCallStart {
        kind: "HookResolveIdCallStart",
        importer: args.importer.map(ToString::to_string),
        module_request: args.specifier.to_string(),
        import_kind: args.kind.to_string(),
        plugin_name: plugin.call_name().to_string(),
        plugin_index: plugin_idx.raw()
      });
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
        .instrument(debug_span!("resolve_id_hook", plugin_name = plugin.call_name().as_ref()))
        .await?
      {
        trace_action!(action::HookResolveIdCallEnd {
          kind: "HookResolveIdCallEnd",
          resolved_id: Some(r.id.to_string()),
          is_external: r.external.map(|v| v.is_external()),
          plugin_name: plugin.call_name().to_string(),
          plugin_index: plugin_idx.raw(),
        });
        return Ok(Some(r));
      }
      trace_action!(action::HookResolveIdCallEnd {
        kind: "HookResolveIdCallEnd",
        resolved_id: None,
        is_external: None,
        plugin_name: plugin.call_name().to_string(),
        plugin_index: plugin_idx.raw(),
      });
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
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::ResolveDynamicImport) {
        continue;
      }
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
        .instrument(debug_span!(
          "resolve_dynamic_import_hook",
          plugin_name = plugin.call_name().as_ref()
        ))
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
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::Load) {
        continue;
      }
      trace_action!(action::HookLoadCallStart {
        kind: "HookLoadCallStart".to_string(),
        module_id: args.id.to_string(),
        plugin_name: plugin.call_name().to_string(),
        plugin_index: plugin_idx.raw()
      });
      if let Some(r) = plugin
        .call_load(ctx, args)
        .instrument(debug_span!("load_hook", plugin_name = plugin.call_name().as_ref()))
        .await?
      {
        trace_action!(action::HookLoadCallEnd {
          kind: "HookLoadCallEnd".to_string(),
          module_id: args.id.to_string(),
          source: Some(r.code.to_string()),
          plugin_name: plugin.call_name().to_string(),
          plugin_index: plugin_idx.raw()
        });
        return Ok(Some(r));
      }
      trace_action!(action::HookLoadCallEnd {
        kind: "HookLoadCallEnd".to_string(),
        module_id: args.id.to_string(),
        source: None,
        plugin_name: plugin.call_name().to_string(),
        plugin_index: plugin_idx.raw()
      });
    }
    Ok(None)
  }

  #[tracing::instrument(target = "devtool", level = "trace", skip_all)]
  pub async fn transform(
    &self,
    id: &str,
    original_code: String,
    sourcemap_chain: &mut Vec<SourceMap>,
    side_effects: &mut Option<HookSideEffects>,
    module_type: &mut ModuleType,
  ) -> Result<String> {
    let mut code = original_code;
    let mut original_sourcemap_chain = std::mem::take(sourcemap_chain);
    let mut plugin_sourcemap_chain = UniqueArc::new(original_sourcemap_chain);
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_transform_meta)
    {
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::Transform) {
        continue;
      }
      trace_action!(action::HookTransformCallStart {
        kind: "HookTransformCallStart".to_string(),
        module_id: id.to_string(),
        source: code.clone(),
        plugin_name: plugin.call_name().to_string(),
        plugin_index: plugin_idx.raw()
      });
      if let Some(r) = plugin
        .call_transform(
          Arc::new(TransformPluginContext::new(
            ctx.clone(),
            plugin_sourcemap_chain.weak_ref(),
            code.as_str().into(),
            id.into(),
          )),
          &HookTransformArgs { id, code: &code, module_type: &*module_type },
        )
        .instrument(debug_span!("transform_hook", plugin_name = plugin.call_name().as_ref()))
        .await?
      {
        original_sourcemap_chain = plugin_sourcemap_chain.into_inner();
        if let Some(map) = Self::normalize_transform_sourcemap(r.map, id, &code, r.code.as_ref()) {
          original_sourcemap_chain.push(map);
        }
        plugin_sourcemap_chain = UniqueArc::new(original_sourcemap_chain);
        if let Some(v) = r.side_effects {
          *side_effects = Some(v);
        }
        if let Some(v) = r.code {
          code = v;
          trace_action!(action::HookTransformCallEnd {
            kind: "HookTransformCallEnd".to_string(),
            module_id: id.to_string(),
            transformed_source: Some(code.to_string()),
            plugin_name: plugin.call_name().to_string(),
            plugin_index: plugin_idx.raw(),
          });
        }
        if let Some(ty) = r.module_type {
          *module_type = ty;
        }
      } else {
        trace_action!(action::HookTransformCallEnd {
          kind: "HookTransformCallEnd".to_string(),
          module_id: id.to_string(),
          transformed_source: Some(code.to_string()),
          plugin_name: plugin.call_name().to_string(),
          plugin_index: plugin_idx.raw(),
        });
      }
    }
    *sourcemap_chain = plugin_sourcemap_chain.into_inner();
    Ok(code)
  }

  #[inline]
  fn normalize_transform_sourcemap(
    map: Option<SourceMap>,
    id: &str,
    original_code: &str,
    code: Option<&String>,
  ) -> Option<SourceMap> {
    if let Some(mut map) = map {
      // If sourcemap  hasn't `sources`, using original id to fill it.
      let source = map.get_source(0);
      if source.is_none_or(str::is_empty)
        || (map.get_sources().count() == 1 && (source != Some(id)))
      {
        map.set_sources(vec![id]);
      }
      // If sourcemap hasn't `sourcesContent`, using original code to fill it.
      if map.get_source_content(0).is_none_or(str::is_empty) {
        map.set_source_contents(vec![original_code]);
      }
      Some(map)
    } else if let Some(code) = code {
      if original_code == code {
        None
      } else {
        // If sourcemap is empty and code has changed, need to create one remapping original code.
        // Here using `hires: true` to get more accurate column information, but it has more overhead.
        // TODO: maybe it should be add a option to control hires.
        let magic_string = MagicString::new(original_code);
        Some(magic_string.source_map(SourceMapOptions {
          hires: string_wizard::Hires::True,
          include_content: true,
          source: id.into(),
        }))
      }
    } else {
      None
    }
  }

  pub async fn transform_ast(&self, mut args: HookTransformAstArgs<'_>) -> HookTransformAstReturn {
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_transform_ast_meta)
    {
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::TransformAst) {
        continue;
      }
      args.ast = plugin
        .call_transform_ast(
          ctx,
          HookTransformAstArgs {
            cwd: args.cwd,
            ast: args.ast,
            id: args.id,
            stable_id: args.stable_id,
            is_user_defined_entry: args.is_user_defined_entry,
            module_type: args.module_type,
          },
        )
        .instrument(debug_span!("transform_ast_hook", plugin_name = plugin.call_name().as_ref()))
        .await?;
    }
    Ok(args.ast)
  }

  pub async fn module_parsed(
    &self,
    module_info: Arc<ModuleInfo>,
    normal_module: &NormalModule,
  ) -> HookNoopReturn {
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_module_parsed_meta)
    {
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::ModuleParsed) {
        continue;
      }
      plugin
        .call_module_parsed(ctx, Arc::clone(&module_info), normal_module)
        .instrument(debug_span!("module_parsed_hook", plugin_name = plugin.call_name().as_ref()))
        .await?;
    }
    Ok(())
  }

  pub async fn build_end(&self, args: Option<&HookBuildEndArgs<'_>>) -> HookNoopReturn {
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_build_end_meta)
    {
      if !self.plugin_usage_vec[plugin_idx].contains(HookUsage::BuildEnd) {
        continue;
      }
      plugin
        .call_build_end(ctx, args)
        .instrument(debug_span!("build_end_hook", plugin_name = plugin.call_name().as_ref()))
        .await?;
    }
    Ok(())
  }
}
