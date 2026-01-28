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
/// # Errors
/// Returns an `io::Error` if path canonicalization fails (e.g., path doesn't exist).
///
/// # Examples
/// ```no_run
/// # use std::path::PathBuf;
/// # use rolldown_binding::utils::normalize_windows_path;
/// // On Windows:
/// // normalize_windows_path("\\\\?\\Volume{b91e17d5-0f25-4590-af8c-3b0508620c31}\\foo\\bar")
/// // -> "D:\\foo\\bar" (if the volume is mounted as D:)
/// ```
#[expect(clippy::unnecessary_wraps, reason = "Consistent API across platforms")]
#[inline]
pub fn normalize_windows_path(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
  #[cfg(windows)]
  {
    normalize_windows_path_impl(path.as_ref())
  }
  #[cfg(not(windows))]
  {
    Ok(path.as_ref().to_path_buf())
  }
}

#[cfg(windows)]
fn normalize_windows_path_impl(path: &Path) -> std::io::Result<PathBuf> {
  use std::os::windows::ffi::OsStrExt;

  let path_bytes = path.as_os_str().as_encoded_bytes();

  // Check if this is a DOS device path that needs special handling
  let needs_canonicalize = if let Some(p) = path_bytes.strip_prefix(br"\\?\") {
    // Check if it's NOT a simple verbatim disk path like "\\?\C:\"
    // Verbatim disk paths have the format "\\?\X:\" or "\\?\X:\..." where X is a drive letter
    let is_verbatim_disk = p.len() >= 2
      && p[1] == b':'
      && (p[0] as char).is_ascii_alphabetic()
      && (p.len() == 2 || p[2] == b'\\');
    !is_verbatim_disk
  } else if path_bytes.starts_with(br"\\.\") {
    // Also handle "\\.\Volume{...}" format
    true
  } else {
    // Regular paths don't need canonicalization
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
  fn test_normalize_windows_path_detects_volume_guid_pattern() {
    // Test that the function recognizes volume GUID patterns correctly
    // This path won't exist, so we expect an error, but we can verify
    // that it's identified as needing canonicalization by checking
    // that it attempts to canonicalize (which will fail with NotFound)
    let volume_guid_path = r"\\?\Volume{b91e17d5-0f25-4590-af8c-3b0508620c31}\test";
    let result = normalize_windows_path(volume_guid_path);
    
    // We expect this to fail since the path doesn't exist,
    // but it should fail with NotFound, not with a panic
    assert!(result.is_err());
    if let Err(e) = result {
      // The error should be NotFound or similar I/O error
      assert!(matches!(e.kind(), std::io::ErrorKind::NotFound));
    }
  }
}
