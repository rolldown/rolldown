use oxc_resolver::ResolveError;

/// rewrite error reason and ignore path param
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
    ResolveError::Builtin { .. } => "Builtin module".to_string(),
    ResolveError::ExtensionAlias { .. } => {
      "All of the aliased extensions are not found".to_string()
    }
    ResolveError::Specifier(_) => "The provided path specifier cannot be parsed".to_string(),
    ResolveError::Json(_) => "JSON parse error".to_string(),
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
    ResolveError::MatchedAliasNotFound(_, _) => "Matched alias not found".to_string(),
    _ => todo!(),
  }
}
