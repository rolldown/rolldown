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
  pub fn stabilize_path(&self, path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    if path.is_absolute() {
      path.relative(&self.cwd).to_slash_lossy().into_owned()
    } else {
      path.to_string_lossy().to_string()
    }
  }
}
