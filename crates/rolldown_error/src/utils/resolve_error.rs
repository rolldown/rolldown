use oxc_resolver::ResolveError;

use crate::types::diagnostic_options::DiagnosticOptions;

/// Converts an oxc resolver error to a human-readable error message.
///
/// This function extracts the error reason and includes relevant path information.
/// Paths are stabilized (made relative to cwd) for consistent output.
///
/// # Arguments
/// * `error` - The resolve error from oxc_resolver
/// * `opts` - Diagnostic options for path stabilization
///
/// # Returns
/// A string describing the error reason in a user-friendly format.
pub fn resolve_error_to_message(error: &ResolveError, opts: &DiagnosticOptions) -> String {
  match error {
    ResolveError::Ignored(_) => "Path is ignored".to_string(),
    ResolveError::NotFound(_) => "Cannot find module".to_string(),
    ResolveError::TsconfigNotFound(_) => "Tsconfig not found".to_string(),
    ResolveError::TsconfigSelfReference(_) => {
      "Tsconfig's project reference path points to itself".to_string()
    }
    ResolveError::TsconfigCircularExtend(_) => {
      "Circular reference detected in tsconfig 'extends'".to_string()
    }
    ResolveError::IOError(_) => "I/O error occurred".to_string(),
    ResolveError::Builtin { .. } => "Builtin module".to_string(),
    ResolveError::ExtensionAlias { .. } => "None of the aliased extensions were found".to_string(),
    ResolveError::Specifier(_) => "The provided path specifier cannot be parsed".to_string(),
    ResolveError::Json(json_error) => {
      let path = opts.stabilize_path(&json_error.path);
      format!("JSON parse error in '{path}'")
    }
    ResolveError::InvalidModuleSpecifier(..) => "Invalid module specifier".to_string(),
    ResolveError::InvalidPackageTarget(..) => "Invalid package target".to_string(),
    ResolveError::PackagePathNotExported { .. } => {
      "Package subpath is not defined by exports".to_string()
    }
    ResolveError::InvalidPackageConfig(_) => "Invalid package configuration".to_string(),
    ResolveError::InvalidPackageConfigDefault(_) => {
      "Default condition should be last in package configuration".to_string()
    }
    ResolveError::InvalidPackageConfigDirectory(_) => {
      "Expected folder-to-folder mapping in package configuration".to_string()
    }
    ResolveError::PackageImportNotDefined(..) => {
      "Package import specifier is not defined".to_string()
    }
    ResolveError::Unimplemented(_) => "Feature not yet implemented".to_string(),
    ResolveError::Recursion => "Circular dependency detected during resolution".to_string(),
    ResolveError::MatchedAliasNotFound(..) => "Matched alias target not found".to_string(),
    _ => error.to_string(),
  }
}
