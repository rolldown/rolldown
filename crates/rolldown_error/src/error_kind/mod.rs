use std::{
  borrow::Cow,
  fmt::Display,
  path::{self, Path, PathBuf},
};

use crate::utils::format_quoted_strings;
type StaticStr = Cow<'static, str>;

pub mod error_code;

#[derive(Debug)]
pub enum ErrorKind {
  // --- Aligned with rollup
  UnresolvedEntry {
    unresolved_id: PathBuf,
  },
  ExternalEntry {
    id: PathBuf,
  },
  MissingExport {
    importer: PathBuf,
    importee: PathBuf,
    missing_export: StaticStr,
  },
  AmbiguousExternalNamespaces {
    reexporting_module: PathBuf,
    used_module: PathBuf,
    binding: StaticStr,
    sources: Vec<PathBuf>,
  },
  CircularDependency(Vec<PathBuf>),
  InvalidExportOptionValue(StaticStr),
  IncompatibleExportOptionValue {
    option_value: &'static str,
    exported_keys: Vec<StaticStr>,
    entry_module: PathBuf,
  },
  ShimmedExport {
    binding: StaticStr,
    exporter: PathBuf,
  },
  CircularReexport {
    exporter: PathBuf,
    export_name: StaticStr,
  },

  UnresolvedImport {
    specifier: StaticStr,
    importer: PathBuf,
  },

  // --- Custom
  Napi {
    status: String,
    reason: String,
  },

  IoError(std::io::Error),
}

trait Mock {
  fn pretty_display(&self) -> path::Display<'_>;
}

impl Mock for Path {
  fn pretty_display(&self) -> path::Display<'_> {
    self.display()
  }
}

impl Display for ErrorKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // Aligned with rollup
      ErrorKind::UnresolvedEntry { unresolved_id } => write!(f, "Could not resolve entry module \"{}\"", unresolved_id.pretty_display()),
      ErrorKind::ExternalEntry { id } => write!(f, "Entry module \"{}\" cannot be external.", id.pretty_display()),
      ErrorKind::MissingExport { missing_export, importee, importer } => write!(
        f,
        r#""{missing_export}" is not exported by "{}", imported by "{}"."#,
        importee.pretty_display(),
        importer.pretty_display(),
      ),
      ErrorKind::AmbiguousExternalNamespaces {
        binding,
        reexporting_module,
        used_module,
        sources,
      } => write!(
        f,
        "Ambiguous external namespace resolution: \"{}\" re-exports \"{binding}\" from one of the external modules {}, guessing \"{}\".",
        reexporting_module.pretty_display(),
        format_quoted_strings(&sources.iter().map(|p| p.pretty_display().to_string()).collect::<Vec<_>>()),
        used_module.pretty_display(),
      ),
      ErrorKind::CircularDependency(path) => write!(f, "Circular dependency: {}", path.iter().map(|p| p.pretty_display().to_string()).collect::<Vec<_>>().join(" -> ")),
      ErrorKind::InvalidExportOptionValue(value) =>  write!(f, r#""output.exports" must be "default", "named", "none", "auto", or left unspecified (defaults to "auto"), received "{value}"."#),
      ErrorKind::IncompatibleExportOptionValue { option_value, exported_keys, entry_module } => {
        let mut exported_keys = exported_keys.iter().collect::<Vec<_>>();
        exported_keys.sort();
        write!(f, r#""{option_value}" was specified for "output.exports", but entry module "{}" has the following exports: {}"#, entry_module.pretty_display(), format_quoted_strings(&exported_keys))
      }
      ErrorKind::ShimmedExport { binding, exporter } => write!(f, r#"Missing export "{binding}" has been shimmed in module "{}"."#, exporter.pretty_display()),
      ErrorKind::CircularReexport { export_name, exporter } => write!(f, r#""{export_name}" cannot be exported from "{}" as it is a reexport that references itself."#, exporter.pretty_display()),
      ErrorKind::UnresolvedImport { specifier, importer } => write!(f, r#"Could not resolve "{specifier}" from "{}""#, importer.pretty_display()),
      ErrorKind::IoError(e) => e.fmt(f),
      ErrorKind::Napi { status: _, reason: _ } => todo!()
    }
  }
}

impl ErrorKind {
  pub fn code(&self) -> &'static str {
    match self {
      // Aligned with rollup
      ErrorKind::UnresolvedEntry { .. } => error_code::UNRESOLVED_ENTRY,
      ErrorKind::ExternalEntry { .. } => error_code::UNRESOLVED_ENTRY,
      ErrorKind::MissingExport { .. } => error_code::MISSING_EXPORT,
      ErrorKind::AmbiguousExternalNamespaces { .. } => error_code::AMBIGUOUS_EXTERNAL_NAMESPACES,
      ErrorKind::CircularDependency(_) => error_code::CIRCULAR_DEPENDENCY,
      ErrorKind::InvalidExportOptionValue(_) => error_code::INVALID_EXPORT_OPTION,
      ErrorKind::IncompatibleExportOptionValue { .. } => error_code::INVALID_EXPORT_OPTION,
      ErrorKind::ShimmedExport { .. } => error_code::SHIMMED_EXPORT,
      ErrorKind::CircularReexport { .. } => error_code::CIRCULAR_REEXPORT,
      ErrorKind::UnresolvedImport { .. } => error_code::UNRESOLVED_IMPORT,
      // Rolldown specific
      ErrorKind::IoError(_) => error_code::IO_ERROR,
      ErrorKind::Napi { .. } => todo!(),
    }
  }
}
