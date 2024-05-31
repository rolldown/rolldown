pub fn text_to_esm(txt: &str) -> String {
  "export default ".to_owned() + serde_json::to_string(txt).ok().unwrap().as_str() + ";"
}
