use std::collections::HashMap;

pub(crate) fn expand_typeof_replacements(
  values: &HashMap<String, String>,
) -> HashMap<String, String> {
  let mut replacements: Vec<(String, String)> = Vec::new();
  for key in values.keys() {
    expand_key(key, &mut replacements);
  }
  HashMap::from_iter(replacements)
}

fn expand_key(key: &str, replacements: &mut Vec<(String, String)>) {
  let replacements_len = replacements.len();

  let parts = KeyPartsIterator::new(key);
  for part in parts {
    let KeyPart::Valid(part) = part else {
      // Invalid part found. Remove any replacements added for previous parts.
      replacements.truncate(replacements_len);
      return;
    };

    replacements.extend([
      (format!("typeof {part} ==="), "\"object\" ===".to_string()),
      (format!("typeof {part}==="), "\"object\"===".to_string()),
      (format!("typeof {part} !=="), "\"object\" !==".to_string()),
      (format!("typeof {part}!=="), "\"object\"!==".to_string()),
      (format!("typeof {part} =="), "\"object\" ===".to_string()),
      (format!("typeof {part}=="), "\"object\"===".to_string()),
      (format!("typeof {part}!="), "\"object\"!==".to_string()),
      (format!("typeof {part} !="), "\"object\" !==".to_string()),
    ]);
  }
}

/// Iterator over key parts.
///
/// Splits key into parts on `.` separators.
/// Each part must match regex `[_$a-zA-Z\xA0-\uFFFF][_$a-zA-Z0-9\xA0-\uFFFF]*`.
/// If any part does not match that pattern, yields `KeyPart::Invalid` and stops iteration.
/// Valid parts are yielded as `KeyPart::Valid(&str)`.
/// Last part is checked against pattern, but is NOT yielded.
///
/// `"abc.def.ghi"` -> yields `"abc"`, `"def"`.
///
/// Process string byte-by-byte for speed.
/// Fast paths for ASCII chars using lookup tables, slow path for unicode (very uncommon case).
struct KeyPartsIterator<'s> {
  // `Some` if iterating, `None` if iteration has ended
  key: Option<&'s str>,
}

impl<'s> KeyPartsIterator<'s> {
  fn new(key: &'s str) -> Self {
    Self { key: Some(key) }
  }
}

enum KeyPart<'s> {
  Valid(&'s str),
  Invalid,
}

impl<'s> Iterator for KeyPartsIterator<'s> {
  type Item = KeyPart<'s>;

  fn next(&mut self) -> Option<KeyPart<'s>> {
    let Some(key) = self.key else {
      // Iteration has ended
      return None;
    };

    // Match first char of part
    let mut bytes = key.as_bytes().iter();
    let is_valid = if let Some(&b) = bytes.next() {
      if b.is_ascii() {
        ASCII_START.matches(b)
      } else {
        self.handle_unicode(&mut bytes)
      }
    } else {
      // Empty string - invalid
      false
    };
    if !is_valid {
      return self.invalid();
    }

    // Match remaining chars of part
    let mut found_dot = false;
    while let Some(&b) = bytes.next() {
      let is_valid = if b.is_ascii() {
        if b == b'.' {
          found_dot = true;
          break;
        }
        ASCII_CONTINUE.matches(b)
      } else {
        self.handle_unicode(&mut bytes)
      };
      if !is_valid {
        return self.invalid();
      }
    }

    if !found_dot {
      // Reached the end of string.
      // Don't yield last part, and stop iterating.
      self.key = None;
      return None;
    }

    // Found a `.`.
    // Yield the part, and set `self.key` to remaining string after the `.`.
    let after_dot_index = key.len() - bytes.as_slice().len();
    let part = &key[..after_dot_index - 1];
    let remaining = &key[after_dot_index..];
    self.key = Some(remaining);
    Some(KeyPart::Valid(part))
  }
}

impl<'s> KeyPartsIterator<'s> {
  /// Call when invalid part found
  #[allow(clippy::unnecessary_wraps)]
  fn invalid(&mut self) -> Option<KeyPart<'s>> {
    self.key = None;
    Some(KeyPart::Invalid)
  }

  /// Call when Unicode byte found.
  /// Returns `true` if it's valid.
  ///
  /// `#[cold]` and `#[inline(never)]` to keep body of loop in `KeyPartsIterator::next`
  /// as small as possible, so compiler can unroll the loop.
  #[cold]
  #[inline(never)]
  fn handle_unicode(&mut self, bytes: &mut std::slice::Iter<'s, u8>) -> bool {
    let key = self.key.unwrap();
    // `- 1` because `iter` has advanced past 1st byte of this unicode char
    let index = key.len() - bytes.as_slice().len() - 1;
    let mut chars = key[index..].chars();
    let c = chars.next().unwrap();
    if Self::is_valid_unicode_char(c) {
      *bytes = chars.as_str().as_bytes().iter();
      true
    } else {
      false
    }
  }

  fn is_valid_unicode_char(c: char) -> bool {
    // Match regex `[\xA0-\uFFFF]`.
    // The regex matches all characters above 0xA0 because it's not unicode-aware and is matching
    // UTF-16 char codes, not unicode code points. Rust's `char` uses unicode code points.
    c as u32 >= 0xA0
  }
}

/// Lookup table for ASCII bytes.
/// Aligned on 128 to fit into a pair of L1 cache lines.
#[repr(C, align(128))]
struct ByteMatchTable([bool; 128]);

impl ByteMatchTable {
  #[inline]
  fn matches(&self, b: u8) -> bool {
    self.0[b as usize]
  }
}

macro_rules! byte_match_table {
  (|$b:ident| $test:expr) => {{
    let mut arr = ByteMatchTable([false; 128]);
    let mut i = 0u8;
    while i < 128 {
      arr.0[i as usize] = {
        let $b = i;
        $test
      };
      i += 1;
    }
    arr
  }};
}

// Lookup tables for ASCII bytes
static ASCII_START: ByteMatchTable =
  byte_match_table!(|b| b == b'_' || b == b'$' || b.is_ascii_alphabetic());
static ASCII_CONTINUE: ByteMatchTable =
  byte_match_table!(|b| b == b'_' || b == b'$' || b.is_ascii_alphanumeric());

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
