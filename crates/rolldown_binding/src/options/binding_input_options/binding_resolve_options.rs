use std::path::PathBuf;

use napi::Either;
use napi_derive::napi;

use crate::types::binding_resolve_alias_item::AliasItem;
use crate::types::binding_resolve_extension_alias::ExtensionAliasItem;

#[napi(object)]
#[derive(Debug, Default)]
pub struct BindingResolveOptions {
  // Option<Vec<(String, Vec<String>)>>> is better, maybe NAPI-RS should support tuples.
  pub alias: Option<Vec<AliasItem>>,
  pub alias_fields: Option<Vec<Vec<String>>>,
  pub condition_names: Option<Vec<String>>,
  pub exports_fields: Option<Vec<Vec<String>>>,
  pub extensions: Option<Vec<String>>,
  pub extension_alias: Option<Vec<ExtensionAliasItem>>,
  pub main_fields: Option<Vec<String>>,
  pub main_files: Option<Vec<String>>,
  pub modules: Option<Vec<String>>,
  pub symlinks: Option<bool>,
  pub tsconfig: Option<BindingResolveTsconfigOptions>,
  pub yarn_pnp: Option<bool>,
}

impl From<BindingResolveOptions> for rolldown::ResolveOptions {
  fn from(value: BindingResolveOptions) -> Self {
    Self {
      alias: value.alias.map(|alias| {
        alias
          .into_iter()
          .map(|alias_item| (alias_item.find, alias_item.replacements))
          .collect::<Vec<_>>()
      }),
      alias_fields: value.alias_fields,
      condition_names: value.condition_names,
      exports_fields: value.exports_fields,
      extensions: value.extensions,
      extension_alias: value.extension_alias.map(|alias| {
        alias.into_iter().map(|item| (item.target, item.replacements)).collect::<Vec<_>>()
      }),
      main_fields: value.main_fields,
      main_files: value.main_files,
      symlinks: value.symlinks,
      tsconfig: value.tsconfig.map(Into::into),
      yarn_pnp: value.yarn_pnp,
    }
  }
}

#[napi(object)]
#[derive(Debug)]
pub struct BindingResolveTsconfigOptions {
  pub config_file: String,
  #[napi(ts_type = r"'auto' | string[]")]
  pub references: Option<Either<String, Vec<String>>>,
}

// TODO: use `oxc_resolver_napi::TsconfigOptions` instead
impl From<BindingResolveTsconfigOptions> for oxc_resolver::TsconfigOptions {
  fn from(value: BindingResolveTsconfigOptions) -> Self {
    Self {
      config_file: PathBuf::from(value.config_file),
      references: match value.references {
        Some(Either::A(string)) if string.as_str() == "auto" => {
          oxc_resolver::TsconfigReferences::Disabled
        }
        Some(Either::A(opt)) => {
          panic!("`{opt}` is not a valid option for  tsconfig references")
        }
        Some(Either::B(paths)) => oxc_resolver::TsconfigReferences::Paths(
          paths.into_iter().map(PathBuf::from).collect::<Vec<_>>(),
        ),
        None => oxc_resolver::TsconfigReferences::Disabled,
      },
    }
  }
}
