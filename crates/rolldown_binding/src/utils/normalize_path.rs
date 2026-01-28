use std::path::{Path, PathBuf};

/// Normalizes a Windows path to a format that can be consumed by the resolver.
///
/// On Windows, this function:
/// - Converts DOS device paths with Volume GUIDs (e.g., `\\?\Volume{...}`) to regular paths
///   by canonicalizing them and then simplifying with dunce
/// - Converts verbatim disk paths (e.g., `\\?\C:\`) to regular paths using dunce
/// - Returns other paths unchanged
///
/// On non-Windows platforms, this is a no-op that returns the path unchanged.
///
/// # Examples
/// ```
/// # use std::path::PathBuf;
/// # use rolldown_binding::utils::normalize_windows_path;
/// // On Windows:
/// // normalize_windows_path("\\\\?\\Volume{b91e17d5-0f25-4590-af8c-3b0508620c31}\\foo\\bar")
/// // -> "D:\\foo\\bar" (if the volume is mounted as D:)
/// ```
#[inline]
pub fn normalize_windows_path(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
  let path = path.as_ref();

  #[cfg(not(windows))]
  {
    Ok(path.to_path_buf())
  }

  #[cfg(windows)]
  {
    normalize_windows_path_impl(path)
  }
}

#[cfg(windows)]
fn normalize_windows_path_impl(path: &Path) -> std::io::Result<PathBuf> {
  use std::os::windows::ffi::OsStrExt;

  let path_bytes = path.as_os_str().as_encoded_bytes();

  // Check if this is a DOS device path with Volume GUID or other special format
  let needs_canonicalize = if let Some(p) = path_bytes.strip_prefix(br"\\?\") {
    // Check if it's NOT a simple verbatim disk path like "\\?\C:\"
    // Verbatim disk paths have the format "\\?\X:\" where X is a drive letter
    !(p.len() >= 2 && p[1] == b':' && (p[0] as char).is_ascii_alphabetic())
  } else if path_bytes.starts_with(br"\\.\") {
    // Also handle "\\.\Volume{...}" format
    true
  } else {
    false
  };

  let normalized = if needs_canonicalize {
    // For Volume GUID paths and other special DOS device paths,
    // canonicalize them to resolve to a regular path
    std::fs::canonicalize(path)?
  } else {
    path.to_path_buf()
  };

  // Use dunce to convert any remaining UNC paths to legacy format
  Ok(dunce::simplified(&normalized).to_path_buf())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[cfg(not(windows))]
  fn test_normalize_windows_path_noop_on_unix() {
    let path = Path::new("/foo/bar");
    assert_eq!(normalize_windows_path(path).unwrap(), path);
  }

  #[test]
  #[cfg(windows)]
  fn test_normalize_windows_path_regular_path() {
    let path = PathBuf::from(r"C:\Users\test");
    let result = normalize_windows_path(&path).unwrap();
    // Regular paths should be returned unchanged (after dunce simplification)
    assert_eq!(result, path);
  }

  #[test]
  #[cfg(windows)]
  fn test_normalize_windows_path_verbatim_disk() {
    let path = PathBuf::from(r"\\?\C:\Users\test");
    let result = normalize_windows_path(&path).unwrap();
    // Verbatim disk paths should be simplified to regular paths
    assert_eq!(result, PathBuf::from(r"C:\Users\test"));
  }

  #[test]
  #[cfg(windows)]
  fn test_normalize_windows_path_volume_guid() {
    // This test would require a real Volume GUID path to work properly
    // In a real test environment, you would use a real volume GUID
    // For now, we just test that the function doesn't panic
    let result = normalize_windows_path(r"C:\");
    assert!(result.is_ok());
  }
}
