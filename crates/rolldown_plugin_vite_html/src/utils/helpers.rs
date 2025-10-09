use std::ops::Range;

use string_wizard::MagicString;

pub fn overwrite_check_public_file(
  s: &mut MagicString<'_>,
  span: Range<usize>,
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

pub fn is_excluded_url(url: &str) -> bool {
  url.starts_with('#')
    || {
      let b = url.as_bytes();
      if b.starts_with(b"//") {
        return true;
      }
      let mut i = 0;
      while i < b.len() && b[i].is_ascii_lowercase() {
        i += 1;
      }
      i > 0 && i + 2 < b.len() && &b[i..i + 3] == b"://"
    }
    || url.trim_start().get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:"))
}
