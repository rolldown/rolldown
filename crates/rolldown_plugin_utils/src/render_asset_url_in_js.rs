use rolldown_utils::url::clean_url;

use super::{
  AssetUrlItem, AssetUrlIter, PublicAssetUrlCache, ToOutputFilePathEnv, constants::ViteMetadata,
  to_relative_runtime_path::create_to_import_meta_url_based_relative_runtime,
};

pub struct RenderAssetUrlInJsEnv<'a> {
  pub code: &'a str,
  pub is_worker: bool,
  pub env: &'a ToOutputFilePathEnv<'a>,
  pub ctx: &'a rolldown_plugin::PluginContext,
}

impl RenderAssetUrlInJsEnv<'_> {
  pub async fn render_asset_url_in_js(&self) -> anyhow::Result<Option<String>> {
    let mut last = 0;
    let mut code = None;
    for item in AssetUrlIter::from(self.code).into_asset_url_iter() {
      let (range, filename, is_public_asset) = match item {
        AssetUrlItem::Asset((range, reference_id, postfix)) => {
          let file = self.ctx.get_file_name(reference_id)?;
          let vite_metadata = self.ctx.meta().get_or_insert_default::<ViteMetadata>();
          let chunk_metadata = vite_metadata.get_or_insert_default(self.env.host_id.into());

          chunk_metadata.imported_assets.insert(clean_url(&file).into());

          let filename = if let Some(postfix) = postfix {
            rolldown_utils::concat_string!(file, postfix)
          } else {
            file.to_string()
          };

          (range, filename, false)
        }
        AssetUrlItem::PublicAsset((range, hash)) => {
          let cache =
            self.ctx.meta().get::<PublicAssetUrlCache>().expect("PublicAssetUrlCache missing");

          let url = cache.0.get(hash).ok_or_else(|| {
            anyhow::anyhow!("Can't find the cache of {}", &self.code[range.start..range.end])
          })?;

          (range, url[1..].to_owned(), true)
        }
      };

      let url = self
        .env
        .to_output_file_path(
          &filename,
          "js",
          is_public_asset,
          create_to_import_meta_url_based_relative_runtime(
            self.ctx.options().format,
            self.is_worker,
          ),
        )
        .await?;

      let code = code.get_or_insert_with(|| String::with_capacity(self.code.len()));
      code.push_str(&self.code[last..range.start]);
      code.push_str(&url.to_asset_url_in_js()?);
      last = range.end;
    }

    if let Some(code) = &mut code {
      if last < self.code.len() {
        code.push_str(&self.code[last..]);
      }
    }

    Ok(code)
  }
}
