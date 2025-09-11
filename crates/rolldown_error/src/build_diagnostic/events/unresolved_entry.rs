use oxc_resolver::ResolveError;

use super::BuildEvent;
use crate::types::diagnostic_options::DiagnosticOptions;
use std::path::PathBuf;

#[derive(Debug)]
pub struct UnresolvedEntry {
  pub(crate) unresolved_id: PathBuf,
  pub(crate) resolve_error: Option<ResolveError>,
}

impl BuildEvent for UnresolvedEntry {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::UnresolvedEntry
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let mut message =
      vec![format!("Cannot resolve entry module {}.", opts.stabilize_path(&self.unresolved_id))];

    match &self.resolve_error {
      Some(ResolveError::PackagePathNotExported(pkg_subpath, pkg_json_path)) => {
        message.push(format!(
          r#"- Package subpath '{pkg_subpath}' is not defined by "exports" in {pkg_json_path}"#,
          pkg_json_path = opts.stabilize_path(pkg_json_path),
        ));
      }
      _ => {}
    }

    message.join("\n")
  }
}
