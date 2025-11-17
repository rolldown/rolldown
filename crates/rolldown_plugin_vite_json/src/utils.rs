/// /\.json(?:$|\?)(?!commonjs-(?:proxy|external))/
pub fn is_json_ext(ext: &str) -> bool {
  if ext.ends_with(".json") {
    return true;
  }
  let Some(i) = memchr::memmem::rfind(ext.as_bytes(), b".json?") else {
    return false;
  };
  let postfix = &ext[i + 6..];
  postfix != "commonjs-proxy" && postfix != "commonjs-external"
}

#[test]
fn json_ext() {
  assert!(is_json_ext("test.json"));
  assert!(is_json_ext("test.json?test=test&b=100"));
  assert!(is_json_ext("test.json?commonjs-prox"));
  assert!(is_json_ext("test.json?commonjs-externa"));

  assert!(!is_json_ext("test.json?commonjs-proxy"));
  assert!(!is_json_ext("test.json?commonjs-external"));
}
