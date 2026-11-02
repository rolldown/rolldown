use memchr::{memchr, memchr2};

#[inline]
/// ref https://github.com/rolldown/vite/blob/454c8fff9f7115ed29281c2d927366280508a0ab/packages/vite/src/shared/utils.ts#L31-L34
/// https://regex101.com/delete/E5Xk8cGCIde8tiY8I4TOe9eWqgTxyQj006TK
pub fn clean_url(v: &str) -> &str {
  if let Some(index) = memchr2(b'?', b'#', v.as_bytes()) { &v[0..index] } else { v }
}

pub fn get_query(v: &str) -> &str {
  if let Some(query_start) = memchr(b'?', v.as_bytes()) {
    let query_part = &v[query_start + 1..];
    if let Some(fragment_start) = memchr(b'#', query_part.as_bytes()) {
      &query_part[0..fragment_start]
    } else {
      query_part
    }
  } else {
    ""
  }
}

/// Decode the path of a URL-like specifier the way `decodeURI` does, so `my%20file.txt` refers
/// to `my file.txt` as it would at runtime. The query and fragment are left as written, and so
/// are escapes of reserved characters such as `%2F`, which would otherwise change the path.
/// Returns `None` when nothing changes or the decoded bytes are not valid UTF-8.
pub fn decode_uri_path(v: &str) -> Option<String> {
  let path = clean_url(v);
  memchr(b'%', path.as_bytes())?;

  let bytes = path.as_bytes();
  let mut decoded = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    let byte = bytes[i];
    if byte == b'%'
      && let Some(value) = bytes.get(i + 1..i + 3).and_then(|hex| {
        std::str::from_utf8(hex).ok().and_then(|hex| u8::from_str_radix(hex, 16).ok())
      })
      && !b";/?:@&=+$,#".contains(&value)
    {
      decoded.push(value);
      i += 3;
    } else {
      decoded.push(byte);
      i += 1;
    }
  }

  if decoded == bytes {
    return None;
  }
  let mut decoded = String::from_utf8(decoded).ok()?;
  decoded.push_str(&v[path.len()..]);
  Some(decoded)
}

#[cfg(test)]
mod tests {
  use super::decode_uri_path;

  #[test]
  fn decodes_like_decode_uri() {
    assert_eq!(decode_uri_path("./my%20file.txt").as_deref(), Some("./my file.txt"));
    assert_eq!(decode_uri_path("./%E2%9C%93.txt").as_deref(), Some("./\u{2713}.txt"));
    assert_eq!(decode_uri_path("./100%25.txt").as_deref(), Some("./100%.txt"));
    // Reserved characters keep their escapes, as with `decodeURI`.
    assert_eq!(decode_uri_path("./a%2Fb%20c.txt").as_deref(), Some("./a%2Fb c.txt"));
    assert_eq!(decode_uri_path("./a%3Fb.txt"), None);
    // The query and fragment are left as written.
    assert_eq!(decode_uri_path("./a%20b.txt?x=%20#y%20").as_deref(), Some("./a b.txt?x=%20#y%20"));
  }

  #[test]
  fn leaves_input_without_valid_escapes() {
    assert_eq!(decode_uri_path("./plain.txt"), None);
    assert_eq!(decode_uri_path("./a.txt?x=%20"), None);
    assert_eq!(decode_uri_path("./100%.txt"), None);
    assert_eq!(decode_uri_path("./%zz.txt"), None);
    // A lone UTF-8 continuation byte is not valid UTF-8.
    assert_eq!(decode_uri_path("./%80.txt"), None);
  }
}
