use std::{borrow::Cow, path::Path};

use crate::pattern_filter::normalize_path;

/// Joins a base path with a glob pattern, returning the joined pattern as a `Cow<'a, str>`.
///
/// For Windows, it normalizes the base path by replacing backslashes with forward slashes
/// before joining with the glob pattern. It also ensures a forward slash separator
/// between the base path and the glob if needed.
///
/// Note: This function only constructs the joined glob pattern string. It does not
/// perform the actual globbing (finding matching files).
///
/// # Arguments
///
/// * `path`: The base directory path.
/// * `glob`: The glob pattern to join with the base path.
///
/// # Returns
///
/// A `Cow<'a, str>` representing the joined glob pattern.
pub fn join_path_with_glob<'a>(path: &'a str, glob: &'a str) -> Cow<'a, str> {
  if glob.starts_with("**") || Path::new(glob).is_absolute() {
    return Cow::Borrowed(glob);
  }

  let base = normalize_path(path);

  #[cfg(windows)]
  let base = if base.as_bytes().last().is_some_and(|&c| c != b'/') {
    let mut p = String::with_capacity(base.len() + 1);
    p.push_str(&base);
    p.push('/');
    Cow::Owned(p)
  } else {
    base
  };

  Cow::Owned(Path::new(base.as_ref()).join(glob).to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_join_path_with_glob_basic() {
    assert_eq!(join_path_with_glob("path/to", "*.txt"), "path/to/*.txt");
    #[cfg(windows)]
    {
      assert_eq!(join_path_with_glob("path\\to", "*.txt"), "path/to/*.txt");
      assert_eq!(join_path_with_glob("path\\to\\", "*.txt"), "path/to/*.txt");
      assert_eq!(join_path_with_glob("C:\\path\\to", "*.txt"), "C:/path/to/*.txt");
    }
  }

  #[test]
  fn test_join_path_with_glob_glob_with_separator() {
    assert_eq!(join_path_with_glob("path/to", "/*.txt"), "/*.txt");
    #[cfg(windows)]
    assert_eq!(join_path_with_glob("C:\\path\\to", "/*.txt"), "C:/*.txt");
  }
}
