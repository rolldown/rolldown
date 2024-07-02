pub fn raw_text_to_esm(source: &str) -> String {
  ["export default '", source, "';"].concat()
}
