use std::{borrow::Cow, path::Path, sync::Arc};

use html5gum::Span;
use rolldown_common::OutputChunk;
use rolldown_plugin::PluginContext;
use rolldown_plugin_utils::{
  AssetUrlItem, AssetUrlIter, AssetUrlResult, PublicAssetUrlCache, ToOutputFilePathEnv,
  constants::{HTMLProxyMap, HTMLProxyMapItem, HTMLProxyResult, ViteMetadata},
  uri::encode_uri_path,
};
use rolldown_utils::{pattern_filter::normalize_path, url::clean_url, xxhash::xxhash_with_base};
use string_wizard::MagicString;
use sugar_path::SugarPath as _;

use crate::ViteHtmlPlugin;

impl ViteHtmlPlugin {
  pub fn get_base_in_html(&self, url_relative_path: &str) -> Cow<'_, str> {
    if self.url_base.is_empty() || self.url_base == "./" {
      let count = url_relative_path.matches('/').count();
      Cow::Owned(if count == 0 { "./".to_owned() } else { "../".repeat(count) })
    } else {
      Cow::Borrowed(self.url_base.as_ref())
    }
  }

  #[inline]
  pub fn add_to_html_proxy_cache(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    file: String,
    index: usize,
    result: HTMLProxyMapItem,
  ) {
    ctx
      .meta()
      .get_or_insert_default::<HTMLProxyMap>()
      .inner
      .entry(file)
      .or_default()
      .insert(index, result);
  }

  #[expect(clippy::too_many_arguments)]
  pub fn handle_style_tag_or_attribute(
    &self,
    s: &mut MagicString,
    js: &mut String,
    id: &str,
    ctx: &PluginContext,
    file_path: String,
    inline_module_count: &mut usize,
    is_style_attribute: bool,
    (value, span): (&str, Span),
  ) -> anyhow::Result<()> {
    *inline_module_count += 1;

    js.push_str("import \"");
    js.push_str(id);
    js.push_str("?html-proxy&inline-css");
    if is_style_attribute {
      js.push_str("&style-attr");
    }
    js.push_str("&index=");
    js.push_str(itoa::Buffer::new().format(*inline_module_count - 1));
    js.push_str(".css\"\n");

    self.add_to_html_proxy_cache(
      ctx,
      file_path,
      *inline_module_count,
      HTMLProxyMapItem { code: value.into(), map: None },
    );

    super::overwrite_check_public_file(
      s,
      span.start..span.end,
      rolldown_utils::concat_string!(
        "__VITE_INLINE_CSS__",
        xxhash_with_base(clean_url(id).as_bytes(), 16),
        "_",
        itoa::Buffer::new().format(*inline_module_count - 1),
        "__"
      ),
    )
  }

  pub async fn url_to_built_url(
    &self,
    ctx: &PluginContext,
    url: &str,
    importer: &str,
    force_inline: Option<bool>,
  ) -> anyhow::Result<String> {
    if rolldown_plugin_utils::check_public_file(url, &self.public_dir).is_some() {
      let env = rolldown_plugin_utils::PublicFileToBuiltUrlEnv::new(ctx);
      return Ok(env.public_file_to_built_url(url));
    }
    let path = if url.starts_with('/') {
      ctx.cwd().join(url)
    } else {
      Path::new(importer).parent().unwrap().join(url)
    };
    let path = path.normalize();
    let env = rolldown_plugin_utils::FileToUrlEnv {
      ctx,
      root: ctx.cwd(),
      is_lib: self.is_lib,
      public_dir: &self.public_dir,
      asset_inline_limit: &self.asset_inline_limit,
    };
    env.file_to_built_url(&path.to_string_lossy(), true, force_inline).await
  }

  pub fn handle_inline_css<'a>(ctx: &PluginContext, html: &'a str) -> Option<MagicString<'a>> {
    let mut s = None;
    for (start, _) in "__VITE_INLINE_CSS__".match_indices(html) {
      let prefix_end = start + 19; // "__VITE_INLINE_CSS__".len()
      let bytes = html.as_bytes();

      // Match pattern: __VITE_INLINE_CSS__([a-z\d]{8}_\d+)__
      // Check 8 hex characters [a-z\d]{8}
      let hex_count = bytes[prefix_end..]
        .iter()
        .take_while(|&&b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        .count();

      if hex_count != 8 {
        continue;
      }

      let after_hex = prefix_end + 8;

      // Check single underscore '_'
      if !html[after_hex..].starts_with('_') {
        continue;
      }

      let after_underscore = after_hex + 1;

      // Count digits '\d+'
      let digit_count =
        bytes[after_underscore..].iter().take_while(|&&b| b.is_ascii_digit()).count();

      if digit_count == 0 {
        continue;
      }

      let after_digits = after_underscore + digit_count;

      // Check ending '__'
      if !html[after_digits..].starts_with("__") {
        continue;
      }

      // Match successful - extract full match and scoped name
      let match_end = after_digits + 2; // Include ending '__'
      let scoped_name = &html[prefix_end..after_digits]; // e.g., "abcd1234_0"

      let s = s.get_or_insert_with(|| string_wizard::MagicString::new(html));

      let cache = ctx.meta().get::<HTMLProxyResult>().expect("HTMLProxyResult missing");
      let css_transformed_code = cache.inner.get(scoped_name).unwrap();
      s.update(start, match_end, css_transformed_code.to_string());
    }
    s
  }

  pub async fn to_output_file_path(
    &self,
    filename: &str,
    assets_base: &str,
    is_public_asset: bool,
    relative_url_path: &str,
  ) -> anyhow::Result<String> {
    let env = ToOutputFilePathEnv {
      is_ssr: self.is_ssr,
      host_id: relative_url_path,
      url_base: &self.url_base,
      decoded_base: &self.decoded_base,
      render_built_url: self.render_built_url.as_deref(),
    };

    if super::is_excluded_url(filename) {
      Ok(filename.to_owned())
    } else {
      env
        .to_output_file_path(filename, "html", is_public_asset, |filename: &Path, _: &Path| {
          AssetUrlResult::WithoutRuntime(rolldown_utils::concat_string!(
            &assets_base,
            filename.to_string_lossy()
          ))
        })
        .await
        .map(rolldown_plugin_utils::AssetUrlResult::to_asset_url_in_css_or_html)
    }
  }

  pub async fn handle_html_asset_url(
    &self,
    ctx: &PluginContext,
    html: &str,
    chunk: Option<&Arc<OutputChunk>>,
    assets_base: &str,
    relative_url_path: &str,
  ) -> anyhow::Result<Option<String>> {
    let mut s = None;
    let mut end = 0;
    for item in AssetUrlIter::from(html).into_asset_url_iter() {
      let s = s.get_or_insert_with(|| String::with_capacity(html.len()));
      match item {
        AssetUrlItem::Asset((range, reference_id, postfix)) => {
          let filename = ctx.get_file_name(reference_id)?;

          if let Some(chunk) = chunk {
            ctx
              .meta()
              .get_or_insert_default::<ViteMetadata>()
              .get_or_insert_default(chunk.preliminary_filename.as_str().into())
              .imported_assets
              .insert(clean_url(&filename).into());
          }

          let uri =
            self.to_output_file_path(&filename, assets_base, false, relative_url_path).await?;

          s.push_str(&html[end..range.start]);
          s.push_str(&encode_uri_path(uri));
          if let Some(postfix) = postfix {
            s.push_str(postfix);
          }
          end = range.end;
        }
        AssetUrlItem::PublicAsset((range, hash)) => {
          let cache = ctx
            .meta()
            .get::<PublicAssetUrlCache>()
            .ok_or_else(|| anyhow::anyhow!("PublicAssetUrlCache missing"))?;

          let filename = cache.0.get(hash).unwrap();
          let uri =
            self.to_output_file_path(&filename, assets_base, true, relative_url_path).await?;

          s.push_str(&html[end..range.start]);
          s.push_str(&encode_uri_path(normalize_path(&uri).into_owned()));
          end = range.end;
        }
      }
    }
    if let Some(s) = &mut s
      && end < html.len()
    {
      s.push_str(&html[end..]);
    }
    Ok(s)
  }
}
