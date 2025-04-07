use oxc::syntax::identifier;
use rustc_hash::FxHashMap;

/// Checks if a string matches the pattern of an object property access chain
/// (e.g., "process.env.NODE_ENV").
///
/// The pattern requires:
/// 1. A valid identifier at the start
/// 2. One or more dot-separated valid identifiers following it
///
/// Valid identifiers follow JavaScript rules: they start with a letter, underscore,
/// dollar sign, or Unicode character, and can contain numbers in subsequent positions.
fn is_object_property_chain(s: &str) -> bool {
  // Empty string is not a valid object property chain
  if s.is_empty() {
    return false;
  }

  // Split the string by dots
  let parts: Vec<&str> = s.split('.').collect();

  // Must have at least two parts (object.property)
  if parts.len() < 2 {
    return false;
  }

  // Check each part is a valid identifier
  for part in parts {
    // Empty part means there were consecutive dots or a trailing/leading dot
    if part.is_empty() {
      return false;
    }

    // Check first character is valid for identifier start
    let mut chars = part.chars();
    let Some(first) = chars.next() else {
      return false;
    };
    if !identifier::is_identifier_start(first) {
      return false;
    }

    // Check remaining characters are valid for identifier parts
    for c in chars {
      if !identifier::is_identifier_part(c) {
        return false;
      }
    }
  }

  true
}

pub fn expand_typeof_replacements(values: &FxHashMap<String, String>) -> Vec<(String, String)> {
  let mut replacements: Vec<(String, String)> = Vec::new();

  for key in values.keys() {
    if is_object_property_chain(key) {
      // Skip last part
      replacements.extend(key.match_indices('.').map(|(index, _)| {
        let match_str = &key[..index];
        (format!("typeof {match_str}"), "\"object\"".to_string())
      }));
    }
  }

  replacements
}

#[cfg(test)]
mod tests {
  use rustc_hash::FxHashMap;

  use super::expand_typeof_replacements;

  fn run_test(keys: &[&str], expected: &[(&str, &str)]) {
    let map = keys.iter().copied().map(|key| (key.to_string(), "x".to_string())).collect();
    let result = expand_typeof_replacements(&map).into_iter().collect::<FxHashMap<_, _>>();
    let expected = expected
      .iter()
      .copied()
      .map(|(key, replacement)| (key.to_string(), replacement.to_string()))
      .collect::<FxHashMap<_, _>>();
    assert_eq!(result, expected);
  }

  #[test]
  fn test_expand() {
    run_test(&["a"], &[]);
    run_test(&["abc"], &[]);

    run_test(&["abc.def"], &[("typeof abc", "\"object\"")]);

    run_test(
      &["process.env.NODE_ENV"],
      &[("typeof process", "\"object\""), ("typeof process.env", "\"object\"")],
    );

    run_test(
      &["a.b.c.d"],
      &[("typeof a", "\"object\""), ("typeof a.b", "\"object\""), ("typeof a.b.c", "\"object\"")],
    );
  }

  #[test]
  fn test_expand_unicode() {
    run_test(
      &["कुत्तेपरपानी.पतलूनमेंआग.मेरेशॉर्ट्सखाओ"],
      &[("typeof कुत्तेपरपानी", "\"object\""), ("typeof कुत्तेपरपानी.पतलूनमेंआग", "\"object\"")],
    );
  }

  #[test]
  fn test_expand_multiple() {
    run_test(
      &["a.x", "b.y", "c.z", "d.e.f", "g.h"],
      &[
        ("typeof a", "\"object\""),
        ("typeof b", "\"object\""),
        ("typeof c", "\"object\""),
        ("typeof d", "\"object\""),
        ("typeof d.e", "\"object\""),
        ("typeof g", "\"object\""),
      ],
    );
  }

  #[test]
  fn test_expand_invalid() {
    run_test(&[""], &[]);
    run_test(&["~"], &[]);
    run_test(&["."], &[]);
    run_test(&["a."], &[]);
    run_test(&[".a"], &[]);
    run_test(&["a.b."], &[]);
    run_test(&["a.b..c"], &[]);
    run_test(&["!a.b.c"], &[]);
    run_test(&["a!.b.c"], &[]);
    run_test(&["a.!b.c"], &[]);
    run_test(&["a.b!.d"], &[]);
    run_test(&["a.b!c.d"], &[]);
    run_test(&["a.b.!cde"], &[]);
    run_test(&["a.b.cde!"], &[]);
    run_test(&["a.b.c.d!e"], &[]);

    run_test(&["a.x", "!", "b.y"], &[("typeof a", "\"object\""), ("typeof b", "\"object\"")]);
  }
}
