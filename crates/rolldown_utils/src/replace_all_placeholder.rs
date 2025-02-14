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
pub fn replace_all_placeholder(
  pattern: String,
  placeholder: &str,
  mut replacer: impl Replacer,
) -> String {
  let offset = placeholder.len() - 1;
  let mut iter = pattern.match_indices(&placeholder[..offset]).peekable();

  if iter.peek().is_none() {
    return pattern;
  }

  let mut ending = 0;
  let mut result = String::with_capacity(pattern.len());

  for (start, _) in iter {
    if start < ending {
      continue;
    }

    let start_offset = start + offset;
    let pat_temp = &pattern[start_offset..];

    let (end, len) = match pat_temp.as_bytes().first() {
      Some(b']') => (start_offset, None),
      Some(b':') => {
        if let Some(end) = pat_temp.find(']') {
          let end = start_offset + end;
          let len = pattern[start_offset + 1..end].parse::<usize>().ok();

          if len.is_none() {
            continue;
          }

          (end, len)
        } else {
          continue;
        }
      }
      _ => continue,
    };

    let replacer = replacer.get(len);

    result.push_str(&pattern[ending..start]);
    result.push_str(replacer.as_ref());

    ending = end + 1;
  }

  if ending < pattern.len() {
    result.push_str(&pattern[ending..]);
  }

  result
}

#[test]
fn test_replace_all_placeholder() {
  let result = replace_all_placeholder(
    "hello-[hash]-[hash:-]-[hash_name]-[hash:1]-[hash:].js".to_string(),
    "[hash]",
    "abc",
  );
  assert_eq!(result, "hello-abc-[hash:-]-[hash_name]-abc-[hash:].js");

  let result = replace_all_placeholder(
    "hello-[hash]-[hash:5]-[hash_name]-[hash:o].js".to_string(),
    "[hash]",
    |n: Option<usize>| &"abcdefgh"[..n.unwrap_or(8)],
  );
  assert_eq!(result, "hello-abcdefgh-abcde-[hash_name]-[hash:o].js");
}
