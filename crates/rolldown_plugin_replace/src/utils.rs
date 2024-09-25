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
    if let Ok(Some(matched)) = OBJECT_RE.captures(key) {
      let capture_str = matched.get(0).unwrap().as_str();

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

  fn run_test(keys: &[&str], expected: &[(&str, &str)]) {
    let map = keys.iter().copied().map(|key| (key.to_string(), "x".to_string())).collect();
    let result = expand_typeof_replacements(&map);
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

    run_test(
      &["abc.def"],
      &[
        ("typeof abc===", "\"object\"==="),
        ("typeof abc ===", "\"object\" ==="),
        ("typeof abc==", "\"object\"==="),
        ("typeof abc ==", "\"object\" ==="),
        ("typeof abc!==", "\"object\"!=="),
        ("typeof abc !==", "\"object\" !=="),
        ("typeof abc!=", "\"object\"!=="),
        ("typeof abc !=", "\"object\" !=="),
      ],
    );

    run_test(
      &["a.b.c.d"],
      &[
        ("typeof a===", "\"object\"==="),
        ("typeof a ===", "\"object\" ==="),
        ("typeof a==", "\"object\"==="),
        ("typeof a ==", "\"object\" ==="),
        ("typeof a!==", "\"object\"!=="),
        ("typeof a !==", "\"object\" !=="),
        ("typeof a!=", "\"object\"!=="),
        ("typeof a !=", "\"object\" !=="),
        ("typeof b===", "\"object\"==="),
        ("typeof b ===", "\"object\" ==="),
        ("typeof b==", "\"object\"==="),
        ("typeof b ==", "\"object\" ==="),
        ("typeof b!==", "\"object\"!=="),
        ("typeof b !==", "\"object\" !=="),
        ("typeof b!=", "\"object\"!=="),
        ("typeof b !=", "\"object\" !=="),
        ("typeof c===", "\"object\"==="),
        ("typeof c ===", "\"object\" ==="),
        ("typeof c==", "\"object\"==="),
        ("typeof c ==", "\"object\" ==="),
        ("typeof c!==", "\"object\"!=="),
        ("typeof c !==", "\"object\" !=="),
        ("typeof c!=", "\"object\"!=="),
        ("typeof c !=", "\"object\" !=="),
      ],
    );
  }

  #[test]
  fn test_expand_unicode() {
    run_test(
      &["कुत्तेपरपानी.पतलूनमेंआग.मेरेशॉर्ट्सखाओ"],
      &[
        ("typeof कुत्तेपरपानी===", "\"object\"==="),
        ("typeof कुत्तेपरपानी ===", "\"object\" ==="),
        ("typeof कुत्तेपरपानी==", "\"object\"==="),
        ("typeof कुत्तेपरपानी ==", "\"object\" ==="),
        ("typeof कुत्तेपरपानी!==", "\"object\"!=="),
        ("typeof कुत्तेपरपानी !==", "\"object\" !=="),
        ("typeof कुत्तेपरपानी!=", "\"object\"!=="),
        ("typeof कुत्तेपरपानी !=", "\"object\" !=="),
        ("typeof पतलूनमेंआग===", "\"object\"==="),
        ("typeof पतलूनमेंआग ===", "\"object\" ==="),
        ("typeof पतलूनमेंआग==", "\"object\"==="),
        ("typeof पतलूनमेंआग ==", "\"object\" ==="),
        ("typeof पतलूनमेंआग!==", "\"object\"!=="),
        ("typeof पतलूनमेंआग !==", "\"object\" !=="),
        ("typeof पतलूनमेंआग!=", "\"object\"!=="),
        ("typeof पतलूनमेंआग !=", "\"object\" !=="),
      ],
    );
  }

  #[test]
  fn test_expand_multiple() {
    run_test(
      &["a.x", "b.y"],
      &[
        ("typeof a===", "\"object\"==="),
        ("typeof a ===", "\"object\" ==="),
        ("typeof a==", "\"object\"==="),
        ("typeof a ==", "\"object\" ==="),
        ("typeof a!==", "\"object\"!=="),
        ("typeof a !==", "\"object\" !=="),
        ("typeof a!=", "\"object\"!=="),
        ("typeof a !=", "\"object\" !=="),
        ("typeof b===", "\"object\"==="),
        ("typeof b ===", "\"object\" ==="),
        ("typeof b==", "\"object\"==="),
        ("typeof b ==", "\"object\" ==="),
        ("typeof b!==", "\"object\"!=="),
        ("typeof b !==", "\"object\" !=="),
        ("typeof b!=", "\"object\"!=="),
        ("typeof b !=", "\"object\" !=="),
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

    run_test(
      &["a.x", "!", "b.y"],
      &[
        ("typeof a===", "\"object\"==="),
        ("typeof a ===", "\"object\" ==="),
        ("typeof a==", "\"object\"==="),
        ("typeof a ==", "\"object\" ==="),
        ("typeof a!==", "\"object\"!=="),
        ("typeof a !==", "\"object\" !=="),
        ("typeof a!=", "\"object\"!=="),
        ("typeof a !=", "\"object\" !=="),
        ("typeof b===", "\"object\"==="),
        ("typeof b ===", "\"object\" ==="),
        ("typeof b==", "\"object\"==="),
        ("typeof b ==", "\"object\" ==="),
        ("typeof b!==", "\"object\"!=="),
        ("typeof b !==", "\"object\" !=="),
        ("typeof b!=", "\"object\"!=="),
        ("typeof b !=", "\"object\" !=="),
      ],
    );
  }
}
