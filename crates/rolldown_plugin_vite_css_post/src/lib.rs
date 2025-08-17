mod utils;

use std::borrow::Cow;

use cow_utils::CowUtils;
use rolldown_plugin::{HookTransformOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{
  constants::HTMLProxyResult,
  css::{is_css_request, is_special_query},
  find_special_query,
};
use rolldown_utils::{url::clean_url, xxhash::xxhash_with_base};

#[derive(Debug)]
pub struct ViteCssPostPlugin;

impl Plugin for ViteCssPostPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css-post")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !is_css_request(args.id) || is_special_query(args.id) {
      return Ok(None);
    }

    // strip bom tag
    let mut code = Cow::Borrowed(args.code.trim_start_matches('\u{feff}'));
    let inline_css = find_special_query(args.id, b"inline-css").is_some();
    let is_html_proxy = find_special_query(args.id, b"html-proxy").is_some();

    if inline_css && is_html_proxy {
      if find_special_query(args.id, b"style-attr").is_some() {
        code = Cow::Owned(code.cow_replace('"', "&quot;").into_owned());
      }
      let Some(index) = utils::extract_index(args.id) else {
        return Err(anyhow::anyhow!("HTML proxy index in '{}' not found", args.id));
      };

      let hash = xxhash_with_base(clean_url(args.id).as_bytes(), 16);
      let cache = ctx.meta().get::<HTMLProxyResult>().expect("HTMLProxyResult missing");
      cache.inner.insert(rolldown_utils::concat_string!(hash, "_", index), code.into_owned());
      return Ok(Some(HookTransformOutput {
        code: Some("export default ''".to_owned()),
        ..Default::default()
      }));
    }
    Ok(None)
  }
}
