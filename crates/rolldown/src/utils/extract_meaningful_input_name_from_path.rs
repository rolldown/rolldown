use std::path::Path;

pub fn try_extract_meaningful_input_name_from_path(path: impl AsRef<Path>) -> Option<String> {
  let path = path.as_ref();
  let file_name = path.file_stem().and_then(|f| f.to_str()).map(ToString::to_string)?;

  Some(file_name)
}

#[test]
fn test_try_extract_meaningful_input_name_from_path() {
  assert_eq!(
    try_extract_meaningful_input_name_from_path("foo/bar/baz.js"),
    Some("baz".to_string())
  );
  assert_eq!(
    try_extract_meaningful_input_name_from_path("react-dom"),
    Some("react-dom".to_string())
  );
}
