use std::sync::Arc;

use crate::types::hook_render_error::HookRenderErrorArgs;
use crate::{HookAddonArgs, PluginDriver};
use crate::{HookAugmentChunkHashReturn, HookNoopReturn, HookRenderChunkArgs};
use anyhow::{Context, Ok, Result};
use rolldown_common::{Output, RollupRenderedChunk, SharedNormalizedBundlerOptions};
use rolldown_debug::{action, trace_action};
use rolldown_error::{BuildDiagnostic, CausedPlugin};
use rolldown_sourcemap::SourceMap;
use tracing::Instrument;

impl PluginDriver {
  pub async fn render_start(&self, opts: &SharedNormalizedBundlerOptions) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_render_start_meta)
    {
      plugin
        .call_render_start(ctx, &crate::HookRenderStartArgs { options: opts })
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?;
    }
    Ok(())
  }

  pub async fn banner(&self, args: HookAddonArgs, mut banner: String) -> Result<Option<String>> {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_banner_meta) {
      if let Some(r) = plugin
        .call_banner(ctx, &args)
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?
      {
        banner.push('\n');
        banner.push_str(r.as_str());
      }
    }
    if banner.is_empty() {
      return Ok(None);
    }
    Ok(Some(banner))
  }

  pub async fn footer(&self, args: HookAddonArgs, mut footer: String) -> Result<Option<String>> {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_footer_meta) {
      if let Some(r) = plugin
        .call_footer(ctx, &args)
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?
      {
        footer.push('\n');
        footer.push_str(r.as_str());
      }
    }
    if footer.is_empty() {
      return Ok(None);
    }
    Ok(Some(footer))
  }

  pub async fn intro(&self, args: HookAddonArgs, mut intro: String) -> Result<Option<String>> {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_intro_meta) {
      if let Some(r) = plugin
        .call_intro(ctx, &args)
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?
      {
        intro.push('\n');
        intro.push_str(r.as_str());
      }
    }
    if intro.is_empty() {
      return Ok(None);
    }
    Ok(Some(intro))
  }

  pub async fn outro(&self, args: HookAddonArgs, mut outro: String) -> Result<Option<String>> {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_outro_meta) {
      if let Some(r) = plugin
        .call_outro(ctx, &args)
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?
      {
        outro.push('\n');
        outro.push_str(r.as_str());
      }
    }
    if outro.is_empty() {
      return Ok(None);
    }
    Ok(Some(outro))
  }

  pub async fn render_chunk(
    &self,
    mut args: HookRenderChunkArgs<'_>,
  ) -> Result<(String, Vec<SourceMap>)> {
    let mut sourcemap_chain = vec![];
    for (plugin_idx, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_render_chunk_meta)
    {
      async {
        trace_action!(action::HookRenderChunkStart {
          action: "HookRenderChunkStart",
          plugin_name: plugin.call_name().to_string(),
          plugin_id: plugin_idx.raw(),
          call_id: "${call_id}",
          content: args.code.clone(),
        });
        if let Some(r) = plugin
          .call_render_chunk(ctx, &args)
          .await
          .with_context(|| CausedPlugin::new(plugin.call_name()))?
        {
          args.code = r.code;
          if let Some(map) = r.map {
            sourcemap_chain.push(map);
          }
          trace_action!(action::HookRenderChunkEnd {
            action: "HookRenderChunkEnd",
            plugin_name: plugin.call_name().to_string(),
            plugin_id: plugin_idx.raw(),
            call_id: "${call_id}",
            content: Some(args.code.clone()),
          });
        } else {
          trace_action!(action::HookRenderChunkEnd {
            action: "HookRenderChunkEnd",
            plugin_name: plugin.call_name().to_string(),
            plugin_id: plugin_idx.raw(),
            call_id: "${call_id}",
            content: None,
          });
        }

        Ok(())
      }
      .instrument(tracing::trace_span!(
        "HookRenderChunk",
        CONTEXT_call_id = rolldown_utils::uuid::uuid_v4()
      ))
      .await?;
    }
    Ok((args.code, sourcemap_chain))
  }

  pub async fn augment_chunk_hash(
    &self,
    chunk: Arc<RollupRenderedChunk>,
  ) -> HookAugmentChunkHashReturn {
    let mut hash = None;
    for (_, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_augment_chunk_hash_meta)
    {
      if let Some(plugin_hash) = plugin
        .call_augment_chunk_hash(ctx, Arc::clone(&chunk))
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?
      {
        hash.get_or_insert_with(String::default).push_str(&plugin_hash);
      }
    }
    Ok(hash)
  }

  pub async fn render_error(&self, args: &HookRenderErrorArgs<'_>) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_render_error_meta)
    {
      plugin
        .call_render_error(ctx, args)
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?;
    }
    Ok(())
  }

  pub async fn generate_bundle(
    &self,
    bundle: &mut Vec<Output>,
    is_write: bool,
    opts: &SharedNormalizedBundlerOptions,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> HookNoopReturn {
    for (_, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_generate_bundle_meta)
    {
      let mut args = crate::HookGenerateBundleArgs { is_write, bundle, options: opts };
      plugin
        .call_generate_bundle(ctx, &mut args)
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?;
      ctx.file_emitter().add_additional_files(bundle, warnings);
    }
    Ok(())
  }

  pub async fn write_bundle(
    &self,
    bundle: &mut Vec<Output>,
    opts: &SharedNormalizedBundlerOptions,
    warnings: &mut Vec<BuildDiagnostic>,
  ) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_write_bundle_meta)
    {
      let mut args = crate::HookWriteBundleArgs { bundle, options: opts };
      plugin
        .call_write_bundle(ctx, &mut args)
        .await
        .with_context(|| CausedPlugin::new(plugin.call_name()))?;
      ctx.file_emitter().add_additional_files(bundle, warnings);
    }
    Ok(())
  }

  pub async fn close_bundle(&self) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_close_bundle_meta)
    {
      plugin.call_close_bundle(ctx).await.with_context(|| CausedPlugin::new(plugin.call_name()))?;
    }
    Ok(())
  }
}
