use std::{borrow::Cow, sync::Arc};

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug, Default)]
pub struct OxcRuntimePlugin {
  pub resolve_base: Option<String>,
}

impl Plugin for OxcRuntimePlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:oxc-runtime")
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if args.specifier.starts_with("@oxc-project/runtime/") {
      let resolved_id = ctx
        .resolve(
          args.specifier,
          self.resolve_base.as_deref(),
          Some(rolldown_plugin::PluginContextResolveOptions {
            skip_self: true,
            import_kind: args.kind,
            custom: Arc::clone(&args.custom),
          }),
        )
        .await??;

      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: resolved_id.id,
        external: Some(resolved_id.external),
        side_effects: resolved_id.side_effects,
        normalize_external_id: resolved_id.normalize_external_id,
      }));
    }

    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}
