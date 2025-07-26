mod utils;

use std::{borrow::Cow, sync::Arc};

use rolldown_common::ModuleType;
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_plugin_utils::{FileToUrlEnv, check_public_file, file_to_url, find_special_query};
use rolldown_utils::{dashmap::FxDashMap, pattern_filter::StringOrRegex, url::clean_url};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct AssetPlugin {
  pub is_server: bool,
  pub url_base: String,
  pub public_dir: String,
  pub assets_include: Vec<StringOrRegex>,
}

#[allow(dead_code)]
#[derive(Default)]
struct AssetCache(pub FxDashMap<String, String>);

impl Plugin for AssetPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:asset")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart | HookUsage::ResolveId | HookUsage::Load
  }

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.meta().insert(Arc::new(AssetCache::default()));
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if self.is_not_valid_assets(ctx.cwd(), args.specifier) {
      return Ok(None);
    }
    Ok(check_public_file(clean_url(args.specifier), &self.public_dir).map(|_| {
      rolldown_plugin::HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }
    }))
  }

  async fn load(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id.starts_with('\0') {
      return Ok(None);
    }

    if find_special_query(args.id, b"raw").is_some() {
      let path = match check_public_file(args.id, &self.public_dir) {
        Some(f) => Cow::Owned(f.to_string_lossy().into_owned()),
        None => Cow::Borrowed(clean_url(args.id)),
      };

      ctx.add_watch_file(&path);

      let file = serde_json::from_str::<Value>(&std::fs::read_to_string(path.as_ref())?)?;
      let code = arcstr::format!("export default {}", serde_json::to_string(&file)?);
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code,
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }

    if self.is_not_valid_assets(ctx.cwd(), args.id) {
      return Ok(None);
    }

    let id = rolldown_plugin_utils::remove_url_query(args.id);
    // TODO(shulaoda): finish below logic
    let url = file_to_url(
      &FileToUrlEnv {
        root: &ctx.cwd().to_string_lossy(),
        command: "serve",
        url_base: &self.url_base,
        public_dir: &self.public_dir,
      },
      &id,
    )?;

    let url = rolldown_plugin_utils::encode_uri_path(url);
    let code = arcstr::format!("export default {}", serde_json::to_string(&Value::String(url))?);
    Ok(Some(rolldown_plugin::HookLoadOutput {
      code,
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }
}
