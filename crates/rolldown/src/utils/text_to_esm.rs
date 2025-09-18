use serde::Serialize;

/// create `("")` used for create a `StringLiteral` with `oxc`,
/// since a plain `"literal"` will be parsed as `Directive`
/// https://oxc-project.github.io/oxc/playground/?code=3YCAAICGgICAgICAgICRnYgn56xU7FAff34zGIA%3D
#[inline]
pub fn text_to_string_literal(txt: &str) -> anyhow::Result<String> {
  // pre-allocate 2x more bytes for the quotes and potential escapes
  let mut bytes = Vec::with_capacity(txt.len() * 2 + 2);
  let mut ser = serde_json::ser::Serializer::new(&mut bytes);
  txt.serialize(&mut ser)?;
  // SAFETY: serde_json will not produce invalid utf8
  Ok(unsafe { String::from_utf8_unchecked(bytes) })
}
