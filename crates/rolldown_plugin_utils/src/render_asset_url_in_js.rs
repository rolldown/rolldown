use std::sync::Arc;

use rolldown_utils::url::clean_url;

use crate::{
  PublicAssetUrlCache, ToOutputFilePathInJSEnv,
  constants::ViteMetadata,
  encode_uri_path,
  to_output_file_path_in_js::{AssetUrlResult, RenderBuiltUrl, RenderBuiltUrlConfig},
  to_relative_runtime_path::create_to_import_meta_url_based_relative_runtime,
};

pub struct RenderAssetUrlInJsEnvConfig<'a> {
  pub is_ssr: bool,
  pub is_worker: bool,
  pub base: &'a str,
  pub decoded_base: &'a str,
  pub render_built_url: Option<&'a RenderBuiltUrl>,
}

pub struct RenderAssetUrlInJsEnv<'a> {
  pub code: &'a str,
  pub chunk_filename: &'a str,
  pub config: RenderAssetUrlInJsEnvConfig<'a>,
  pub ctx: &'a rolldown_plugin::PluginContext,
}

impl RenderAssetUrlInJsEnv<'_> {
  #[allow(clippy::too_many_lines)]
  pub async fn render_asset_url_in_js(&self) -> anyhow::Result<Option<String>> {
    // __VITE_ASSET__([\w$]+)__(?:\$_(.*?)__ -> 14 && __VITE_ASSET_PUBLIC__([a-z\d]{8})__ -> 21
    let mut vite_asset_iter = self.code.match_indices("__VITE_ASSET_").peekable();

    if vite_asset_iter.peek().is_none() {
      return Ok(None);
    }

    let mut last = 0;
    let mut code = None;
    for (start, _) in vite_asset_iter {
      last = start;
      let (end, filename, r#type) = if self.code[start + 13..].starts_with('_') {
        let start = start + 14;
        let Some((reference_id, mut end)) =
          self.code[start..].find(|c: char| !c.is_alphanumeric() && c != '&').and_then(|i| {
            self.code[start + i..]
              .starts_with("__")
              .then_some((&self.code[start..start + i], start + i + 2))
          })
        else {
          continue;
        };

        let file = self.ctx.get_file_name(reference_id)?;
        let vite_meta_data = self.ctx.meta().get::<ViteMetadata>().unwrap_or_else(|| {
          let value = Arc::new(ViteMetadata::default());
          self.ctx.meta().insert(Arc::<ViteMetadata>::clone(&value));
          value
        });

        vite_meta_data.imported_assets.insert(clean_url(&file).to_string());

        let postfix = self.code[end..].starts_with("$_").then(|| {
          self.code[end + 2..].find("__").map_or("", |i| {
            let v = &self.code[end + 2..end + 2 + i];
            end = end + 2 + i + 2;
            v
          })
        });

        let filename = if let Some(postfix) = postfix {
          rolldown_utils::concat_string!(file, postfix)
        } else {
          file.to_string()
        };

        (end, filename, "asset")
      } else if self.code[start + 13..].starts_with("PUBLIC__") {
        let start = start + 21;
        let Some((hash, end)) = self.code[start..].find("__").and_then(|i| {
          let hash = &self.code[start..start + i];
          (i == 8 && hash.bytes().all(|b| (b'a'..=b'f').contains(&b) || b.is_ascii_digit()))
            .then_some((hash, start + 8 + 2))
        }) else {
          continue;
        };

        let cache = self
          .ctx
          .meta()
          .get::<PublicAssetUrlCache>()
          .ok_or_else(|| anyhow::anyhow!("PublicAssetUrlCache missing"))?;

        let filename = cache
          .0
          .get(hash)
          .ok_or_else(|| anyhow::anyhow!("Can't find the cache of {}", &self.code[start..end]))?
          .to_string();

        (end, filename, "js")
      } else {
        continue;
      };

      let env = ToOutputFilePathInJSEnv {
        base: self.config.base,
        decoded_base: self.config.decoded_base,
        render_built_url: self.config.render_built_url,
        render_built_url_config: RenderBuiltUrlConfig {
          r#type,
          is_ssr: self.config.is_ssr,
          host_id: self.chunk_filename,
          host_type: "js",
        },
      };

      let url = env
        .to_output_file_path_in_js(
          &filename,
          create_to_import_meta_url_based_relative_runtime(
            self.ctx.options().format,
            self.config.is_worker,
          ),
        )
        .await?;

      let code = code.get_or_insert_with(|| String::with_capacity(self.code.len()));
      code.push_str(&self.code[..last]);
      match url {
        AssetUrlResult::WithRuntime(v) => {
          code.push_str(&rolldown_utils::concat_string!("\"+", v, "+\""));
        }
        AssetUrlResult::WithoutRuntime(v) => {
          let string = serde_json::to_string(&encode_uri_path(v))?;
          code.push_str(&string[1..string.len() - 1]);
        }
      }
      last = end;
    }

    if let Some(code) = &mut code {
      if last < self.code.len() {
        code.push_str(&self.code[last..]);
      }
    }

    Ok(code)
  }
}
