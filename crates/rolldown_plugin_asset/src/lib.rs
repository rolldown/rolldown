mod utils;

use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};
use rolldown_utils::{
  pattern_filter::{StringOrRegex, filter as pattern_filter},
  url::clean_url,
};

#[derive(Debug, Default)]
pub struct AssetPlugin {
  pub public_dir: Option<String>,
  pub assets_include: Vec<StringOrRegex>,
}

impl Plugin for AssetPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:asset")
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    let cleaned_id = clean_url(args.specifier);
    let is_valid_assets = utils::has_special_ext(cleaned_id)
      || ((cleaned_id.len() != args.specifier.len()) && utils::contains_url_param(args.specifier));

    if !is_valid_assets
      && (self.assets_include.is_empty()
        || !pattern_filter(
          None::<&[StringOrRegex]>,
          Some(&self.assets_include),
          cleaned_id,
          ctx.cwd().to_string_lossy().as_ref(),
        )
        .inner())
    {
      return Ok(None);
    }

    if utils::check_public_file(cleaned_id, self.public_dir.as_deref()).is_some() {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: args.specifier.into(),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}
