use std::borrow::Cow;

use html5gum::Span;
use rolldown_plugin::PluginContext;
use rolldown_plugin_utils::constants::{HTMLProxyMap, HTMLProxyMapItem};
use rolldown_utils::{url::clean_url, xxhash::xxhash_with_base};
use string_wizard::MagicString;

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
      span,
      rolldown_utils::concat_string!(
        "__VITE_INLINE_CSS__",
        xxhash_with_base(clean_url(id).as_bytes(), 16),
        "_",
        itoa::Buffer::new().format(*inline_module_count - 1),
        "__"
      ),
    )
  }
}
