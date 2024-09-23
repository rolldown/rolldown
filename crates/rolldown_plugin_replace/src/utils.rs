use std::{collections::HashMap, sync::LazyLock};

use fancy_regex::Regex;

static OBJECT_RE: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"^([_$a-zA-Z\xA0-\uFFFF][_$a-zA-Z0-9\xA0-\uFFFF]*)(\.([_$a-zA-Z\xA0-\uFFFF][_$a-zA-Z0-9\xA0-\uFFFF]*))+$";
  Regex::new(pattern).expect("Should be valid regex")
});

pub(crate) fn expand_typeof_replacements(
  values: &HashMap<String, String>,
) -> HashMap<String, String> {
  let mut replacements: Vec<(String, String)> = Vec::new();

  for key in values.keys() {
    if let Ok(matched) = OBJECT_RE.captures(key) {
      let capture_str = matched.unwrap().get(0).unwrap().as_str();

      let capture_vec: Vec<&str> = capture_str.split('.').collect::<Vec<&str>>();

      let capture_arr = capture_vec.as_slice();

      let replaces: Vec<(String, String)> = capture_arr[0..capture_arr.len() - 1]
        .iter()
        .flat_map(|x| {
          vec![
            (format!("typeof {} ===", *x), "\"object\" ===".to_string()),
            (format!("typeof {}===", *x), "\"object\"===".to_string()),
            (format!("typeof {} !==", *x), "\"object\" !==".to_string()),
            (format!("typeof {}!==", *x), "\"object\"!==".to_string()),
            (format!("typeof {} ==", *x), "\"object\" ===".to_string()),
            (format!("typeof {}==", *x), "\"object\"===".to_string()),
            (format!("typeof {}!=", *x), "\"object\"!==".to_string()),
            (format!("typeof {} !=", *x), "\"object\" !==".to_string()),
          ]
        })
        .collect();
      replacements.extend(replaces);
    };
  }

  HashMap::from_iter(replacements)
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
      ("typeof a===".to_string(), "\"object\"===".to_string()),
      ("typeof a ===".to_string(), "\"object\" ===".to_string()),
      ("typeof a==".to_string(), "\"object\"===".to_string()),
      ("typeof a ==".to_string(), "\"object\" ===".to_string()),
      ("typeof a!==".to_string(), "\"object\"!==".to_string()),
      ("typeof a !==".to_string(), "\"object\" !==".to_string()),
      ("typeof a!=".to_string(), "\"object\"!==".to_string()),
      ("typeof a !=".to_string(), "\"object\" !==".to_string()),
      ("typeof b===".to_string(), "\"object\"===".to_string()),
      ("typeof b ===".to_string(), "\"object\" ===".to_string()),
      ("typeof b==".to_string(), "\"object\"===".to_string()),
      ("typeof b ==".to_string(), "\"object\" ===".to_string()),
      ("typeof b!==".to_string(), "\"object\"!==".to_string()),
      ("typeof b !==".to_string(), "\"object\" !==".to_string()),
      ("typeof b!=".to_string(), "\"object\"!==".to_string()),
      ("typeof b !=".to_string(), "\"object\" !==".to_string()),
      ("typeof c===".to_string(), "\"object\"===".to_string()),
      ("typeof c ===".to_string(), "\"object\" ===".to_string()),
      ("typeof c==".to_string(), "\"object\"===".to_string()),
      ("typeof c ==".to_string(), "\"object\" ===".to_string()),
      ("typeof c!==".to_string(), "\"object\"!==".to_string()),
      ("typeof c !==".to_string(), "\"object\" !==".to_string()),
      ("typeof c!=".to_string(), "\"object\"!==".to_string()),
      ("typeof c !=".to_string(), "\"object\" !==".to_string()),
    ]);

    assert_eq!(result, expected);
  }
}
