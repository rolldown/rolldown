mod types;
mod utils;

use std::path::Path;
use std::{borrow::Cow, sync::Arc};

use rolldown_common::ModuleType;
use rolldown_plugin::{Plugin, PluginContextResolveOptions, SharedTransformPluginContext};
use rolldown_utils::{clean_url::clean_url, pattern_filter::StringOrRegex};

use types::transform_options::TransformOptions;

#[derive(Debug, Default)]
pub struct TransformPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  pub jsx_refresh_include: Vec<StringOrRegex>,
  pub jsx_refresh_exclude: Vec<StringOrRegex>,

  pub jsx_inject: Option<String>,

  pub filename: String,
  pub environment_consumer: String,

  pub transform_options: TransformOptions,
}

/// only handle ecma like syntax, `jsx`,`tsx`,`ts`
impl Plugin for TransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:transform")
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
          Some(&self.filename),
          Some(PluginContextResolveOptions {
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

  async fn transform(
    &self,
    ctx: SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let cwd = ctx.inner.cwd().to_string_lossy();
    let ext = Path::new(args.id).extension().map(|s| s.to_string_lossy());
    let module_type = ext.as_ref().map(|s| ModuleType::from_str_with_fallback(clean_url(s)));
    if !self.filter(args.id, &cwd, &module_type) {
      return Ok(None);
    }

    Ok(None)
  }
}
