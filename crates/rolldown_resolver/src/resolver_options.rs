#[derive(Debug)]
pub struct ResolverOptions {
  /// Create aliases to import or require certain modules more easily.
  /// A trailing $ can also be added to the given object's keys to signify an exact match.
  pub alias: Option<Vec<(String, Vec<String>)>>,

  /// A list of alias fields in description files.
  /// Specify a field, such as `browser`, to be parsed according to [this specification](https://github.com/defunctzombie/package-browser-field-spec).
  /// Can be a path to json object such as `["path", "to", "exports"]`.
  ///
  /// Default `[]`
  pub alias_fields: Option<Vec<Vec<String>>>,

  /// Condition names for exports field which defines entry points of a package.
  /// The key order in the exports field is significant. During condition matching, earlier entries have higher priority and take precedence over later entries.
  ///
  /// Default `[]`
  pub condition_names: Option<Vec<String>>,

  /// A list of exports fields in description files.
  /// Can be a path to json object such as `["path", "to", "exports"]`.
  ///
  /// Default `[["exports"]]`.
  pub exports_fields: Option<Vec<Vec<String>>>,
  /// Attempt to resolve these extensions in order.
  /// If multiple files share the same name but have different extensions,
  /// will resolve the one with the extension listed first in the array and skip the rest.
  ///
  /// Default `[".js", ".json", ".node"]`
  pub extensions: Option<Vec<String>>,

  /// A list of main fields in description files
  ///
  /// Default `["main"]`.
  pub main_fields: Option<Vec<String>>,

  /// The filename to be used while resolving directories.
  ///
  /// Default `["index"]`
  pub main_files: Option<Vec<String>>,

  /// A list of directories to resolve modules from, can be absolute path or folder name.
  ///
  /// Default `["node_modules"]`
  pub modules: Option<Vec<String>>,
  /// Whether to resolve symlinks to their symlinked location.
  /// When enabled, symlinked resources are resolved to their real path, not their symlinked location.
  /// Note that this may cause module resolution to fail when using tools that symlink packages (like npm link).
  ///
  /// Default `true`
  pub symlinks: Option<bool>,
}

impl From<ResolverOptions> for oxc_resolver::ResolveOptions {
  fn from(value: ResolverOptions) -> Self {
    Self {
      alias: value
        .alias
        .map(|alias| {
          alias
            .into_iter()
            .map(|(key, value)| {
              (key, value.into_iter().map(oxc_resolver::AliasValue::Path).collect::<Vec<_>>())
            })
            .collect::<Vec<_>>()
        })
        .unwrap_or(vec![]),
      alias_fields: value.alias_fields.unwrap_or(vec![]),
      condition_names: value.condition_names.unwrap_or(vec![]),
      exports_fields: value.exports_fields.unwrap_or(vec![vec!["exports".into()]]),
      extensions: value.extensions.unwrap_or(vec![".js".into(), ".json".into(), ".node".into()]),
      main_fields: value.main_fields.unwrap_or(vec!["main".into()]),
      main_files: value.main_files.unwrap_or(vec!["index".into()]),
      modules: value.modules.unwrap_or(vec!["node_modules".into()]),
      symlinks: value.symlinks.unwrap_or(true),
      tsconfig: None,
      description_files: vec!["package.json".into()],
      enforce_extension: oxc_resolver::EnforceExtension::Auto,
      extension_alias: vec![],
      fallback: vec![],
      fully_specified: false,
      resolve_to_context: false,
      prefer_relative: false,
      prefer_absolute: false,
      restrictions: vec![],
      roots: vec![],
      builtin_modules: false,
    }
  }
}
