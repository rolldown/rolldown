use std::{collections::HashMap, sync::LazyLock};

use regex::Regex;

static OBJECT_RE: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"^([_$a-zA-Z\xA0-\uFFFF][_$a-zA-Z0-9\xA0-\uFFFF]*)(\.([_$a-zA-Z\xA0-\uFFFF][_$a-zA-Z0-9\xA0-\uFFFF]*))+$";
  Regex::new(pattern).expect("Should be valid regex")
});

pub(crate) fn expand_typeof_replacements(
  values: &HashMap<String, String>,
) -> Vec<(String, String)> {
  let mut replacements: Vec<(String, String)> = Vec::new();

  for key in values.keys() {
    if OBJECT_RE.is_match(key) {
      // Skip last part
      replacements.extend(key.match_indices('.').map(|(index, _)| {
        let match_str = &key[..index];
        (format!("typeof {match_str}"), "\"object\"".to_string())
      }));
    };
  }

  replacements
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::expand_typeof_replacements;

  fn run_test(keys: &[&str], expected: &[(&str, &str)]) {
    let map = keys.iter().copied().map(|key| (key.to_string(), "x".to_string())).collect();
    let result = expand_typeof_replacements(&map).into_iter().collect::<HashMap<_, _>>();
    let expected = expected
      .iter()
      .copied()
      .map(|(key, replacement)| (key.to_string(), replacement.to_string()))
      .collect::<HashMap<_, _>>();
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
