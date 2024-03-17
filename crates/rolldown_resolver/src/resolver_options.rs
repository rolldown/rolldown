/// A simple wrapper around `oxc_resolver::ResolveOptions` to make it easier to use in the `rolldown_resolver` crate.
/// See [oxc_resolver::ResolveOptions](https://docs.rs/oxc_resolver/latest/oxc_resolver/struct.ResolveOptions.html) for more information.
#[derive(Debug, Default)]
pub struct ResolveOptions {
  pub alias: Option<Vec<(String, Vec<String>)>>,
  pub alias_fields: Option<Vec<Vec<String>>>,
  pub condition_names: Option<Vec<String>>,
  pub exports_fields: Option<Vec<Vec<String>>>,
  pub extensions: Option<Vec<String>>,
  pub main_fields: Option<Vec<String>>,
  pub main_files: Option<Vec<String>>,
  pub modules: Option<Vec<String>>,
  pub symlinks: Option<bool>,
}

impl From<ResolveOptions> for oxc_resolver::ResolveOptions {
  fn from(value: ResolveOptions) -> Self {
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
        .unwrap_or_default(),
      alias_fields: value.alias_fields.unwrap_or_default(),
      condition_names: value.condition_names.unwrap_or_default(),
      exports_fields: value.exports_fields.unwrap_or_else(|| vec![vec!["exports".into()]]),
      extensions: value
        .extensions
        .unwrap_or_else(|| vec![".js".into(), ".json".into(), ".node".into()]),
      main_fields: value.main_fields.unwrap_or_else(|| vec!["main".into()]),
      main_files: value.main_files.unwrap_or_else(|| vec!["index".into()]),
      modules: value.modules.unwrap_or_else(|| vec!["node_modules".into()]),
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
