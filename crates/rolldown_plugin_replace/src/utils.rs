use std::{collections::HashMap, sync::LazyLock};

use fancy_regex::Regex;

static OBJECT_RE: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"^([_$a-zA-Z\xA0-\uFFFF][_$a-zA-Z0-9\xA0-\uFFFF]*)(\.([_$a-zA-Z\xA0-\uFFFF][_$a-zA-Z0-9\xA0-\uFFFF]*))+$";
  Regex::new(pattern).expect("Should be valid regex")
});

pub(crate) fn expand_typeof_replacements(
  values: &HashMap<String, String>,
) -> HashMap<String, String> {
  values
    .keys()
    .filter_map(|key| {
      if let Ok(true) = OBJECT_RE.is_match(key) {
        Some(key.char_indices().filter_map(|(pos, c)| {
          if c == '.' {
            Some((format!("typeof {}", &key[0..pos]), "\"object\"".to_string()))
          } else {
            None
          }
        }))
      } else {
        None
      }
    })
    .flatten()
    .collect()
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::expand_typeof_replacements;

  #[test]
  fn test_expand() {
    let map = HashMap::from([("a.b.c.d".to_string(), "x".to_string())]);

    let result = expand_typeof_replacements(&map);

    let expected = HashMap::from([
      ("typeof a".to_string(), "\"object\"".to_string()),
      ("typeof a.b".to_string(), "\"object\"".to_string()),
      ("typeof a.b.c".to_string(), "\"object\"".to_string()),
    ]);

    assert_eq!(result, expected);
  }
}
