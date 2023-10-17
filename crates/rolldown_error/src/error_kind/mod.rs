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
      Self::UnresolvedEntry { unresolved_id } => write!(f, "Could not resolve entry module \"{}\"", unresolved_id.pretty_display()),
      Self::ExternalEntry { id } => write!(f, "Entry module \"{}\" cannot be external.", id.pretty_display()),
      Self::MissingExport { missing_export, importee, importer } => write!(
        f,
        r#""{missing_export}" is not exported by "{}", imported by "{}"."#,
        importee.pretty_display(),
        importer.pretty_display(),
      ),
      Self::AmbiguousExternalNamespaces {
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
      Self::CircularDependency(path) => write!(f, "Circular dependency: {}", path.iter().map(|p| p.pretty_display().to_string()).collect::<Vec<_>>().join(" -> ")),
      Self::InvalidExportOptionValue(value) =>  write!(f, r#""output.exports" must be "default", "named", "none", "auto", or left unspecified (defaults to "auto"), received "{value}"."#),
      Self::IncompatibleExportOptionValue { option_value, exported_keys, entry_module } => {
        let mut exported_keys = exported_keys.iter().collect::<Vec<_>>();
        exported_keys.sort();
        write!(f, r#""{option_value}" was specified for "output.exports", but entry module "{}" has the following exports: {}"#, entry_module.pretty_display(), format_quoted_strings(&exported_keys))
      }
      Self::ShimmedExport { binding, exporter } => write!(f, r#"Missing export "{binding}" has been shimmed in module "{}"."#, exporter.pretty_display()),
      Self::CircularReexport { export_name, exporter } => write!(f, r#""{export_name}" cannot be exported from "{}" as it is a reexport that references itself."#, exporter.pretty_display()),
      Self::UnresolvedImport { specifier, importer } => write!(f, r#"Could not resolve "{specifier}" from "{}""#, importer.pretty_display()),
      Self::IoError(e) => e.fmt(f),
      Self::Napi { status: _, reason: _ } => unimplemented!()
    }
  }
}

impl ErrorKind {
  #[allow(dead_code)]
  pub fn code(&self) -> &'static str {
    match self {
      // Aligned with rollup
      Self::UnresolvedEntry { .. } | Self::ExternalEntry { .. } => error_code::UNRESOLVED_ENTRY,
      Self::MissingExport { .. } => error_code::MISSING_EXPORT,
      Self::AmbiguousExternalNamespaces { .. } => error_code::AMBIGUOUS_EXTERNAL_NAMESPACES,
      Self::CircularDependency(_) => error_code::CIRCULAR_DEPENDENCY,
      Self::IncompatibleExportOptionValue { .. } | Self::InvalidExportOptionValue(_) => {
        error_code::INVALID_EXPORT_OPTION
      }
      Self::ShimmedExport { .. } => error_code::SHIMMED_EXPORT,
      Self::CircularReexport { .. } => error_code::CIRCULAR_REEXPORT,
      Self::UnresolvedImport { .. } => error_code::UNRESOLVED_IMPORT,
      // Rolldown specific
      Self::IoError(_) => error_code::IO_ERROR,
      Self::Napi { .. } => todo!(),
    }
  }
}
