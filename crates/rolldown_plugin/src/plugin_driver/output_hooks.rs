use crate::types::hook_render_error::HookRenderErrorArgs;
use crate::{HookAddonArgs, PluginDriver};
use crate::{HookAugmentChunkHashReturn, HookNoopReturn, HookRenderChunkArgs};
use anyhow::{Ok, Result};
use rolldown_common::{Output, RollupRenderedChunk};
use rolldown_sourcemap::SourceMap;

impl PluginDriver {
  pub async fn render_start(&self) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_render_start_meta)
    {
      plugin.call_render_start(ctx).await?;
    }
    Ok(())
  }

  pub async fn banner(
    &self,
    args: HookAddonArgs<'_>,
    mut banner: String,
  ) -> Result<Option<String>> {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_banner_meta) {
      if let Some(r) = plugin.call_banner(ctx, &args).await? {
        banner.push('\n');
        banner.push_str(r.as_str());
      }
    }
    if banner.is_empty() {
      return Ok(None);
    }
    Ok(Some(banner))
  }

  pub async fn footer(
    &self,
    args: HookAddonArgs<'_>,
    mut footer: String,
  ) -> Result<Option<String>> {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_footer_meta) {
      if let Some(r) = plugin.call_footer(ctx, &args).await? {
        footer.push('\n');
        footer.push_str(r.as_str());
      }
    }
    if footer.is_empty() {
      return Ok(None);
    }
    Ok(Some(footer))
  }

  pub async fn intro(&self, args: HookAddonArgs<'_>, mut intro: String) -> Result<Option<String>> {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_intro_meta) {
      if let Some(r) = plugin.call_intro(ctx, &args).await? {
        intro.push('\n');
        intro.push_str(r.as_str());
      }
    }
    if intro.is_empty() {
      return Ok(None);
    }
    Ok(Some(intro))
  }

  pub async fn outro(&self, args: HookAddonArgs<'_>, mut outro: String) -> Result<Option<String>> {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_outro_meta) {
      if let Some(r) = plugin.call_outro(ctx, &args).await? {
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
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_render_chunk_meta)
    {
      if let Some(r) = plugin.call_render_chunk(ctx, &args).await? {
        args.code = r.code;
        if let Some(map) = r.map {
          sourcemap_chain.push(map);
        }
      }
    }
    Ok((args.code, sourcemap_chain))
  }

  pub async fn augment_chunk_hash(
    &self,
    chunk: &RollupRenderedChunk,
  ) -> HookAugmentChunkHashReturn {
    let mut hash = None;
    for (_, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_augment_chunk_hash_meta)
    {
      if let Some(plugin_hash) = plugin.call_augment_chunk_hash(ctx, chunk).await? {
        hash.get_or_insert_with(String::default).push_str(&plugin_hash);
      }
    }
    Ok(hash)
  }

  pub async fn render_error(&self, args: &HookRenderErrorArgs) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_render_error_meta)
    {
      plugin.call_render_error(ctx, args).await?;
    }
    Ok(())
  }

  pub async fn generate_bundle(&self, bundle: &mut Vec<Output>, is_write: bool) -> HookNoopReturn {
    for (_, plugin, ctx) in
      self.iter_plugin_with_context_by_order(&self.order_by_generate_bundle_meta)
    {
      plugin.call_generate_bundle(ctx, bundle, is_write).await?;
      ctx.file_emitter.add_additional_files(bundle);
    }
    Ok(())
  }

  pub async fn write_bundle(&self, bundle: &mut Vec<Output>) -> HookNoopReturn {
    for (_, plugin, ctx) in self.iter_plugin_with_context_by_order(&self.order_by_write_bundle_meta)
    {
      plugin.call_write_bundle(ctx, bundle).await?;
      ctx.file_emitter.add_additional_files(bundle);
    }
    Ok(())
  }
}
