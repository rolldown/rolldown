#[inline]
pub fn strip_bom(code: &str) -> &str {
  code.strip_prefix("\u{FEFF}").unwrap_or(code)
}
