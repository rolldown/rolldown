mod utils;

use rolldown_plugin::{HookLoadOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{
  PublicFileToBuiltUrlEnv, check_public_file, find_special_query, inject_query,
  remove_special_query, uri::decode_uri,
};

#[derive(Debug)]
pub struct ViteCssPlugin {
  pub public_dir: String,
}

impl Plugin for ViteCssPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load | HookUsage::Transform
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

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !utils::is_css_request(args.id)
      || ["commonjs-proxy", "worker", "sharedworker", "raw", "url"]
        .iter()
        .any(|query| find_special_query(args.id, query.as_bytes()).is_some())
    {
      return Ok(None);
    }

    #[allow(clippy::no_effect_underscore_binding)]
    let _url_resolver = |url: &str, _importer: Option<&str>| -> (String, Option<String>) {
      let decoded_url = decode_uri(url);
      if check_public_file(&decoded_url, &self.public_dir).is_some() {
        let env = PublicFileToBuiltUrlEnv::new(&ctx);
        return (env.public_file_to_built_url(&decoded_url), None);
      }
      todo!();
    };

    todo!()
  }
}
