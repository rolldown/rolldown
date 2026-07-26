use std::{borrow::Cow, path::Path};

use rolldown_common::{ModuleType, side_effects::HookSideEffects};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookRenderChunkArgs, HookRenderChunkReturn,
  HookUsage, Plugin, PluginHookMeta, PluginOrder, SharedLoadPluginContext,
};
use rolldown_plugin_utils::{emit_asset, rewrite_emitted_asset_references};
use rolldown_utils::url::clean_url;
use rustc_hash::FxHashSet;

const PREFIX: &str = "__ROLLDOWN_ASSET__#";

#[derive(Debug)]
pub struct AssetModulePlugin {
  asset_extensions: FxHashSet<String>,
}

impl AssetModulePlugin {
  pub fn new(module_types: &rustc_hash::FxHashMap<Cow<'static, str>, ModuleType>) -> Self {
    let mut asset_extensions = FxHashSet::default();
    for (ext, module_type) in module_types {
      if matches!(module_type, ModuleType::Asset) {
        let ext = ext.strip_prefix('.').unwrap_or(ext);
        asset_extensions.insert(ext.to_string());
      }
    }
    Self { asset_extensions }
  }
}

impl Plugin for AssetModulePlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:asset-module")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load | HookUsage::RenderChunk
  }

  fn load_meta(&self) -> Option<PluginHookMeta> {
    // Run after user plugins so they can override asset loading
    Some(PluginHookMeta { order: Some(PluginOrder::Post) })
  }

  fn load(
    &self,
    ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    self.load_impl(ctx, args)
  }

  fn render_chunk_meta(&self) -> Option<PluginHookMeta> {
    // Run before user plugins so placeholders are resolved first
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    Ok(rewrite_emitted_asset_references(ctx, args, PREFIX))
  }
}

impl AssetModulePlugin {
  async fn load_impl(
    &self,
    ctx: SharedLoadPluginContext,
    args: &HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    // Determine if this module should be treated as an asset:
    // 1. Via asserted_module_type (e.g. from `new URL('./file', import.meta.url)`)
    // 2. Via file extension matching `moduleTypes` config
    //
    // Strip query/fragment (e.g. `file.png?url`) before extension check and path operations,
    // consistent with CopyModulePlugin's handling.
    let clean_id = clean_url(args.id);
    let is_asset = args.asserted_module_type.is_some_and(|ty| matches!(ty, ModuleType::Asset))
      || self.is_asset_by_extension(clean_id);

    if !is_asset {
      return Ok(None);
    }

    let reference_id = emit_asset(&ctx, clean_id, |e| {
      anyhow::anyhow!("Failed to read asset module {clean_id}: {e}")
    })
    .await?;
    ctx.associate_module_with_file_ref(args.id, &reference_id);
    // Through LoadPluginContext, which also records the module's HMR transform dependency.
    ctx.add_watch_file(clean_id);

    // Return JS code that exports the asset placeholder via CJS.
    // Using `module.exports` ensures `require()` returns the string directly.
    // The placeholder will be resolved in renderChunk to the actual filename.
    let code = format!("module.exports = \"{PREFIX}{reference_id}\"");

    Ok(Some(HookLoadOutput {
      code: code.into(),
      module_type: Some(ModuleType::Js),
      // Mark as side-effect-free so tree-shaking excludes the module's statements
      // when nothing imports from it (e.g. `new URL()` only references).
      side_effects: Some(HookSideEffects::False),
      ..Default::default()
    }))
  }

  fn is_asset_by_extension(&self, id: &str) -> bool {
    if self.asset_extensions.is_empty() {
      return false;
    }
    Path::new(id)
      .extension()
      .and_then(|e| e.to_str())
      .is_some_and(|ext| self.asset_extensions.contains(ext))
  }
}
