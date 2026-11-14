use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use rolldown_std_utils::PathExt;

use crate::concat_string;

/// Resolve `path` to an absolute path the way Node's `path` module sees it.
///
/// Node (and therefore Rollup) treat a rooted-but-drive-less path such as
/// `/favicon` or `\favicon` as absolute — implicitly anchored to the current
/// volume — while Rust's `Path::is_absolute()` reports it as non-absolute on
/// Windows because it lacks a volume prefix. This anchors such a path to `cwd`'s
/// volume — its drive `C:`, or its `\\server\share` UNC root — so downstream
/// path math (`preserveModulesRoot` stripping, relativizing against the input
/// base) treats it like any other absolute path.
/// See https://github.com/rolldown/rolldown/issues/10186.
///
/// Returns:
/// - `Some(Cow::Borrowed(path))` when `path` is already absolute,
/// - `Some(Cow::Owned(..))` with `cwd`'s volume glued on when `path` is rooted
///   but drive-less (only reachable on Windows),
/// - `None` when `path` is truly relative, or no volume can be taken from `cwd`
///   (a non-disk/non-UNC prefix, or a relative `cwd`).
pub fn node_style_absolute<'a>(path: &'a Path, cwd: &Path) -> Option<Cow<'a, Path>> {
  if path.is_absolute() {
    return Some(Cow::Borrowed(path));
  }
  if !path.has_root() {
    return None;
  }
  let std::path::Component::Prefix(prefix) = cwd.components().next()? else {
    return None;
  };
  let path = path.expect_to_str();
  // Reduce the prefix to a plain (non-verbatim) volume root — a drive `C:` or a
  // `\\server\share` UNC root — then glue the rooted path onto it. Verbatim
  // prefixes are normalized away because a verbatim path (`\\?\`) neither allows
  // forward slashes nor normalizes `.`/`..`, which the rooted id may carry.
  let glued = match prefix.kind() {
    std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) => {
      concat_string!(char::from(drive).to_string(), ":", path)
    }
    std::path::Prefix::UNC(server, share) | std::path::Prefix::VerbatimUNC(server, share) => {
      concat_string!(r"\\", server.to_str()?, r"\", share.to_str()?, path)
    }
    _ => return None,
  };
  Some(Cow::Owned(PathBuf::from(glued)))
}

#[test]
fn test_node_style_absolute() {
  // Truly relative paths and bare specifiers are not touched.
  assert_eq!(node_style_absolute(Path::new("src/a.js"), Path::new("/cwd")), None);
  assert_eq!(node_style_absolute(Path::new("react"), Path::new("/cwd")), None);

  // Already-absolute paths pass through by reference.
  #[cfg(not(target_os = "windows"))]
  {
    // On posix a rooted path IS absolute, so it never needs gluing.
    let p = Path::new("/favicon");
    assert!(matches!(node_style_absolute(p, Path::new("/cwd")), Some(Cow::Borrowed(_))));
  }

  #[cfg(target_os = "windows")]
  {
    let p = Path::new(r"E:\proj\a.js");
    assert!(matches!(node_style_absolute(p, Path::new(r"C:\cwd")), Some(Cow::Borrowed(_))));

    // Rooted-but-drive-less paths get the cwd drive glued on.
    assert_eq!(
      node_style_absolute(Path::new("/favicon"), Path::new(r"E:\proj")).as_deref(),
      Some(Path::new("E:/favicon"))
    );
    assert_eq!(
      node_style_absolute(Path::new(r"\a\b"), Path::new(r"C:\x")).as_deref(),
      Some(Path::new(r"C:\a\b"))
    );

    // A UNC cwd anchors either separator form to its `\\server\share` root,
    // discarding the cwd path below the share just like Node's `path.resolve`.
    assert_eq!(
      node_style_absolute(Path::new("/favicon"), Path::new(r"\\server\share\project")).as_deref(),
      Some(Path::new(r"\\server\share/favicon"))
    );
    assert_eq!(
      node_style_absolute(Path::new(r"\favicon"), Path::new(r"\\server\share\project")).as_deref(),
      Some(Path::new(r"\\server\share\favicon"))
    );

    // No volume to take from cwd -> None.
    assert_eq!(node_style_absolute(Path::new("/favicon"), Path::new("relative")), None);
  }
}
