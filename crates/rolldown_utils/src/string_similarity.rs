/// Calculate the Levenshtein distance between two strings.
/// This is used to find similar names for error messages.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
  let a_len = a.chars().count();
  let b_len = b.chars().count();

  if a_len == 0 {
    return b_len;
  }
  if b_len == 0 {
    return a_len;
  }

  let mut prev_row: Vec<usize> = (0..=b_len).collect();
  let mut curr_row = vec![0; b_len + 1];

  for (i, a_char) in a.chars().enumerate() {
    curr_row[0] = i + 1;

    for (j, b_char) in b.chars().enumerate() {
      let cost = if a_char == b_char { 0 } else { 1 };
      curr_row[j + 1] = (curr_row[j] + 1) // insertion
        .min(prev_row[j + 1] + 1) // deletion
        .min(prev_row[j] + cost); // substitution
    }

    std::mem::swap(&mut prev_row, &mut curr_row);
  }

  prev_row[b_len]
}

/// Find the most similar strings from a list of candidates.
/// Returns up to `max_results` candidates sorted by similarity (most similar first).
pub fn find_similar_str<'a>(
  target: &str,
  candidates: impl IntoIterator<Item = &'a str>,
  max_results: usize,
) -> Vec<&'a str> {
  if target.is_empty() {
    return vec![];
  }

  let mut scored: Vec<(&str, usize)> = candidates
    .into_iter()
    .map(|candidate| {
      let distance = levenshtein_distance(target, candidate);
      (candidate, distance)
    })
    .collect();

  // Only include candidates that have a reasonable similarity
  // Use a threshold based on the target length:
  // - For short strings (<=3 chars): distance <= 2
  // - For longer strings: distance <= target.len() / 2
  let max_distance = if target.len() <= 3 { 2 } else { target.len() / 2 };
  scored.retain(|(_, distance)| *distance <= max_distance);

  // Sort by distance (ascending) and then by name (for stable ordering)
  scored.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)));

  scored.into_iter().take(max_results).map(|(name, _)| name).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_levenshtein_distance() {
    assert_eq!(levenshtein_distance("", ""), 0);
    assert_eq!(levenshtein_distance("abc", "abc"), 0);
    assert_eq!(levenshtein_distance("", "abc"), 3);
    assert_eq!(levenshtein_distance("abc", ""), 3);
    assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    assert_eq!(levenshtein_distance("saturday", "sunday"), 3);
  }

  #[test]
  fn test_find_similar_str() {
    let candidates = vec!["foo", "bar", "baz", "foobar"];
    // "fo" (len 2) -> distance <= 2: "foo" (1)
    assert_eq!(find_similar_str("fo", candidates.clone(), 3), vec!["foo"]);
    // "fooo" (len 4) -> distance <= 2: "foo" (1)
    assert_eq!(find_similar_str("fooo", candidates.clone(), 3), vec!["foo"]);
    // "ba" (len 2) -> distance <= 2: "bar" (1), "baz" (1)
    assert_eq!(find_similar_str("ba", candidates.clone(), 3), vec!["bar", "baz"]);
    
    // Test with actual typo: "baz" when looking for "baz" should find exact match
    let candidates2 = vec!["foo", "baz"];
    assert_eq!(find_similar_str("baz", candidates2.clone(), 3), vec!["baz"]);
    
    // Test with typo: "bax" -> "baz" (distance 1)
    assert_eq!(find_similar_str("bax", candidates2.clone(), 3), vec!["baz"]);
  }

  #[test]
  fn test_find_similar_str_limits_results() {
    let candidates = vec!["foo", "for", "fou"];
    let results = find_similar_str("foe", candidates, 2);
    assert_eq!(results.len(), 2);
  }
}
