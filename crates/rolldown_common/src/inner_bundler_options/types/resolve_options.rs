pub use oxc_resolver::{TsconfigOptions, TsconfigReferences};

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// A simple wrapper around `oxc_resolver::ResolveOptions` to make it easier to use in the `rolldown_resolver` crate.
/// See [oxc_resolver::ResolveOptions](https://docs.rs/oxc_resolver/latest/oxc_resolver/struct.ResolveOptions.html) for more information.
#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ResolveOptions {
  pub alias: Option<Vec<(String, Vec<String>)>>,
  pub alias_fields: Option<Vec<Vec<String>>>,
  pub condition_names: Option<Vec<String>>,
  pub exports_fields: Option<Vec<Vec<String>>>,
  pub extensions: Option<Vec<String>>,
  pub extension_alias: Option<Vec<(String, Vec<String>)>>,
  pub main_fields: Option<Vec<String>>,
  pub main_files: Option<Vec<String>>,
  pub symlinks: Option<bool>,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(default, deserialize_with = "deserialize_tsconfig"),
    schemars(with = "Option<String>")
  )]
  pub tsconfig: Option<TsconfigOptions>,
  pub yarn_pnp: Option<bool>,
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_tsconfig<'de, D>(deserializer: D) -> Result<Option<TsconfigOptions>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  Ok(Option::<String>::deserialize(deserializer)?.map(|v| TsconfigOptions {
    config_file: std::path::PathBuf::from(v),
    references: TsconfigReferences::Disabled,
  }))
}
