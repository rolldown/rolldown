use std::{
  borrow::Cow,
  fmt::Display,
  path::{Path, PathBuf},
};

use crate::error_kind::ErrorKind;

type StaticStr = Cow<'static, str>;

// TODO reduce struct size
#[derive(Debug)]
pub struct Error {
  contexts: Vec<Cow<'static, str>>,
  kind: ErrorKind,
}

impl PartialEq for Error {
  fn eq(&self, other: &Self) -> bool {
    self.kind.to_string().eq(&other.kind.to_string())
  }
}

impl Eq for Error {}

impl PartialOrd for Error {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.kind.to_string().partial_cmp(&other.kind.to_string())
  }
}

impl Ord for Error {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.kind.to_string().cmp(&other.kind.to_string())
  }
}

impl Error {
  fn with_kind(kind: ErrorKind) -> Self {
    Self { contexts: vec![], kind }
  }

  pub fn context(&mut self, context: impl Into<StaticStr>) -> &mut Self {
    self.contexts.push(context.into());
    self
  }

  // --- Aligned with rollup
  pub fn entry_cannot_be_external(unresolved_id: impl AsRef<Path>) -> Self {
    Self::with_kind(ErrorKind::ExternalEntry { id: unresolved_id.as_ref().to_path_buf() })
  }

  pub fn ambiguous_external_namespaces(
    binding: impl Into<StaticStr>,
    reexporting_module: PathBuf,
    used_module: PathBuf,
    sources: Vec<PathBuf>,
  ) -> Self {
    Self::with_kind(ErrorKind::AmbiguousExternalNamespaces {
      reexporting_module,
      used_module,
      binding: binding.into(),
      sources,
    })
  }

  pub fn unresolved_entry(unresolved_id: impl AsRef<Path>) -> Self {
    Self::with_kind(ErrorKind::UnresolvedEntry {
      unresolved_id: unresolved_id.as_ref().to_path_buf(),
    })
  }

  pub fn missing_export(
    missing_export: impl Into<StaticStr>,
    importer: impl AsRef<Path>,
    importee: impl AsRef<Path>,
  ) -> Self {
    let importer = importer.as_ref().to_path_buf();
    let importee = importee.as_ref().to_path_buf();
    Self::with_kind(ErrorKind::MissingExport {
      importer,
      importee,
      missing_export: missing_export.into(),
    })
  }

  pub fn circular_dependency(circular_path: Vec<String>) -> Self {
    Self::with_kind(ErrorKind::CircularDependency(
      circular_path.into_iter().map(PathBuf::from).collect(),
    ))
  }

  pub fn invalid_export_option_value(value: impl Into<StaticStr>) -> Self {
    Self::with_kind(ErrorKind::InvalidExportOptionValue(value.into()))
  }

  pub fn incompatible_export_option_value(
    option_value: &'static str,
    exported_keys: Vec<impl Into<StaticStr>>,
    entry_module: impl AsRef<Path>,
  ) -> Self {
    let entry_module = entry_module.as_ref().to_path_buf();
    Self::with_kind(ErrorKind::IncompatibleExportOptionValue {
      option_value,
      exported_keys: exported_keys.into_iter().map(Into::into).collect(),
      entry_module,
    })
  }

  pub fn shimmed_export(binding: impl Into<StaticStr>, exporter: PathBuf) -> Self {
    Self::with_kind(ErrorKind::ShimmedExport { binding: binding.into(), exporter })
  }

  pub fn circular_reexport(export_name: impl Into<StaticStr>, exporter: PathBuf) -> Self {
    Self::with_kind(ErrorKind::CircularReexport { exporter, export_name: export_name.into() })
  }

  pub fn unresolved_import(specifier: impl Into<StaticStr>, importer: impl Into<PathBuf>) -> Self {
    Self::with_kind(ErrorKind::UnresolvedImport {
      specifier: specifier.into(),
      importer: importer.into(),
    })
  }

  // --- rolldown specific

  // --- TODO: we should remove following errors

  pub fn io_error(e: std::io::Error) -> Self {
    Self::with_kind(ErrorKind::IoError(e))
  }

  pub fn napi_error(status: String, reason: String) -> Self {
    Self::with_kind(ErrorKind::Napi { status, reason })
  }
}

impl Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.kind.fmt(f)
  }
}
