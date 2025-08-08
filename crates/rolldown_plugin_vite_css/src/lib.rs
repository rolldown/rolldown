mod utils;

use rolldown_plugin::{HookLoadOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{find_special_query, inject_query, remove_special_query};

#[derive(Debug)]
pub struct ViteCssPlugin;

impl Plugin for ViteCssPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }

  async fn load(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if utils::is_css_request(args.id) && find_special_query(args.id, b"url").is_some() {
      if utils::is_css_module(args.id) {
        return Err(anyhow::anyhow!(
          "?url is not supported with CSS modules. (tried to import '{}')",
          args.id
        ));
      }

      let url = remove_special_query(args.id, b"url");
      let code = rolldown_utils::concat_string!(
        "import ",
        serde_json::to_string(&inject_query(&url, "transform-only"))?,
        "; export default '__VITE_CSS_URL__",
        base64_simd::STANDARD.encode_to_string(url.as_bytes()),
        "__'"
      );
      return Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }));
    }
    Ok(None)
  }
}
