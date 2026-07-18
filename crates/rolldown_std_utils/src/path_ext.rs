//! Path helpers for Rolldown's known-UTF-8 module / filesystem path domain.
//!
//! Prefer these over open-coding `sugar_path` compositions. sugar_path 3 returns
//! `Cow<Path>` from `relative` and makes slash conversion explicit; the patterns
//! below are the ones we want in call sites so later code does not reintroduce
//! lossy or double-allocating chains like
//! `relative(...).to_slash_lossy().into_owned()` or
//! `normalize().as_ref().expect_to_slash()`.

use std::{
  borrow::Cow,
  ffi::OsStr,
  path::{Path, PathBuf},
};

use sugar_path::{SugarPath as _, SugarPathBuf as _};

pub trait PathExt {
  fn expect_to_str(&self) -> &str;

  fn expect_to_slash(&self) -> String;

  fn is_in_node_modules(&self) -> bool;

  fn representative_file_name(&self) -> Cow<'_, str>;
}

impl PathExt for Path {
  fn expect_to_str(&self) -> &str {
    self.to_str().unwrap_or_else(|| {
      panic!("Failed to convert {:?} to valid utf8 str", self.display());
    })
  }

  fn expect_to_slash(&self) -> String {
    // Strict slash conversion: panics on invalid UTF-8, matching the prior contract.
    self.to_slash().into_owned()
  }

  fn is_in_node_modules(&self) -> bool {
    self.components().any(|comp| comp.as_os_str() == "node_modules")
  }

  /// It doesn't ensure the file name is a valid identifier in JS.
  ///
  /// Callers pass module ids / specifiers, which are always valid UTF-8, so names
  /// are extracted with `to_str()` and this never needs `to_string_lossy()`.
  fn representative_file_name(&self) -> Cow<'_, str> {
    let file_name =
      self.file_stem().and_then(OsStr::to_str).or_else(|| self.to_str()).unwrap_or_default();

    match file_name {
      // "index": Node.js use `index` as a special name for directory import.
      // "mod": https://docs.deno.com/runtime/manual/references/contributing/style_guide#do-not-use-the-filename-indextsindexjs.
      "index" | "mod" => Cow::Borrowed(
        self.parent().and_then(Path::file_name).and_then(OsStr::to_str).unwrap_or(file_name),
      ),
      _ => Cow::Borrowed(file_name),
    }
  }
}

/// The first one is for chunk name, the second element is used for generate absolute file name
pub fn representative_file_name_for_preserve_modules(
  path: &Path,
) -> (Cow<'_, str>, String, Option<Cow<'_, str>>) {
  // As above: `path` is a module id (valid UTF-8), so `to_str()` is infallible
  // and we avoid `to_string_lossy()`.
  let file_name = Cow::Borrowed(
    path.file_stem().and_then(OsStr::to_str).or_else(|| path.to_str()).unwrap_or_default(),
  );
  let ab_path = path.with_extension("").to_str().unwrap_or_default().to_owned();
  (file_name, ab_path, path.extension().and_then(OsStr::to_str).map(Cow::Borrowed))
}

pub fn strip_path_prefix_to_slash(path: &Path, prefix: &Path) -> Option<String> {
  path.strip_prefix(prefix).ok().map(PathExt::expect_to_slash)
}

/// Lexical path from `base` to `target` as a `/`-separated UTF-8 string.
///
/// Uses sugar_path 3's intended composition for known-UTF-8 Rolldown paths:
/// `relative` may borrow a clean descendant, then one owned buffer becomes the
/// final slash `String` via [`sugar_path::SugarPathBuf::into_slash`].
///
/// Prefer this over `relative(...).to_slash_lossy().into_owned()` or
/// `relative(...).as_path().expect_to_slash()`.
#[inline]
pub fn relative_path_to_slash(target: impl AsRef<Path>, base: impl AsRef<Path>) -> String {
  target.as_ref().relative(base).into_owned().into_slash()
}

/// Like [`relative_path_to_slash`], but formats a JS-style relative specifier:
/// - equal paths → `"."`
/// - paths that leave the base (`..`…) → the slash relative as-is
/// - otherwise → `"./…"`
///
/// sugar_path 3 returns an empty path for equal inputs; this helper keeps the
/// historical Rolldown/Rollup `./` spelling at call sites that emit import
/// specifiers or chunk-relative asset URLs.
#[inline]
pub fn relative_path_as_js_specifier(target: impl AsRef<Path>, base: impl AsRef<Path>) -> String {
  let relative = target.as_ref().relative(base);
  if relative.as_os_str().is_empty() {
    return ".".to_string();
  }
  let slash = relative.into_owned().into_slash();
  // Only true parent segments (`..` / `../…`), not filenames like `..foo`.
  if slash == ".." || slash.starts_with("../") { slash } else { format!("./{slash}") }
}

/// Absolute `path` → slash-separated path relative to `cwd` (stable ids / diagnostics).
///
/// Non-absolute inputs are returned as `path` with native separators converted via
/// strict slash conversion when they are valid UTF-8 paths; virtual ids should be
/// handled by the caller before calling this.
#[inline]
pub fn absolute_path_to_relative_slash(path: impl AsRef<Path>, cwd: impl AsRef<Path>) -> String {
  let path = path.as_ref();
  if path.is_absolute() { relative_path_to_slash(path, cwd) } else { path.expect_to_slash() }
}

/// Ensure an owned path is absolute before using it as sugar_path 3's explicit cwd.
#[inline]
pub fn absolutize_path_buf(path: PathBuf) -> PathBuf {
  if path.is_absolute() { path } else { path.absolutize().into_owned() }
}

/// Consume a `PathBuf` into a `/`-separated UTF-8 string (known-UTF-8 invariant).
#[inline]
pub fn path_buf_to_slash(path: PathBuf) -> String {
  path.into_slash()
}

/// Normalize an owned path, then consume it into a `/`-separated UTF-8 string.
///
/// Prefer this when `path` was just created by `join` or another owned operation.
/// The consuming chain lets the normalized `PathBuf` become the final `String`
/// instead of copying it through a non-consuming
/// [`sugar_path::SugarPath::normalize`] result.
///
/// # Panics
///
/// Panics if `path` is not valid UTF-8. Rolldown module and resolver paths are
/// required to satisfy that invariant.
#[inline]
pub fn normalize_path_buf_to_slash(path: PathBuf) -> String {
  path.into_normalized().into_slash()
}

#[test]
fn test_relative_path_helpers() {
  let workspace = std::env::current_dir().unwrap().join("path-helper-tests");
  let base = workspace.join("src");
  let nested = base.join("lib").join("mod.js");
  assert_eq!(relative_path_to_slash(&nested, &base), "lib/mod.js");
  assert_eq!(relative_path_as_js_specifier(&nested, &base), "./lib/mod.js");
  assert_eq!(relative_path_as_js_specifier(&base, &base), ".");
  assert_eq!(relative_path_as_js_specifier(workspace.join("other"), &base), "../other");
  // Filename `..foo` is not a parent segment — still needs the `./` prefix.
  assert_eq!(relative_path_as_js_specifier(base.join("..foo.js"), &base), "./..foo.js");
  assert_eq!(relative_path_as_js_specifier(base.join(".hidden.js"), &base), "./.hidden.js");
  assert_eq!(absolute_path_to_relative_slash(&nested, &workspace), "src/lib/mod.js");
  assert!(absolutize_path_buf(PathBuf::from("path-helper-tests")).is_absolute());
  assert_eq!(path_buf_to_slash(PathBuf::from("src").join("lib.js")), "src/lib.js");
  assert_eq!(
    normalize_path_buf_to_slash(
      PathBuf::from("src").join(".").join("nested").join("..").join("lib.js")
    ),
    "src/lib.js",
  );
}

#[test]
fn test_representative_file_name() {
  let cwd = Path::new(".").join("project");
  let path = cwd.join("src").join("vue.js");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("vue").join("index.js");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("vue").join("mod.ts");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("foo.bar").join("index.js");
  assert_eq!(path.representative_file_name(), "foo.bar");

  let path = cwd.join("x.jsx");
  let (_, ab_path, _) = representative_file_name_for_preserve_modules(&path);
  assert_eq!(Path::new(&ab_path).file_name().unwrap().to_string_lossy(), "x");

  #[cfg(not(target_os = "windows"))]
  {
    let path = cwd.join("src").join("vue.js");
    assert_eq!(representative_file_name_for_preserve_modules(&path).1, "./project/src/vue");
  }
}

#[test]
fn test_strip_path_prefix_to_slash() {
  let path = Path::new("/project/src/bin/index");
  let prefix = Path::new("/project/src");
  assert_eq!(strip_path_prefix_to_slash(path, prefix).as_deref(), Some("bin/index"));

  let path = Path::new("/project/src2/bin/index");
  let prefix = Path::new("/project/src");
  assert_eq!(strip_path_prefix_to_slash(path, prefix), None);
}

#[cfg(target_os = "windows")]
#[test]
fn test_strip_path_prefix_to_slash_with_mixed_windows_separators() {
  let path = Path::new(r"C:/project/src/bin/index");
  let prefix = Path::new(r"C:\project\src");
  assert_eq!(strip_path_prefix_to_slash(path, prefix).as_deref(), Some("bin/index"));
}
