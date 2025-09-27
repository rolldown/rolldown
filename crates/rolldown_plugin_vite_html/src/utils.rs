use std::borrow::Cow;

use html5gum::Span;
use string_wizard::MagicString;

use super::ViteHtmlPlugin;

impl ViteHtmlPlugin {
  pub fn get_base_in_html(&self, url_relative_path: &str) -> Cow<'_, str> {
    if self.url_base.is_empty() || self.url_base == "./" {
      let count = url_relative_path.matches('/').count();
      Cow::Owned(if count == 0 { "./".to_owned() } else { "../".repeat(count) })
    } else {
      Cow::Borrowed(self.url_base.as_ref())
    }
  }
}

pub fn overwrite_check_public_file(
  s: &mut MagicString<'_>,
  span: Span,
  value: String,
) -> anyhow::Result<()> {
  let src = &s.source().as_bytes()[span.start..span.end];
  let Some(start) = src
    .iter()
    .position(|&b| b == b'=')
    .and_then(|i| src[i + 1..].iter().position(|b| !b.is_ascii_whitespace()).map(|p| p + i + 1))
    .map(|pos| span.start + pos)
  else {
    return Err(anyhow::anyhow!("internal error, failed to overwrite attribute value"));
  };
  let pos = src[start - span.start];
  let wrap_offset = usize::from(pos == b'"' || pos == b'\'');
  s.update(start + wrap_offset, span.end - wrap_offset, value);
  Ok(())
}
