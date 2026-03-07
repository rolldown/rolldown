use std::{
  fmt,
  path::{Path, PathBuf},
  sync::Mutex,
};

use lightningcss::{
  bundler::{Bundler, SourceProvider},
  stylesheet::ParserOptions,
};
use rustc_hash::FxHashSet;

// ---------------------------------------------------------------------------
// Error type for the SourceProvider
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum CssImportError {
  Io(std::io::Error),
  NotFound(PathBuf),
}

impl fmt::Display for CssImportError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      CssImportError::Io(e) => write!(f, "I/O error reading CSS: {e}"),
      CssImportError::NotFound(path) => write!(f, "CSS file not found: {}", path.display()),
    }
  }
}

impl std::error::Error for CssImportError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      CssImportError::Io(e) => Some(e),
      CssImportError::NotFound(_) => None,
    }
  }
}

// ---------------------------------------------------------------------------
// SourceProvider implementation for @import inlining
// ---------------------------------------------------------------------------

/// A filesystem-backed source provider that resolves @import specifiers
/// relative to the originating file's directory.
pub struct CssFileProvider {
  /// Stores owned CSS strings so we can return `&str` references from `read`.
  /// Each pointer was created via `Box::into_raw` and freed in `Drop`.
  sources: Mutex<Vec<*mut String>>,
  /// Tracks all resolved dependency paths for watch mode.
  resolved_deps: Mutex<FxHashSet<PathBuf>>,
}

// SAFETY: The raw pointers in `sources` are only used for memory management
// (allocate in `read`, deallocate in `Drop`). Access is always behind a Mutex.
unsafe impl Send for CssFileProvider {}
unsafe impl Sync for CssFileProvider {}

impl CssFileProvider {
  pub fn new() -> Self {
    Self { sources: Mutex::new(Vec::new()), resolved_deps: Mutex::new(FxHashSet::default()) }
  }

  /// Returns the set of resolved dependency paths discovered during bundling.
  pub fn resolved_deps(&self) -> FxHashSet<PathBuf> {
    self.resolved_deps.lock().unwrap().clone()
  }
}

impl Drop for CssFileProvider {
  fn drop(&mut self) {
    let sources = self.sources.lock().unwrap();
    for ptr in sources.iter() {
      // SAFETY: Each pointer was created via `Box::into_raw` in `read`.
      unsafe {
        drop(Box::from_raw(*ptr));
      }
    }
  }
}

impl SourceProvider for CssFileProvider {
  type Error = CssImportError;

  fn read<'a>(&'a self, file: &Path) -> Result<&'a str, Self::Error> {
    let canonical =
      std::fs::canonicalize(file).map_err(|_| CssImportError::NotFound(file.to_path_buf()))?;
    let content = std::fs::read_to_string(&canonical).map_err(CssImportError::Io)?;

    // Track this dependency
    self.resolved_deps.lock().unwrap().insert(canonical);

    // Store the string behind a raw pointer so the &str lives as long as self.
    let boxed = Box::new(content);
    let ptr = Box::into_raw(boxed);
    self.sources.lock().unwrap().push(ptr);
    // SAFETY: ptr is valid and won't be freed until `Drop`.
    Ok(unsafe { &*ptr })
  }

  fn resolve(&self, specifier: &str, originating_file: &Path) -> Result<PathBuf, Self::Error> {
    // Resolve relative to the originating file's directory
    let base_dir = originating_file.parent().unwrap_or(Path::new("."));
    let resolved = base_dir.join(specifier);

    if resolved.exists() {
      Ok(resolved)
    } else {
      // Try adding .css extension
      let with_ext = resolved.with_extension("css");
      if with_ext.exists() { Ok(with_ext) } else { Err(CssImportError::NotFound(resolved)) }
    }
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Result of inlining @import statements in a CSS file.
pub struct InlineResult {
  /// The CSS with all @imports inlined.
  pub code: String,
  /// Paths of all files that were imported (for watch mode).
  pub dependencies: FxHashSet<PathBuf>,
}

/// Inline all `@import` statements in the CSS file at `entry_path`.
///
/// Uses lightningcss's bundler to recursively resolve and inline imports,
/// correctly handling `@layer`, `@media`, and `@supports` wrapping.
pub fn inline_imports(entry_path: &Path) -> anyhow::Result<InlineResult> {
  let provider = CssFileProvider::new();
  let mut bundler = Bundler::new(&provider, None, ParserOptions::default());

  let stylesheet =
    bundler.bundle(entry_path).map_err(|e| anyhow::anyhow!("CSS bundler error: {e}"))?;

  let result = stylesheet
    .to_css(lightningcss::printer::PrinterOptions::default())
    .map_err(|e| anyhow::anyhow!("CSS printer error: {e}"))?;

  let dependencies = provider.resolved_deps();

  Ok(InlineResult { code: result.code, dependencies })
}

/// Inline `@import` statements from an already-loaded CSS string.
///
/// The lightningcss bundler reads files from disk via SourceProvider, so
/// the entry file must exist on disk. For virtual modules or files without
/// `@import`, returns the CSS as-is.
pub fn inline_imports_from_code(file_id: &str, css_code: &str) -> anyhow::Result<InlineResult> {
  let file_path = Path::new(file_id);

  // Fast path: skip the bundler entirely when there are no @import statements
  if !css_code.contains("@import") {
    return Ok(InlineResult { code: css_code.to_owned(), dependencies: FxHashSet::default() });
  }

  // The entry file should exist at `file_id` path since it was loaded by rolldown
  if file_path.exists() {
    inline_imports(file_path)
  } else {
    // File doesn't exist on disk (e.g. virtual module) — return as-is
    Ok(InlineResult { code: css_code.to_owned(), dependencies: FxHashSet::default() })
  }
}
