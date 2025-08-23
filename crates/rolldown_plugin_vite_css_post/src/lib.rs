mod utils;

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use cow_utils::CowUtils;
use rolldown_common::{EmittedAsset, ModuleType, side_effects::HookSideEffects};
use rolldown_plugin::{HookTransformOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{
  AssetUrlResult, RenderBuiltUrl, RenderBuiltUrlConfig, ToOutputFilePathInJSEnv,
  constants::{CSSModuleCache, CSSStyles, HTMLProxyResult, ViteMetadata},
  create_to_import_meta_url_based_relative_runtime,
  css::is_css_request,
  data_to_esm, find_special_query, is_special_query,
  uri::encode_uri_path,
};
use rolldown_utils::{futures::block_on_spawn_all, url::clean_url, xxhash::xxhash_with_base};

use crate::utils::UrlEmitTasks;

#[derive(derive_more::Debug)]
pub struct ViteCssPostPlugin {
  pub is_ssr: bool,
  pub is_worker: bool,
  pub css_minify: bool,
  pub url_base: String,
  pub decoded_base: String,
  #[debug(skip)]
  pub render_built_url: Option<Arc<RenderBuiltUrl>>,
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

    let mut code = string_wizard::MagicString::new(&args.code);
    let mut css_url_iter = args.code.match_indices("__VITE_CSS_URL__").peekable();
    if css_url_iter.peek().is_some() {
      let vite_metadata = ctx.meta().get::<ViteMetadata>().expect("ViteMetadata missing");
      let env = ToOutputFilePathInJSEnv {
        url_base: &self.url_base,
        decoded_base: &self.decoded_base,
        render_built_url: self.render_built_url.as_deref(),
        render_built_url_config: RenderBuiltUrlConfig {
          r#type: "asset",
          is_ssr: self.is_ssr,
          host_id: &args.chunk.filename,
          host_type: "js",
        },
      };
      let tasks = css_url_iter.map(async |(index, _)| {
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

        let reference_id = ctx
          .emit_file_async(EmittedAsset {
            name: css_asset_name,
            source: content.into(),
            original_file_name: Some(original_file_name),
            ..Default::default()
          })
          .await?;

        let filename = ctx.get_file_name(&reference_id)?;
        vite_metadata.imported_assets.insert(clean_url(&reference_id).to_string());

        let url = env
          .to_output_file_path_in_js(
            &filename,
            create_to_import_meta_url_based_relative_runtime(ctx.options().format, self.is_worker),
          )
          .await?;

        let replacement = match url {
          AssetUrlResult::WithRuntime(v) => rolldown_utils::concat_string!("\"+", v, "+\""),
          AssetUrlResult::WithoutRuntime(v) => {
            let string = serde_json::to_string(&encode_uri_path(v))?;
            string[1..string.len() - 1].to_owned()
          }
        };

        Ok(utils::UrlEmitTasks { range: (index, index + pos + 2), replacement })
      });

      for task in block_on_spawn_all(tasks).await {
        let UrlEmitTasks { range: (start, end), replacement } = task?;
        code.update(start, end, replacement);
      }
    }

    Ok(None)
  }
}
