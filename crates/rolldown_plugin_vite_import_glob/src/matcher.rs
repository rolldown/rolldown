use arcstr::ArcStr;
use globstar::{CompileOptions, Glob, GlobError};

use crate::ViteImportGlobPlugin;

impl ViteImportGlobPlugin {
  /// `id` is a slash-normalized module id.
  pub(crate) fn set_globs(&self, id: &str, matchers: Vec<GlobMatcher>) {
    if matchers.is_empty() {
      self.glob_matchers.remove(id);
    } else {
      self.glob_matchers.insert(ArcStr::from(id), matchers);
    }
  }
}

/// Tells whether a file is matched by one `import.meta.glob` call, like the matcher vite builds
/// with picomatch in its `vite:import-glob` plugin.
#[derive(Debug)]
pub struct GlobMatcher {
  affirmed: Glob,
  negated: Option<Glob>,
}

impl GlobMatcher {
  /// The globs are absolute.
  pub fn new(
    affirmed: Vec<String>,
    mut negated: Vec<String>,
    exhaustive: bool,
    case_sensitive: bool,
  ) -> Result<Self, GlobError> {
    let options = CompileOptions::default().dot(exhaustive).case_insensitive(!case_sensitive);
    if !exhaustive {
      negated.push("**/node_modules/**".to_string());
    }
    Ok(Self {
      affirmed: Glob::union_with(affirmed, options)?,
      negated: if negated.is_empty() { None } else { Some(Glob::union_with(negated, options)?) },
    })
  }

  /// `file` is a slash-normalized absolute path.
  pub fn is_match(&self, file: &str) -> bool {
    let file = file.as_bytes();
    self.affirmed.is_match(file) && !self.negated.as_ref().is_some_and(|glob| glob.is_match(file))
  }
}
