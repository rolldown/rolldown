use std::borrow::Cow;

pub trait Replacer {
  fn get(&mut self, _: Option<usize>) -> Cow<'_, str>;
}

impl Replacer for &str {
  #[inline]
  fn get(&mut self, _: Option<usize>) -> Cow<'_, str> {
    Cow::Borrowed(self)
  }
}

impl<F, S> Replacer for F
where
  F: FnMut(Option<usize>) -> S,
  S: AsRef<str>,
{
  #[inline]
  fn get(&mut self, hash_len: Option<usize>) -> Cow<'_, str> {
    Cow::Owned((*self)(hash_len).as_ref().to_string())
  }
}

/// Replace all `[placeholder]` or `[placeholder:8]` in the pattern
pub trait ReplaceAllPlaceholder {
  fn replace_all(self, placeholder: &str, replacer: impl Replacer) -> String;

  fn replace_all_with_len(self, placeholder: &str, replacer: impl Replacer) -> String;
}

impl ReplaceAllPlaceholder for String {
  #[inline]
  fn replace_all(self, placeholder: &str, replacer: impl Replacer) -> String {
    replace_all_placeholder_impl(self, false, placeholder, replacer)
  }

  #[inline]
  fn replace_all_with_len(self, placeholder: &str, replacer: impl Replacer) -> String {
    replace_all_placeholder_impl(self, true, placeholder, replacer)
  }
}

fn replace_all_placeholder_impl(
  pattern: String,
  is_len_enabled: bool,
  mut placeholder: &str,
  mut replacer: impl Replacer,
) -> String {
  let offset = placeholder.len() - 1;

  if is_len_enabled {
    placeholder = &placeholder[..offset];
  }

  let mut iter = pattern.match_indices(placeholder).peekable();

  if iter.peek().is_none() {
    return pattern;
  }

  let mut last_end = 0;
  let mut result = String::with_capacity(pattern.len());

  for (start, _) in iter {
    if start < last_end {
      continue;
    }

    let start_offset = start + offset;
    let (end, len) = if is_len_enabled {
      let rest = &pattern[start_offset..];
      match rest.as_bytes().first() {
        Some(&b':') => {
          if let Some(index) = rest.find(']') {
            match rest[1..index].parse::<usize>() {
              Ok(len) => (start_offset + index, Some(len)),
              Err(_) => continue,
            }
          } else {
            continue;
          }
        }
        Some(&b']') => (start_offset, None),
        _ => continue,
      }
    } else {
      (start_offset, None)
    };

    let replacer = replacer.get(len);

    result.push_str(&pattern[last_end..start]);
    result.push_str(replacer.as_ref());

    last_end = end + 1;
  }

  if last_end < pattern.len() {
    result.push_str(&pattern[last_end..]);
  }

  result
}

#[test]
fn test_replace_all_placeholder() {
  let result = "hello-[hash]-[hash_name]-[hash:1].js".to_string().replace_all("[hash]", "abc");
  assert_eq!(result, "hello-abc-[hash_name]-[hash:1].js");

  let result = "hello-[hash]-[hash:-]-[hash_name]-[hash:1]-[hash:].js"
    .to_string()
    .replace_all_with_len("[hash]", "abc");
  assert_eq!(result, "hello-abc-[hash:-]-[hash_name]-abc-[hash:].js");

  let result = "hello-[hash]-[hash:5]-[hash_name]-[hash:o].js"
    .to_string()
    .replace_all_with_len("[hash]", |n: Option<usize>| &"abcdefgh"[..n.unwrap_or(8)]);
  assert_eq!(result, "hello-abcdefgh-abcde-[hash_name]-[hash:o].js");
}
