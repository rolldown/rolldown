#[derive(Debug, PartialEq, Eq)]
pub struct ExtractedHashPattern<'a> {
  pub pattern: &'a str,
  pub len: Option<usize>,
}

/// Extract `[hash]` or `[hash:8]` in the template
pub fn extract_hash_pattern(pattern: &str) -> Option<ExtractedHashPattern<'_>> {
  let start = pattern.find("[hash")?;
  let end = pattern[start + 5..].find(']')?;
  let len = if let Some(n) = pattern[start + 5..start + 5 + end].strip_prefix(':') {
    Some(n.parse::<usize>().ok()?)
  } else {
    None
  };
  let pattern = &pattern[start..=start + 5 + end];
  Some(ExtractedHashPattern { pattern, len })
}

#[test]
fn test_extract_hash_placeholder() {
  let correct = [("[hash:8]", Some(8)), ("[hash:256]", Some(256)), ("[hash]", None)];
  for (pattern, len) in correct {
    assert_eq!(extract_hash_pattern(pattern), Some(ExtractedHashPattern { pattern, len }),);
  }

  let incorrect = ["[", "[hash", "[hash::8]", "[hash:x]", "hash"];
  for pattern in incorrect {
    assert_eq!(extract_hash_pattern(pattern), None);
  }

  assert_eq!(
    extract_hash_pattern("[name]-[hash].mjs"),
    Some(ExtractedHashPattern { pattern: "[hash]", len: None })
  );

  assert_eq!(
    extract_hash_pattern("[name]-[hash:16].mjs"),
    Some(ExtractedHashPattern { pattern: "[hash:16]", len: Some(16) })
  );
}
