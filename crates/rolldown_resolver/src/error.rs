use oxc_resolver::ResolveError;

/// rewrite error reason ignore path
/// Note this is just the error message fallback, for better dx,
/// you could polish the error message case by case in the caller side
pub fn oxc_resolve_error_to_reason(e: &ResolveError) -> String {
  match e {
    ResolveError::Ignored(_) => "Path is ignored".to_string(),
    ResolveError::NotFound(_) => "Cannot find module".to_string(),
    ResolveError::TsconfigNotFound(_) => "Tsconfig not found".to_string(),
    ResolveError::TsconfigSelfReference(_) => {
      "Tsconfig's project reference path points to this tsconfig".to_string()
    }
    ResolveError::IOError(_) => "IO error".to_string(),
    ResolveError::Builtin(_) => "Builtin module".to_string(),
    ResolveError::ExtensionAlias(_) => "All of the aliased extensions are not found".to_string(),
    ResolveError::Specifier(_) => "The provided path specifier cannot be parsed".to_string(),
    ResolveError::JSON(_) => "JSON parse error".to_string(),
    ResolveError::Restriction(_, _) => "Path restriction".to_string(),
    ResolveError::InvalidModuleSpecifier(_, _) => "Invalid module specifier".to_string(),
    ResolveError::InvalidPackageTarget(_, _, _) => "Invalid package target".to_string(),
    ResolveError::PackagePathNotExported(_, _) => {
      "Package subpath is not defined by exports".to_string()
    }
    ResolveError::InvalidPackageConfig(_) => "Invalid package config".to_string(),
    ResolveError::InvalidPackageConfigDefault(_) => {
      "Default condition should be last one in package config".to_string()
    }
    ResolveError::InvalidPackageConfigDirectory(_) => {
      "Expecting folder to folder mapping".to_string()
    }
    ResolveError::PackageImportNotDefined(_, _) => {
      "Package import specifier is not defined".to_string()
    }
    ResolveError::Unimplemented(_) => "Unimplemented".to_string(),
    ResolveError::Recursion => "Recursion in resolving".to_string(),
  }
}
//   /// Node.js builtin modules
//   ///
//   /// This is an error due to not being a Node.js runtime.
//   /// The `alias` option can be used to resolve a builtin module to a polyfill.
//   #[error("Builtin module {0}")]
//   Builtin(String),
//
//   /// All of the aliased extension are not found
//   #[error("All of the aliased extensions are not found for {0}")]
//   ExtensionAlias(PathBuf),
//
//   /// The provided path specifier cannot be parsed
//   #[error("{0}")]
//   Specifier(SpecifierError),
//
//   /// JSON parse error
//   #[error("{0:?}")]
//   JSON(JSONError),
//
//   /// Restricted by `ResolveOptions::restrictions`
//   #[error(r#"Path "{0}" restricted by {0}"#)]
//   Restriction(PathBuf, PathBuf),
//
//   #[error(
//     r#"Invalid module "{0}" specifier is not a valid subpath for the "exports" resolution of {1}"#
//   )]
//   InvalidModuleSpecifier(String, PathBuf),
//
//   #[error(r#"Invalid "exports" target "{0}" defined for '{1}' in the package config {2}"#)]
//   InvalidPackageTarget(String, String, PathBuf),
//
//   #[error(r#"Package subpath '{0}' is not defined by "exports" in {1}"#)]
//   PackagePathNotExported(String, PathBuf),
//
//   #[error(r#"Invalid package config "{0}", "exports" cannot contain some keys starting with '.' and some not. The exports object must either be an object of package subpath keys or an object of main entry condition name keys only."#)]
//   InvalidPackageConfig(PathBuf),
//
//   #[error(r#"Default condition should be last one in "{0}""#)]
//   InvalidPackageConfigDefault(PathBuf),
//
//   #[error(r#"Expecting folder to folder mapping. "{0}" should end with "/"#)]
//   InvalidPackageConfigDirectory(PathBuf),
//
//   #[error(r#"Package import specifier "{0}" is not defined in package {1}"#)]
//   PackageImportNotDefined(String, PathBuf),
//
//   #[error("{0} is unimplemented")]
//   Unimplemented(&'static str),
//
//   /// Occurs when alias paths reference each other.
//   #[error("Recursion in resolving")]
//   Recursion,
// }
