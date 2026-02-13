use rolldown_common::{Platform, RUNTIME_MODULE_KEY, ResolvedExternal};
use rolldown_plugin::{
  HookResolveIdOutput, HookTransformArgs, HookTransformOutput, HookTransformReturn, Plugin,
  RegisterHook, SharedTransformPluginContext,
};

#[derive(Debug)]
pub struct HmrPlugin;

#[RegisterHook]
impl Plugin for HmrPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "builtin:hmr".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    // Only handle ws external marking for Node.js
    if args.specifier == "ws" {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.into(),
        external: Some(ResolvedExternal::Bool(true)),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  fn resolve_id_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if args.id != RUNTIME_MODULE_KEY {
      return Ok(None);
    }

    let bundler_options = ctx.options();
    let Some(dev_mode_options) = &bundler_options.experimental.dev_mode else {
      return Ok(None);
    };

    let mut hmr_source = String::new();

    // Platform-specific WebSocket import
    if matches!(bundler_options.platform, Platform::Node) {
      hmr_source.push_str("import { WebSocket } from 'ws';\n");
    }

    // Common runtime
    hmr_source.push_str(include_str!("./runtime/runtime-extra-dev-common.js"));

    // Default or custom implementation
    if let Some(implement) = dev_mode_options.implement.as_deref() {
      hmr_source.push_str(implement);
    } else {
      let content = include_str!("./runtime/runtime-extra-dev-default.js");
      let host = dev_mode_options.host.as_deref().unwrap_or("localhost");
      let port = dev_mode_options.port.unwrap_or(3000);
      let addr = format!("{host}:{port}");
      hmr_source.push_str(&content.replace("$ADDR", &addr));
    }

    // Append to runtime
    let new_code = format!("{}\n// HMR Runtime\n{}", args.code, hmr_source);

    Ok(Some(HookTransformOutput { code: Some(new_code), ..Default::default() }))
  }

  fn transform_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }
}
