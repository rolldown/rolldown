pub fn base64_to_esm(source: &str) -> String {
  ["export default '", source, "';"].concat()
}
