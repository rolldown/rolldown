use rolldown_common::RUNTIME_MODULE_KEY;
use rolldown_plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage, Plugin,
  SharedTransformPluginContext,
};

#[derive(Debug)]
pub struct HmrPlugin;

impl Plugin for HmrPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "builtin:hmr".into()
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    HookUsage::Transform
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

    if dev_mode_options.skip_common_runtime_injection != Some(true) {
      hmr_source.push_str(include_str!("./runtime/runtime-extra-dev-common.js"));
    }

    // The JS API supplies the default implementation. Rust consumers must provide the
    // complete implementation, including the common runtime when they need it.
    if let Some(implement) = dev_mode_options.implement.as_deref() {
      hmr_source.push_str(implement);
    }

    // Append to runtime
    let new_code = format!("{}\n// HMR Runtime\n{}", args.code, hmr_source);

    Ok(Some(HookTransformOutput { code: Some(new_code), ..Default::default() }))
  }

  fn transform_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }
}
