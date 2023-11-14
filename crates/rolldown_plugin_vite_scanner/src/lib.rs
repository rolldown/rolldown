use once_cell::sync::Lazy;
use regex::Regex;
use rolldown::{
  HookLoadArgs, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn,
  Plugin, PluginContext,
};
use std::borrow::Cow;

static HTTP_URL_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^(https?:)?\/\/").expect("Init HTTP_URL_REGEX failed"));
static DATA_URL_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"^\s*data:").expect("Init DATA_URL_REGEX failed"));
static VIRTUAL_MODULE_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r#"^virtual-module:.*"#).expect("Init VIRTUAL_MODULE_REGEX failed"));
static VITE_SPECIAL_QUERY_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r"[?&](?:worker|sharedworker|raw|url)\b")
    .expect("Init VITE_SPECIAL_QUERY_REGEX failed")
});
static CSS_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r"\.(css|less|sass|scss|styl|stylus|pcss|postcss|sss)(?:$|\?)")
    .expect("Init CSS_REGEX failed")
});
static JSON_AND_WASM_REGEX: Lazy<Regex> =
  Lazy::new(|| Regex::new(r"\.(json|json5|wasm)$").expect("Init JSON_AND_WASM_REGEX failed"));
static KNOWN_ASSET_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r"\.(apng|png|jpe?g|jfif|pjpeg|pjp|gif|svg|ico|webp|avif|mp4|webm|ogg|mp3|wav|flac|aac|opus|woff2?|eot|ttf|otf|webmanifest|pdf|txt)$").expect("Init KNOWN_ASSET_REGEX failed")
});
static HTML_TYPE_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r"\.(html|vue|svelte|astro|imba)$").expect("Init HTML_TYPE_REGEX failed")
});

#[allow(clippy::empty_structs_with_brackets)]
#[derive(Debug)]
pub struct ViteScannerPlugin {
  pub entries: Vec<String>,
}

impl ViteScannerPlugin {}

#[async_trait::async_trait]
impl Plugin for ViteScannerPlugin {
  fn name(&self) -> Cow<'static, str> {
    "rolldown_plugin_vite_scanner".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &mut PluginContext,
    args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    let HookResolveIdArgs { source, .. } = args;

    // External http url or data url
    if HTTP_URL_REGEX.is_match(source) || DATA_URL_REGEX.is_match(source) {
      return Ok(Some(HookResolveIdOutput { id: (*source).to_string(), external: Some(true) }));
    }

    // local scripts (`<script>` in Svelte and `<script setup>` in Vue)
    if VIRTUAL_MODULE_REGEX.is_match(source) {
      return Ok(Some(HookResolveIdOutput {
        // strip prefix to get valid filesystem path so bundler can resolve imports in the file
        id: source.replace("virtual-module:", ""),
        external: None,
      }));
    }

    // TODO bare imports: record and externalize

    // Externalized file types
    // they are done after the bare import resolve because a package name
    // may end with these extensions

    // css
    if CSS_REGEX.is_match(source)
      // json & wasm
      || JSON_AND_WASM_REGEX.is_match(source)
      // known asset types
      || KNOWN_ASSET_REGEX.is_match(source)
    {
      return Ok(Some(HookResolveIdOutput {
        id: (*source).to_string(),
        external: Some(self.entries.contains(&(*source).to_string())),
      }));
    }

    // known vite query types: ?worker, ?raw
    if VITE_SPECIAL_QUERY_REGEX.is_match(source) {
      return Ok(Some(HookResolveIdOutput { id: (*source).to_string(), external: Some(true) }));
    }

    Ok(None)
  }

  async fn load(&self, _ctx: &mut PluginContext, args: &HookLoadArgs) -> HookLoadReturn {
    let HookLoadArgs { id } = args;

    // extract scripts inside HTML-like files and treat it as a js module
    if HTML_TYPE_REGEX.is_match(id) {
      // TODO
    }

    Ok(None)
  }
}
