use std::{
  borrow::{Borrow, Cow},
  cmp::Ordering,
  hash::{Hash, Hasher},
  path::Path,
};

use arcstr::ArcStr;
use rolldown_std_utils::PathExt as _;
use sugar_path::SugarPath as _;

use super::stable_module_id::StableModuleId;

const EMPTY_MODULE_PREFIX: &str = "\0rolldown/empty.js?";

/// Classification of a [`ModuleId`]'s string identity.
///
/// A module id coming out of resolution is *sometimes* a real filesystem path
/// and sometimes not (virtual modules, bare specifiers, URLs, …). The kind is
/// computed once at construction so path operations only run where they make
/// sense, instead of treating every id as a path and round-tripping the bytes
/// through `Path`/`to_string_lossy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleIdKind {
  /// An absolute filesystem path; path operations are meaningful.
  Path,
  /// A virtual module id, prefixed with `\0` (Rollup convention).
  Virtual,
  /// Anything else: bare specifier (`react`), URL (`https://…`), data URI,
  /// relative specifier, etc. Not a filesystem path.
  Bare,
}

#[derive(Clone)]
enum Repr {
  Path(ArcStr),
  Virtual(ArcStr),
  Bare(ArcStr),
}

/// `ModuleId` is the unique string identifier for each module.
/// - It will be used to identify the module in the whole bundle.
/// - Users could stored the `ModuleId` to track the module in different stages/hooks.
///
/// The backing string is an [`ArcStr`] — cheap to clone and the durable identity
/// that survives across (incremental) builds. The hot key for the module graph is
/// the ephemeral per-build integer handle [`ModuleIdx`](crate::ModuleIdx), not this
/// type, so `ModuleId` optimizes for being a faithful, lossless string/path identity.
#[derive(Clone)]
pub struct ModuleId {
  repr: Repr,
}

impl ModuleId {
  #[inline]
  pub fn new(value: impl Into<ArcStr>) -> Self {
    Self { repr: Self::classify(value.into()) }
  }

  /// Construct a `ModuleId` that is known at compile time to be a virtual id
  /// (e.g. the runtime module sentinel). The caller asserts the value is a
  /// virtual id; it is not re-classified, so this can be used in `const` context.
  #[inline]
  pub const fn new_virtual(inner: ArcStr) -> Self {
    Self { repr: Repr::Virtual(inner) }
  }

  /// Construct the sentinel id used for `browser: false` ignored modules,
  /// concatenated with the original resolved path so each ignored module
  /// stays distinguishable while sharing the empty-module load behavior.
  pub fn new_empty(original: &str) -> Self {
    Self::new(format!("{EMPTY_MODULE_PREFIX}{original}"))
  }

  fn classify(inner: ArcStr) -> Repr {
    if inner.starts_with('\0') {
      Repr::Virtual(inner)
    } else if Path::new(inner.as_str()).is_absolute() {
      Repr::Path(inner)
    } else {
      Repr::Bare(inner)
    }
  }

  pub fn kind(&self) -> ModuleIdKind {
    match self.repr {
      Repr::Path(_) => ModuleIdKind::Path,
      Repr::Virtual(_) => ModuleIdKind::Virtual,
      Repr::Bare(_) => ModuleIdKind::Bare,
    }
  }

  /// Whether the id is an absolute filesystem path.
  pub fn is_path(&self) -> bool {
    matches!(self.repr, Repr::Path(_))
  }

  pub fn is_empty_module(&self) -> bool {
    self.as_str().starts_with(EMPTY_MODULE_PREFIX)
  }

  /// For an id created via `new_empty`, returns the original id portion.
  pub fn strip_empty_prefix(&self) -> Option<&str> {
    self.as_str().strip_prefix(EMPTY_MODULE_PREFIX)
  }

  pub fn as_str(&self) -> &str {
    self.as_arc_str().as_str()
  }

  pub fn as_arc_str(&self) -> &ArcStr {
    match &self.repr {
      Repr::Path(inner) | Repr::Virtual(inner) | Repr::Bare(inner) => inner,
    }
  }

  /// Borrow the id as a filesystem [`Path`], but only when it actually is one
  /// (an absolute path). Returns `None` for virtual ids, bare specifiers, URLs,
  /// etc. This is a zero-cost view (`Path::new`); use it to *gate* path logic so
  /// non-path ids don't get silently path-parsed.
  pub fn as_path(&self) -> Option<&Path> {
    match &self.repr {
      Repr::Path(inner) => Some(Path::new(inner.as_str())),
      Repr::Virtual(_) | Repr::Bare(_) => None,
    }
  }

  /// Whether the id is a filesystem path inside a `node_modules` directory.
  /// Non-path ids (virtual, bare, URL) are never in `node_modules`.
  pub fn is_in_node_modules(&self) -> bool {
    self.as_path().is_some_and(rolldown_std_utils::PathExt::is_in_node_modules)
  }

  /// A short, human-meaningful name derived from the id, used for chunk and
  /// variable naming. This intentionally applies the file-name heuristic to the
  /// raw id regardless of kind (e.g. a virtual `\0…/empty.js?x` still yields
  /// `empty`), matching historical behavior — naming is a heuristic, not a path
  /// operation, so it is not gated on [`is_path`](Self::is_path).
  pub fn representative_name(&self) -> Cow<'_, str> {
    Path::new(self.as_str()).representative_file_name()
  }

  pub fn relative_path(&self, root: impl AsRef<Path>) -> Cow<'_, Path> {
    Path::new(self.as_str()).relative(root)
  }

  pub fn stabilize(&self, cwd: &Path) -> StableModuleId {
    StableModuleId::new(self, cwd)
  }

  pub fn into_inner(self) -> ArcStr {
    match self.repr {
      Repr::Path(inner) | Repr::Virtual(inner) | Repr::Bare(inner) => inner,
    }
  }
}

impl Default for ModuleId {
  fn default() -> Self {
    Self { repr: Repr::Bare(ArcStr::default()) }
  }
}

impl std::fmt::Debug for ModuleId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Debug::fmt(self.as_str(), f)
  }
}

impl std::fmt::Display for ModuleId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Display::fmt(self.as_str(), f)
  }
}

// `Eq`/`Ord`/`Hash`/`Borrow<str>` are implemented in terms of `as_str()` (byte-exact,
// ignoring the kind discriminant). This keeps the `&str`-lookup contract below: a
// `ModuleId` hashes and compares identically to its string, so the same string always
// classifies to one variant and the maps keyed by `ModuleId` stay consistent.
impl PartialEq for ModuleId {
  fn eq(&self, other: &Self) -> bool {
    self.as_str() == other.as_str()
  }
}

impl Eq for ModuleId {}

impl PartialOrd for ModuleId {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for ModuleId {
  fn cmp(&self, other: &Self) -> Ordering {
    self.as_str().cmp(other.as_str())
  }
}

impl Hash for ModuleId {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_str().hash(state);
  }
}

impl AsRef<str> for ModuleId {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl std::ops::Deref for ModuleId {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.as_str()
  }
}

impl From<&str> for ModuleId {
  fn from(value: &str) -> Self {
    Self::new(value)
  }
}

impl From<String> for ModuleId {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl From<ArcStr> for ModuleId {
  fn from(value: ArcStr) -> Self {
    Self::new(value)
  }
}

// This allows to use `&str` to lookup `HashMap<ModuleId, V>`. For `&String`, since it could coerce to `&str`, it also works.
impl Borrow<str> for ModuleId {
  fn borrow(&self) -> &str {
    self.as_str()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn relative_path_preserves_borrowed_and_owned_results() {
    #[cfg(not(windows))]
    let (target, descendant_base, sibling_base, descendant, sibling) = (
      "/workspace/package/src/index.js",
      "/workspace/package",
      "/workspace/package/dist",
      "src/index.js",
      "../src/index.js",
    );
    #[cfg(windows)]
    let (target, descendant_base, sibling_base, descendant, sibling) = (
      r"C:\workspace\package\src\index.js",
      r"C:\workspace\package",
      r"C:\workspace\package\dist",
      r"src\index.js",
      r"..\src\index.js",
    );

    let module_id = ModuleId::new(target);
    let relative = module_id.relative_path(descendant_base);
    assert_eq!(relative, Path::new(descendant));
    assert!(matches!(relative, Cow::Borrowed(_)));

    let relative = module_id.relative_path(sibling_base);
    assert_eq!(relative, Path::new(sibling));
    assert!(matches!(relative, Cow::Owned(_)));
  }
}
