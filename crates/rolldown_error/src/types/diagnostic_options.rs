use std::path::{Path, PathBuf};

use sugar_path::SugarPath;

pub struct DiagnosticOptions {
  pub cwd: PathBuf,
}

impl Default for DiagnosticOptions {
  fn default() -> Self {
    Self { cwd: std::env::current_dir().expect("Failed to get current directory") }
  }
}

impl DiagnosticOptions {
  /// Turns an absolute path into a path relative to the current working directory. This helps make the output consistent across different machines.
  ///
  /// Example: `/Users/you/project/src/index.js` -> `src/index.js` (if cwd is `/Users/you/project`)
  pub fn stabilize_path(&self, path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let result = if path.is_absolute() {
      path.relative(&self.cwd).to_slash_lossy().into_owned()
    } else {
      path.to_string_lossy().to_string()
    };
    // Escape virtual module prefix (\0 → \\0) so null bytes don't appear in diagnostics
    if result.contains('\0') { result.replace('\0', "\\0") } else { result }
  }
}
