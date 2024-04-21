/// Match `[hash]` or `[hash:8]` in the template
/// Review it in https://rregex.dev/
static HASH_PLACEHOLDER_RE: once_cell::sync::Lazy<regex::Regex> =
  once_cell::sync::Lazy::new(|| regex::Regex::new(r"\[hash(?::(\d+))?\]").unwrap());

#[derive(Debug, PartialEq, Eq)]
pub struct ExtractedHashPattern {
  pub pattern: String,
  pub len: Option<usize>,
}

pub fn extract_hash_pattern(template: &str) -> Option<ExtractedHashPattern> {
  let captures = HASH_PLACEHOLDER_RE.captures(template)?;
  let pattern = captures.get(0)?.as_str().to_string();
  let len = captures.get(1).map(|m| m.as_str().parse().unwrap_or(8));

  Some(ExtractedHashPattern { pattern, len })
}

#[test]
fn test_extract_hash_placeholder() {
  assert_eq!(
    extract_hash_pattern("[hash:8]"),
    Some(ExtractedHashPattern { pattern: "[hash:8]".to_string(), len: Some(8) })
  );
  assert_eq!(
    extract_hash_pattern("[hash]"),
    Some(ExtractedHashPattern { pattern: "[hash]".to_string(), len: None })
  );
}
