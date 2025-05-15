mod utils;

use std::borrow::Cow;

use rolldown_common::ModuleType;
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_utils::{
  pattern_filter::StringOrRegex, percent_encoding::encode_as_percent_escaped, url::clean_url,
};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct AssetPlugin {
  pub is_server: bool,
  pub url_base: String,
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
    if self.is_not_valid_assets(ctx.cwd(), args.specifier) {
      return Ok(None);
    }

    if self.check_public_file(clean_url(args.specifier)).is_some() {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: args.specifier.into(),
        ..Default::default()
      }));
    }

    Ok(None)
  }

  async fn load(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id.starts_with('\0') || utils::find_query_param(args.id, b"raw").is_some() {
      let cleaned_id = clean_url(args.id);
      let file = match self.check_public_file(cleaned_id) {
        Some(f) => Cow::Owned(f.to_string_lossy().into_owned()),
        None => Cow::Borrowed(cleaned_id),
      };

      ctx.add_watch_file(&file);

      let value = serde_json::from_str::<Value>(&std::fs::read_to_string(file.as_ref())?)?;
      let code = format!("export default {}", serde_json::to_string(&value)?);
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code,
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }

    if self.is_not_valid_assets(ctx.cwd(), args.id) {
      return Ok(None);
    }

    let id = utils::remove_url_query(args.id);
    let mut url = if self.is_server {
      self.file_to_dev_url(&id, ctx.cwd())?
    } else {
      self.file_to_built_url(&id)?
    };

    // TODO(shulaoda): align below logic
    // Inherit HMR timestamp if this asset was invalidated
    // if (!url.startsWith('data:') && this.environment.mode === 'dev') {
    //   const mod = this.environment.moduleGraph.getModuleById(id)
    //   if (mod && mod.lastHMRTimestamp > 0) {
    //     url = injectQuery(url, `t=${mod.lastHMRTimestamp}`)
    //   }
    // }

    if !url.starts_with("data:") {
      if let Some(value) = encode_as_percent_escaped(url.as_bytes()) {
        url = value;
      }
    }

    let code = format!("export default {}", serde_json::to_string(&Value::String(url))?);
    Ok(Some(rolldown_plugin::HookLoadOutput {
      code,
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }
}
