use rolldown_common::{ModuleType, side_effects::HookSideEffects};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage,
  Plugin, PluginContext, SharedTransformPluginContext,
};
use rolldown_utils::dashmap::FxDashMap;
use std::{borrow::Cow, path::Path};

#[derive(Debug)]
pub struct ViteCssPlugin {
  styles: FxDashMap<String, String>,
  chunk_css_map: FxDashMap<String, String>,
}

impl Plugin for ViteCssPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    Cow::Borrowed("builtin:vite-css")
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !matches!(args.module_type, ModuleType::Css) {
      return Ok(None);
    }

    let style = args.code.to_string();
    self.styles.insert(args.id.to_string(), style);

    Ok(Some(HookTransformOutput {
      code: Some("export {}".to_string()),
      map: None,
      side_effects: Some(HookSideEffects::True),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  async fn render_chunk(
    &self,
    ctx: &PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let mut chunk_css = String::new();
    for module_id in &args.chunk.module_ids {
      if let Some(style) = self.styles.get(module_id.resource_id().as_str()) {
        chunk_css.push_str(style.as_str());
      }
    }

    if !chunk_css.is_empty() {
      ctx.emit_file(
        rolldown_common::EmittedAsset {
          name: Some(args.chunk.filename.to_string()),
          file_name: Some(args.chunk.filename.clone()),
          original_file_name: Some(args.chunk.filename.to_string()),
          source: chunk_css.into(),
        },
        None,
        None,
      );
    }

    Ok(None)
  }
}
