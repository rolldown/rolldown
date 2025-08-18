/// SPECIAL_QUERY_RE = /[?&](?:worker|sharedworker|raw|url)\b/
pub fn is_special_query(ext: &str) -> bool {
  for i in memchr::memrchr2_iter(b'?', b'&', ext.as_bytes()) {
    let Some(after) = ext.get(i + 1..) else {
      continue;
    };

    let boundary = if after.starts_with("raw") || after.starts_with("url") {
      3usize
    } else if after.starts_with("worker") {
      6usize
    } else if after.starts_with("sharedworker") {
      12usize
    } else {
      continue;
    };

    // Test if match `\b`
    match after.get(boundary..=boundary).and_then(|c| c.bytes().next()) {
      Some(ch) if !ch.is_ascii_alphanumeric() && ch != b'_' => {
        return true;
      }
      None => return true,
      _ => {}
    }
  }
  false
}

#[test]
fn special_query() {
  assert!(is_special_query("test?workers&worker"));
  assert!(is_special_query("test?url&sharedworker"));
  assert!(is_special_query("test?url&raw"));

  assert!(!is_special_query("test?&woer"));
  assert!(!is_special_query("test?&sharedworker1"));
}
