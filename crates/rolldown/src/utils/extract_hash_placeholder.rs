/// Match `[hash]` or `[hash:8]` in the template
/// Review it in https://rregex.dev/
static HASH_PLACEHOLDER_RE: once_cell::sync::Lazy<regex::Regex> =
  once_cell::sync::Lazy::new(|| regex::Regex::new(r"\[hash(?::(\d+))?\]").unwrap());

#[derive(Debug, PartialEq, Eq)]
pub struct ExtractedHashPlaceholder {
  pub pattern: String,
  pub len: Option<usize>,
}

pub fn extract_hash_placeholder(template: &str) -> Option<ExtractedHashPlaceholder> {
  let captures = HASH_PLACEHOLDER_RE.captures(template)?;
  let pattern = captures.get(0)?.as_str().to_string();
  let len = captures.get(1).map(|m| m.as_str().parse().unwrap_or(8));

  Some(ExtractedHashPlaceholder { pattern, len })
}

#[test]
fn test_extract_hash_placeholder() {
  assert_eq!(
    extract_hash_placeholder("[hash:8]"),
    Some(ExtractedHashPlaceholder { pattern: "[hash:8]".to_string(), len: Some(8) })
  );
  assert_eq!(
    extract_hash_placeholder("[hash]"),
    Some(ExtractedHashPlaceholder { pattern: "[hash]".to_string(), len: None })
  );
}

// [hash:8]
pub fn generate_facade_replacement_of_hash_placeholder(
  extracted: &ExtractedHashPlaceholder,
) -> String {
  let mut facade = String::new();
  // TODO: improve this
  facade.push('^');
  facade.push_str(&"\0".repeat(extracted.len.unwrap_or(8) - 2));
  facade.push('$');

  facade
}
