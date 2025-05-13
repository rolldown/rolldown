use oxc_resolver::ResolveError;

use crate::EventKind;

use super::BuildEvent;

#[derive(Debug)]
pub struct UnresolvedImportTreatedAsExternal {
  pub importer: String,
  pub specifier: String,
  pub resolve_error: Option<ResolveError>,
}

impl BuildEvent for UnresolvedImportTreatedAsExternal {
  fn kind(&self) -> crate::EventKind {
    EventKind::UnresolvedImport
  }

  fn id(&self) -> Option<String> {
    Some(self.importer.clone())
  }

  fn message(&self, opts: &crate::DiagnosticOptions) -> String {
    // https://github.com/rollup/rollup/blob/fe6cb3a291df245408ef2bdc708fc64fa4ecb262/src/utils/logs.ts#L1031-L1041
    let mut message = vec![format!(
      "{importee:?} is imported by {importer:?}, but could not be resolved – treating it as an external dependency.",
      importee = self.specifier,
      importer = opts.stabilize_path(&self.importer)
    )];

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
