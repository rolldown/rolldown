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
      Some(ResolveError::PackagePathNotExported {
        subpath,
        package_path: _,
        package_json_path,
        conditions: _,
      }) => {
        message.push(format!(
          r#"- Package subpath '{subpath}' is not defined by "exports" in {package_json_path}"#,
          package_json_path = opts.stabilize_path(package_json_path),
        ));
      }
      _ => {}
    }

    message.join("\n")
  }
}
