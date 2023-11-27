#[derive(Debug)]
pub struct ResolverOptions {
  /// Create aliases to import or require certain modules more easily.
  /// A trailing $ can also be added to the given object's keys to signify an exact match.
  pub alias: Vec<(String, Vec<String>)>,

  /// A list of alias fields in description files.
  /// Specify a field, such as `browser`, to be parsed according to [this specification](https://github.com/defunctzombie/package-browser-field-spec).
  /// Can be a path to json object such as `["path", "to", "exports"]`.
  ///
  /// Default `[]`
  pub alias_fields: Vec<Vec<String>>,

  /// Condition names for exports field which defines entry points of a package.
  /// The key order in the exports field is significant. During condition matching, earlier entries have higher priority and take precedence over later entries.
  ///
  /// Default `[]`
  pub condition_names: Vec<String>,

  /// A list of exports fields in description files.
  /// Can be a path to json object such as `["path", "to", "exports"]`.
  ///
  /// Default `[["exports"]]`.
  pub exports_fields: Vec<Vec<String>>,
  /// Attempt to resolve these extensions in order.
  /// If multiple files share the same name but have different extensions,
  /// will resolve the one with the extension listed first in the array and skip the rest.
  ///
  /// Default `[".js", ".json", ".node"]`
  pub extensions: Vec<String>,

  /// A list of main fields in description files
  ///
  /// Default `["main"]`.
  pub main_fields: Vec<String>,

  /// The filename to be used while resolving directories.
  ///
  /// Default `["index"]`
  pub main_files: Vec<String>,

  /// A list of directories to resolve modules from, can be absolute path or folder name.
  ///
  /// Default `["node_modules"]`
  pub modules: Vec<String>,
  /// Whether to resolve symlinks to their symlinked location.
  /// When enabled, symlinked resources are resolved to their real path, not their symlinked location.
  /// Note that this may cause module resolution to fail when using tools that symlink packages (like npm link).
  ///
  /// Default `true`
  pub symlinks: bool,
}

impl From<ResolverOptions> for oxc_resolver::ResolveOptions {
  fn from(value: ResolverOptions) -> Self {
    Self {
      alias: value
        .alias
        .into_iter()
        .map(|(key, value)| {
          (key, value.into_iter().map(oxc_resolver::AliasValue::Path).collect::<Vec<_>>())
        })
        .collect::<Vec<_>>(),
      alias_fields: value.alias_fields,
      condition_names: value.condition_names,
      exports_fields: value.exports_fields,
      extensions: value.extensions,
      main_fields: value.main_fields,
      main_files: value.main_files,
      modules: value.modules,
      symlinks: value.symlinks,
      ..Self::default()
    }
  }
}
