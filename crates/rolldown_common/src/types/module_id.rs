use std::{
  borrow::{Borrow, Cow},
  cmp::Ordering,
  hash::{Hash, Hasher},
  path::{Path, PathBuf},
};

use arcstr::ArcStr;
use rolldown_std_utils::PathExt as _;
use sugar_path::SugarPath as _;

use super::{import_attributes::ImportAttributes, stable_module_id::StableModuleId};

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
  inner: ArcStr,
  kind: ModuleIdKind,
  /// Length of the raw id when `inner` is `rawId?attributes=...`, [`NO_ATTRIBUTES`] otherwise.
  raw_len: u32,
}

const NO_ATTRIBUTES: u32 = u32::MAX;

impl ModuleId {
  #[inline]
  pub fn new(value: impl Into<ArcStr>) -> Self {
    let inner = value.into();
    Self { kind: Self::classify(&inner), inner, raw_len: NO_ATTRIBUTES }
  }

  /// The id of the module `raw_id` resolves to when imported with `attributes`. The attributes
  /// become part of the id, so the same file imported with different attributes is a different
  /// module, while path operations keep working on `raw_id`.
  pub fn with_attributes(raw_id: &str, attributes: &ImportAttributes) -> Self {
    if attributes.is_empty() {
      return Self::new(raw_id);
    }
    let mut id = Self::new(attributes.compose_module_id(raw_id));
    id.raw_len = u32::try_from(raw_id.len()).expect("module id should be shorter than 4 GiB");
    id
  }

  /// Construct a `ModuleId` that is known at compile time to be a virtual id
  /// (e.g. the runtime module sentinel). The caller asserts the value is a
  /// virtual id; it is not re-classified, so this can be used in `const` context.
  #[inline]
  pub const fn new_virtual(inner: ArcStr) -> Self {
    Self { inner, kind: ModuleIdKind::Virtual, raw_len: NO_ATTRIBUTES }
  }

  /// Construct the sentinel id used for `browser: false` ignored modules,
  /// concatenated with the original resolved path so each ignored module
  /// stays distinguishable while sharing the empty-module load behavior.
  pub fn new_empty(original: &str) -> Self {
    Self::new(format!("{EMPTY_MODULE_PREFIX}{original}"))
  }

  fn classify(inner: &str) -> ModuleIdKind {
    if inner.starts_with('\0') {
      ModuleIdKind::Virtual
    } else if Path::new(inner).is_absolute() {
      ModuleIdKind::Path
    } else {
      ModuleIdKind::Bare
    }
  }

  pub fn kind(&self) -> ModuleIdKind {
    self.kind
  }

  /// Whether the id is an absolute filesystem path.
  pub fn is_path(&self) -> bool {
    matches!(self.kind, ModuleIdKind::Path)
  }

  /// The id without its import attributes: what the module is read from and watched by.
  pub fn raw_id(&self) -> &str {
    if self.raw_len == NO_ATTRIBUTES { &self.inner } else { &self.inner[..self.raw_len as usize] }
  }

  /// The `?attributes=...` / `&attributes=...` part after [`raw_id`](Self::raw_id), or `""`.
  pub fn attributes_suffix(&self) -> &str {
    if self.raw_len == NO_ATTRIBUTES { "" } else { &self.inner[self.raw_len as usize..] }
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
    &self.inner
  }

  /// Borrow the id as a filesystem [`Path`], but only when it actually is one
  /// (an absolute path). Returns `None` for virtual ids, bare specifiers, URLs,
  /// etc. This is a zero-cost view (`Path::new`); use it to *gate* path logic so
  /// non-path ids don't get silently path-parsed. Import attributes are not part of the path.
  pub fn as_path(&self) -> Option<&Path> {
    self.is_path().then(|| Path::new(self.raw_id()))
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
    Path::new(self.raw_id()).representative_file_name()
  }

  /// The raw id relative to `root`, without import attributes.
  pub fn relative_path(&self, root: impl AsRef<Path>) -> PathBuf {
    Path::new(self.raw_id()).relative(root).into_owned()
  }

  pub fn stabilize(&self, cwd: &Path) -> StableModuleId {
    StableModuleId::new(self, cwd)
  }

  pub fn into_inner(self) -> ArcStr {
    self.inner
  }
}

impl Default for ModuleId {
  fn default() -> Self {
    Self { inner: ArcStr::default(), kind: ModuleIdKind::Bare, raw_len: NO_ATTRIBUTES }
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
