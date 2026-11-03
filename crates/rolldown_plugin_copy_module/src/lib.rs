use std::{borrow::Cow, path::Path};

use arcstr::ArcStr;
use rolldown_common::{ModuleType, ResolvedExternal};
use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext, PluginHookMeta, PluginOrder,
};
use rolldown_plugin_utils::{emit_asset, rewrite_emitted_asset_references};
use rolldown_utils::url::clean_url;
use rustc_hash::FxHashSet;

const PREFIX: &str = "__ROLLDOWN_COPY_MODULE__#";

#[derive(Debug)]
pub struct CopyModulePlugin {
  copy_extensions: FxHashSet<String>,
}

impl CopyModulePlugin {
  pub fn new(module_types: &rustc_hash::FxHashMap<Cow<'static, str>, ModuleType>) -> Self {
    let mut copy_extensions = FxHashSet::default();
    for (ext, module_type) in module_types {
      if matches!(module_type, ModuleType::Copy) {
        let ext = ext.strip_prefix('.').unwrap_or(ext);
        copy_extensions.insert(ext.to_string());
      }
    }
    Self { copy_extensions }
  }
}

impl Plugin for CopyModulePlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:copy-module")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::RenderChunk
  }

  fn resolve_id_meta(&self) -> Option<PluginHookMeta> {
    // Run before users' resolve_id hooks to ensure:
    // - For matched modules, to handle it correctly without users' interference.
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if self.copy_extensions.is_empty() {
      return Ok(None);
    }

    // Don't re-resolve our own prefixed IDs
    if args.specifier.starts_with(PREFIX) {
      return Ok(None);
    }

    // Resolve the specifier to get the absolute path
    let resolved = ctx.resolve(args.specifier, args.importer, None).await?;

    let resolved_id = match resolved {
      Ok(id) => id,
      Err(_) => return Ok(None),
    };

    // Honor external resolutions from other plugins. `rolldown-plugin-dts`,
    // for instance, marks TypeScript ambient-module glob specifiers like
    // `typeof import("*.jpg")` as `{ id, external: true }` so they pass
    // through untouched. Without this check the copy plugin would ignore the
    // `external` flag and try to read `*.jpg` from disk, emitting a
    // confusing `Failed to read copy module *.jpg` error.
    if resolved_id.external.is_external() {
      return Ok(None);
    }

    // Strip query/fragment (e.g. `file.txt?url`) before extension check and file read
    let clean_id = clean_url(resolved_id.id.as_str());
    let resolved_path = Path::new(clean_id);

    // Check if the resolved path has a copy extension
    let ext = match resolved_path.extension().and_then(|e| e.to_str()) {
      Some(e) => e,
      None => return Ok(None),
    };

    if !self.copy_extensions.contains(ext) {
      return Ok(None);
    }

    let reference_id = emit_asset(ctx, clean_id, |e| {
      anyhow::anyhow!("Failed to read copy module {}: {e}", resolved_id.id)
    })
    .await?;
    ctx.add_watch_file(clean_id);

    // Return a prefixed external ID — the prefix will be rewritten in render_chunk
    let placeholder_id: ArcStr = format!("{PREFIX}{reference_id}").into();

    Ok(Some(HookResolveIdOutput {
      id: placeholder_id,
      external: Some(ResolvedExternal::Bool(true)),
      ..Default::default()
    }))
  }

  fn render_chunk_meta(&self) -> Option<PluginHookMeta> {
    // Run before users' render_chunk hooks to ensure:
    // - The placeholder IDs are replaced before any user hooks, so they won't see the placeholder IDs and won't interfere with our processing.
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn render_chunk(
    &self,
    ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    Ok(rewrite_emitted_asset_references(ctx, args, PREFIX))
  }
}
