use memchr::{memchr, memrchr};

/// Compute relative path from `base` to `target`, returning a slash-separated string.
///
/// Both paths must be absolute. Handles `.` and `..` components via lightweight
/// normalization (no filesystem access). For already-normalized paths (the common case),
/// uses a zero-allocation fast path that operates directly on `&str` slices.
///
/// This is a fast alternative to `sugar_path::SugarPath::relative()` that avoids
/// `absolutize()`/`normalize()` overhead and intermediate `PathBuf` allocations.
pub fn relative_to_slash(target: &str, base: &str) -> String {
  // On Windows, normalize `\` to `/` before processing.
  // Uses memchr SIMD to find backslashes; skips allocation when none exist.
  #[cfg(windows)]
  {
    let target_norm = normalize_backslash(target);
    let base_norm = normalize_backslash(base);
    return relative_to_slash_inner(&target_norm, &base_norm);
  }
  #[cfg(not(windows))]
  relative_to_slash_inner(target, base)
}

/// Replace `\` with `/` using memchr SIMD search. Returns the input unchanged
/// (zero allocation) when no backslashes are present.
#[cfg(windows)]
fn normalize_backslash(s: &str) -> std::borrow::Cow<'_, str> {
  let bytes = s.as_bytes();
  let Some(first) = memchr(b'\\', bytes) else {
    return std::borrow::Cow::Borrowed(s);
  };
  // Copy prefix as-is, then replace backslashes via memchr jumps
  let mut out = Vec::with_capacity(bytes.len());
  out.extend_from_slice(&bytes[..first]);
  out.push(b'/');
  let mut offset = first + 1;
  while let Some(pos) = memchr(b'\\', &bytes[offset..]) {
    out.extend_from_slice(&bytes[offset..offset + pos]);
    out.push(b'/');
    offset += pos + 1;
  }
  out.extend_from_slice(&bytes[offset..]);
  // SAFETY: input is valid UTF-8, and we only replaced `\` (single ASCII byte) with `/`
  std::borrow::Cow::Owned(unsafe { String::from_utf8_unchecked(out) })
}

fn relative_to_slash_inner(target: &str, base: &str) -> String {
  // Fast path: check if either path needs normalization (contains /. or /..)
  if needs_normalization(target) || needs_normalization(base) {
    return relative_to_slash_slow(target, base);
  }
  relative_to_slash_fast(target, base)
}

/// Check if a path contains `.` or `..` components that need normalization.
/// Uses `memchr` to jump between `/` positions — most bytes in a path aren't `/`,
/// so this skips the vast majority of the input.
#[inline]
fn needs_normalization(path: &str) -> bool {
  let bytes = path.as_bytes();
  let mut offset = 0;
  while let Some(pos) = memchr(b'/', &bytes[offset..]) {
    let slash = offset + pos;
    // Check for /. at slash+1
    if slash + 1 < bytes.len() && bytes[slash + 1] == b'.' {
      let after_dot = slash + 2;
      // "/." at end or "/./"
      if after_dot >= bytes.len() || bytes[after_dot] == b'/' {
        return true;
      }
      // "/.." at end or "/../"
      if bytes[after_dot] == b'.' && (after_dot + 1 >= bytes.len() || bytes[after_dot + 1] == b'/')
      {
        return true;
      }
    }
    offset = slash + 1;
  }
  false
}

/// Fast path: no normalization needed. Operates directly on `&str` slices with zero Vec allocation.
fn relative_to_slash_fast(target: &str, base: &str) -> String {
  // Find common prefix byte-by-byte, then adjust to component boundary
  let common_byte_len = target.bytes().zip(base.bytes()).take_while(|(a, b)| a == b).count();

  // Adjust to last '/' boundary to ensure we match full components
  let common_prefix = if common_byte_len == target.len() && common_byte_len == base.len() {
    // Exact match
    common_byte_len
  } else if common_byte_len == target.len() && base.as_bytes().get(common_byte_len) == Some(&b'/') {
    // target is prefix of base, next char in base is /
    common_byte_len
  } else if common_byte_len == base.len() && target.as_bytes().get(common_byte_len) == Some(&b'/') {
    // base is prefix of target, next char in target is /
    common_byte_len
  } else {
    // Find last '/' within the common prefix using memrchr
    memrchr(b'/', &target.as_bytes()[..common_byte_len]).unwrap_or(0)
  };

  // Count remaining base components via memchr slash counting
  let base_remaining = &base.as_bytes()[common_prefix..];
  let mut ups = 0u32;
  {
    let mut offset = 0;
    // Count non-empty segments: each '/' starts a new segment (skip leading slash)
    while offset < base_remaining.len() {
      if base_remaining[offset] == b'/' {
        offset += 1;
        continue;
      }
      ups += 1;
      // Jump to next '/' or end
      offset = match memchr(b'/', &base_remaining[offset..]) {
        Some(pos) => offset + pos + 1,
        None => base_remaining.len(),
      };
    }
  }

  // Get remaining target path
  let target_remaining = &target[common_prefix..];
  let target_suffix = target_remaining.trim_start_matches('/');

  let ups = ups as usize;
  let suffix_iter = if target_suffix.is_empty() { None } else { Some(target_suffix) };
  let capacity = ups * 3 + target_suffix.len();
  let mut result = String::with_capacity(capacity);
  std::iter::repeat_n("..", ups).chain(suffix_iter).for_each(|s| {
    if !result.is_empty() {
      result.push('/');
    }
    result.push_str(s);
  });
  result
}

/// Slow path: normalize `.` and `..` components, then compute relative path.
/// Only called when paths contain `.` or `..` segments.
fn relative_to_slash_slow(target: &str, base: &str) -> String {
  let target_components = normalize_components(target);
  let base_components = normalize_components(base);

  // Find common prefix length
  let common_len =
    target_components.iter().zip(base_components.iter()).take_while(|(a, b)| a == b).count();

  let ups = base_components.len() - common_len;
  let remaining = &target_components[common_len..];

  let remaining_len: usize =
    remaining.iter().map(|s| s.len()).sum::<usize>() + remaining.len().saturating_sub(1);
  let capacity = ups * 3 + remaining_len;
  let mut result = String::with_capacity(capacity);
  std::iter::repeat_n("..", ups).chain(remaining.iter().copied()).for_each(|s| {
    if !result.is_empty() {
      result.push('/');
    }
    result.push_str(s);
  });
  result
}

/// Split an absolute path into normalized components, resolving `.` and `..`.
/// No filesystem access — purely lexical normalization.
fn normalize_components(path: &str) -> Vec<&str> {
  let mut components = Vec::new();
  for part in path.split('/') {
    match part {
      "" | "." => {}
      ".." => {
        components.pop();
      }
      _ => components.push(part),
    }
  }
  components
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_relative_same_dir() {
    assert_eq!(relative_to_slash("/a/b/c", "/a/b"), "c");
  }

  #[test]
  fn test_relative_parent_dir() {
    assert_eq!(relative_to_slash("/a/b", "/a/b/c"), "..");
  }

  #[test]
  fn test_relative_sibling() {
    assert_eq!(relative_to_slash("/a/b/c", "/a/b/d"), "../c");
  }

  #[test]
  fn test_relative_deep() {
    assert_eq!(relative_to_slash("/a/b/c/d/e", "/a/b"), "c/d/e");
  }

  #[test]
  fn test_relative_completely_different() {
    assert_eq!(relative_to_slash("/x/y/z", "/a/b/c"), "../../../x/y/z");
  }

  #[test]
  fn test_relative_same_path() {
    assert_eq!(relative_to_slash("/a/b/c", "/a/b/c"), "");
  }

  #[test]
  fn test_relative_root_to_deep() {
    assert_eq!(relative_to_slash("/a/b/c", "/"), "a/b/c");
  }

  #[test]
  fn test_relative_deep_to_root() {
    assert_eq!(relative_to_slash("/", "/a/b/c"), "../../..");
  }

  #[test]
  fn test_relative_with_dot_components() {
    assert_eq!(relative_to_slash("/a/b/c", "/a/./b"), "c");
    assert_eq!(relative_to_slash("/a/./b/c", "/a/b"), "c");
  }

  #[test]
  fn test_relative_with_dotdot_components() {
    assert_eq!(relative_to_slash("/a/b/../c", "/a"), "c");
    assert_eq!(relative_to_slash("/a/b/c", "/a/b/../b"), "c");
    assert_eq!(relative_to_slash("/a/b/../../x/y", "/x"), "y");
  }

  #[test]
  fn test_needs_normalization() {
    assert!(!needs_normalization("/a/b/c"));
    assert!(!needs_normalization("/a/b/c.js"));
    assert!(!needs_normalization("/a/b/.hidden"));
    assert!(!needs_normalization("/a/b/..hidden"));
    assert!(needs_normalization("/a/./b"));
    assert!(needs_normalization("/a/../b"));
    assert!(needs_normalization("/a/b/."));
    assert!(needs_normalization("/a/b/.."));
  }

  #[test]
  fn test_backslash_normalization() {
    // Simulate Windows paths by testing the inner function with pre-normalized input
    assert_eq!(relative_to_slash_inner("C:/Users/dev/src/main.rs", "C:/Users/dev"), "src/main.rs");
    assert_eq!(relative_to_slash_inner("C:/a/b/c", "C:/a/b/d"), "../c");
    assert_eq!(relative_to_slash_inner("D:/x/y", "D:/a/b"), "../../x/y");
  }
}
