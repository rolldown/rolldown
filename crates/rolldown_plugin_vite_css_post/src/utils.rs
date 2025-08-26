use std::{path::PathBuf, sync::Arc};

use rolldown_common::EmittedAsset;
use rolldown_plugin::{HookRenderChunkArgs, PluginContext};
use rolldown_plugin_utils::{
  AssetUrlResult, RenderBuiltUrlConfig, ToOutputFilePathInJSEnv,
  constants::{CSSBundleName, CSSStyles, ViteMetadata},
  create_to_import_meta_url_based_relative_runtime,
  uri::encode_uri_path,
};
use rolldown_utils::{futures::block_on_spawn_all, url::clean_url};
use string_wizard::MagicString;

use crate::ViteCssPostPlugin;

pub const DEFAULT_CSS_BUNDLE_NAME: &str = "style.css";

pub fn extract_index(id: &str) -> Option<&str> {
  let s = id.split_once("&index=")?.1;
  let end = s.as_bytes().iter().take_while(|b| b.is_ascii_digit()).count();
  (end > 0).then_some(&s[..end])
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct UrlEmitTasks {
  pub range: (usize, usize),
  pub replacement: String,
}

impl ViteCssPostPlugin {
  pub async fn finalize_vite_css_urls<'a>(
    &self,
    ctx: &PluginContext,
    args: &'a HookRenderChunkArgs<'_>,
    css_styles: &CSSStyles,
    magic_string: &mut Option<MagicString<'a>>,
  ) -> anyhow::Result<()> {
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

        let Some(style) = css_styles.inner.get(&id) else {
          return Err(anyhow::anyhow!("CSS content for  '{}' was not found", id));
        };

        let content = self.resolve_asset_urls_in_css(&style).await;
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
        vite_metadata.imported_assets.insert(clean_url(&reference_id).into());

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

        Ok(UrlEmitTasks { range: (index, index + pos + 2), replacement })
      });

      let magic_string =
        magic_string.get_or_insert_with(|| string_wizard::MagicString::new(&args.code));
      for task in block_on_spawn_all(tasks).await {
        let UrlEmitTasks { range: (start, end), replacement } = task?;
        magic_string.update(start, end, replacement);
      }
    }
    Ok(())
  }

  #[allow(clippy::unused_async)]
  pub async fn resolve_asset_urls_in_css(&self, _content: &str) -> String {
    todo!()
  }

  #[allow(clippy::unused_async)]
  pub async fn finalize_css(&self, _content: String) -> String {
    todo!()
  }

  pub fn get_css_bundle_name(&self, ctx: &PluginContext) -> anyhow::Result<String> {
    Ok(if let Some(css_asset_name) = ctx.meta().get::<CSSBundleName>() {
      css_asset_name.0.clone()
    } else {
      let css_bundle_name = if self.is_lib {
        if let Some(lib_css_filename) = &self.lib_css_filename {
          lib_css_filename.to_owned()
        } else {
          let mut base_dir = ctx.cwd().to_owned();
          loop {
            let pkg_path = base_dir.join("package.json");
            if pkg_path.is_file() {
              let json = std::fs::read_to_string(&pkg_path)?;
              let json = json.trim_start_matches("\u{feff}");
              let raw_json = serde_json::from_str::<serde_json::Value>(json)?;
              if let Some(json_object) = raw_json.as_object() {
                break json_object
                  .get("name")
                  .and_then(|field| field.as_str())
                  .map(ToString::to_string)
                  .ok_or_else(|| anyhow::anyhow!("Name in package.json is required if option 'build.lib.cssFileName' is not provided."))?;
              }
            }
            base_dir = match base_dir.parent() {
              Some(next) => next.to_path_buf(),
              None => {
                return Err(anyhow::anyhow!(
                  "Didn't find the nearest package.json when determining the library CSS bundle name.",
                ));
              }
            };
          }
        }
      } else {
        DEFAULT_CSS_BUNDLE_NAME.to_owned()
      };
      ctx.meta().insert(Arc::new(CSSBundleName(css_bundle_name.clone())));
      css_bundle_name
    })
  }
}
