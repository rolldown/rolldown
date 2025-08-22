mod utils;

use std::{borrow::Cow, path::PathBuf};

use cow_utils::CowUtils;
use rolldown_common::{ModuleType, side_effects::HookSideEffects};
use rolldown_plugin::{HookTransformOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{
  constants::{CSSModuleCache, CSSStyles, HTMLProxyResult},
  css::is_css_request,
  data_to_esm, find_special_query, is_special_query,
};
use rolldown_utils::{url::clean_url, xxhash::xxhash_with_base};

#[derive(Debug)]
pub struct ViteCssPostPlugin {
  css_minify: bool,
}

impl Plugin for ViteCssPostPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css-post")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::RenderChunk
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
    let mut css = Cow::Borrowed(args.code.trim_start_matches('\u{feff}'));
    let inline_css = find_special_query(args.id, b"inline-css").is_some();
    let is_html_proxy = find_special_query(args.id, b"html-proxy").is_some();

    if inline_css && is_html_proxy {
      if find_special_query(args.id, b"style-attr").is_some() {
        css = Cow::Owned(css.cow_replace('"', "&quot;").into_owned());
      }
      let Some(index) = utils::extract_index(args.id) else {
        return Err(anyhow::anyhow!("HTML proxy index in '{}' not found", args.id));
      };

      let hash = xxhash_with_base(clean_url(args.id).as_bytes(), 16);
      let cache = ctx.meta().get::<HTMLProxyResult>().expect("HTMLProxyResult missing");
      cache.inner.insert(rolldown_utils::concat_string!(hash, "_", index), css.into_owned());
      return Ok(Some(HookTransformOutput {
        code: Some("export default ''".to_owned()),
        ..Default::default()
      }));
    }

    let css_module_cache = ctx.meta().get::<CSSModuleCache>().expect("CSSModuleCache missing");

    let modules = css_module_cache.inner.get(args.id);
    let inlined = find_special_query(args.id, b"inline").is_some();

    let side_effects = if !inlined && modules.is_none() {
      HookSideEffects::NoTreeshake
    } else {
      HookSideEffects::False
    };

    let code = if inlined {
      if self.css_minify {
        todo!()
      }
      rolldown_utils::concat_string!("export default ", serde_json::to_string(&css)?)
    } else {
      let styles = ctx.meta().get::<CSSStyles>().expect("CSSStyles missing");
      styles.inner.insert(args.id.to_string(), css.into_owned());
      if let Some(modules) = modules {
        let data = serde_json::to_value(&*modules)?;
        data_to_esm(&data, true)
      } else {
        String::new()
      }
    };

    Ok(Some(HookTransformOutput {
      code: Some(code),
      side_effects: Some(side_effects),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  #[allow(unused_assignments, unused_variables)]
  async fn render_chunk(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    // Empty if it's a dynamic chunk with only a CSS import
    // Example -> `import('./style.css')` with no other code
    let is_js_chunk_empty = args.code.is_empty() && !args.chunk.is_entry;
    let styles = ctx.meta().get::<CSSStyles>().expect("CSSStyles missing");
    let mut is_pure_css_chunk = args.chunk.exports.is_empty();
    let mut css_chunk = String::new();
    for module_id in &args.chunk.module_ids {
      let id = module_id.resource_id().as_str();
      if let Some(css) = styles.inner.get(id) {
        // `?transform-only` is used for ?url and shouldn't be included in normal CSS chunks
        if find_special_query(id, b"transform-only").is_some() {
          continue;
        }

        // TODO: implement cssScopeTo
        // https://github.com/vitejs/rolldown-vite/blob/c35ec68d/packages/vite/src/node/plugins/css.ts#L661-L667
        // const cssScopeTo = this.getModuleInfo(id)?.meta?.vite?.cssScopeTo
        // if (cssScopeTo && !isCssScopeToRendered(cssScopeTo, renderedModules)) {
        //   continue
        // }

        if rolldown_plugin_utils::css::is_css_module(id) {
          is_pure_css_chunk = false;
        }

        css_chunk.push_str(css.as_str());
      } else if !is_js_chunk_empty {
        // If the chunk has other JS code, it is not a pure CSS chunk
        is_pure_css_chunk = false;
      }
    }

    let tasks = args.code.match_indices("__VITE_CSS_URL__").map(async |(index, _)| {
      let start = index + "__VITE_CSS_URL__".len();
      let Some(pos) = args.code[start..].find("__") else {
        return Err(anyhow::anyhow!(
          "Invalid __VITE_CSS_URL__ in '{}', expected '__VITE_CSS_URL__<base64>__'",
          args.chunk.name
        ));
      };

      let id = unsafe {
        String::from_utf8_unchecked(
          base64_simd::STANDARD
            .decode_to_vec(&args.code[start..start + pos])
            .map_err(|_| anyhow::anyhow!("Invalid base64 in '__VITE_CSS_URL__'"))?,
        )
      };

      let Some(style) = styles.inner.get(&id) else {
        return Err(anyhow::anyhow!("CSS content for  '{}' was not found", id));
      };

      let content = self.resolve_asset_urls_in_css().await;
      let content = self.finalize_css(content).await;

      let original_file_name = clean_url(&id).to_string();
      let css_asset_path = PathBuf::from(&original_file_name).with_extension("css");
      let css_asset_name = css_asset_path.file_name().map(|v| v.to_string_lossy().into_owned());

      Ok(utils::UrlEmitTasks {
        range: (index, index + pos + 2),
        content,
        css_asset_name,
        original_file_name,
      })
    });

    Ok(None)
  }
}
