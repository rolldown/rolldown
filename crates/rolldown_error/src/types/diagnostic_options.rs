use std::path::{Path, PathBuf};
use std::sync::Arc;

use sugar_path::SugarPath;

pub type ImportChainLookupFn = Arc<dyn Fn(&str) -> Option<Vec<String>> + Send + Sync>;

pub struct DiagnosticOptions {
  pub cwd: PathBuf,
  pub import_chain_lookup: Option<ImportChainLookupFn>,
}

impl Default for DiagnosticOptions {
  fn default() -> Self {
    Self { 
      cwd: std::env::current_dir().expect("Failed to get current directory"),
      import_chain_lookup: None,
    }
  }
}

impl DiagnosticOptions {
  /// Turns an absolute path into a path relative to the current working directory. This helps make the output consistent across different machines.
  ///
  /// Example: `/Users/you/project/src/index.js` -> `src/index.js` (if cwd is `/Users/you/project`)
  pub fn stabilize_path(&self, path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    if path.is_absolute() {
      path.relative(&self.cwd).to_slash_lossy().into_owned()
    } else {
      path.to_string_lossy().to_string()
    }
  }
}
