#[derive(Debug, PartialEq, Eq)]
pub struct ExtractedHashPattern<'a> {
  pub pattern: &'a str,
  pub len: Option<usize>,
}

/// Replace all `[hash]` or `[hash:8]` in the pattern
pub fn extract_hash_patterns(pattern: &str) -> Option<Vec<ExtractedHashPattern<'_>>> {
  let placeholder = "[hash]";
  let offset = placeholder.len() - 1;
  let mut iter = pattern.match_indices(&placeholder[..offset]).peekable();

  iter.peek()?;

  let mut ending = 0;
  let mut result = vec![];

  for (start, _) in iter {
    if start < ending {
      continue;
    }

    let start_offset = start + offset;
    if let Some(end) = pattern[start_offset..].find(']') {
      let end = start_offset + end;
      let len = pattern[start_offset..end].strip_prefix(':').and_then(|n| n.parse::<usize>().ok());

      result.push(ExtractedHashPattern { pattern: &pattern[start..=end], len });

      ending = end + 1;
    }
  }

  Some(result)
}

#[test]
fn test_extract_hash_patterns() {
  let pattern = "[name]-[hash]-[hash:16].mjs";
  let extracted = extract_hash_patterns(pattern);
  assert_eq!(
    extracted,
    Some(vec![
      ExtractedHashPattern { pattern: "[hash]", len: None },
      ExtractedHashPattern { pattern: "[hash:16]", len: Some(16) }
    ])
  );

  let pattern = "[name].mjs";
  let extracted = extract_hash_patterns(pattern);
  assert!(extracted.is_none());
}
