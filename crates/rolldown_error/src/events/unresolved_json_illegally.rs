use std::path::PathBuf;

use super::BuildEvent;

#[derive(Debug)]
pub struct UnresolvedJsonIllegally {
  pub(crate) line: usize,
  pub(crate) col: usize,
  pub(crate) path: PathBuf,
  pub(crate) message: String,
}

impl BuildEvent for UnresolvedJsonIllegally {
  fn kind(&self) -> crate::EventKind {
    crate::EventKind::UnresolvedJsonIllegally
  }

  fn message(&self, opts: &crate::DiagnosticOptions) -> String {
    format!(
      "Could not resolve package json,path is {} issue location is line {} and col is {}. and detail is \n {}",
      opts.stabilize_path(&self.path), self.line, self.col,self.message
    )
  }
}
