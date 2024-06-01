pub fn text_to_esm(txt: &str) -> anyhow::Result<String> {
  Ok(serde_json::to_string(txt).map(|text| ["export default ", text.as_str(), ";"].concat())?)
}
