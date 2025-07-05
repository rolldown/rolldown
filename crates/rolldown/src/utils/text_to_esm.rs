/// create `("")` used for create a `StringLiteral` with `oxc`,
/// since a plain `"literal"` will be parsed as `Directive`
/// https://oxc-project.github.io/oxc/playground/?code=3YCAAICGgICAgICAgICRnYgn56xU7FAff34zGIA%3D
pub fn text_to_string_literal(txt: &str) -> anyhow::Result<String> {
  Ok(serde_json::to_string(txt)?)
}
