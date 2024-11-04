pub fn text_to_string_literal(txt: &str) -> anyhow::Result<String> {
  Ok(serde_json::to_string(txt)?)
}
