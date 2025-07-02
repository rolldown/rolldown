use crate::types::binding_resolve_alias_item::AliasItem;
use crate::types::binding_resolve_extension_alias::ExtensionAliasItem;

#[napi_derive::napi(object)]
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
  pub tsconfig_filename: Option<String>,
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
      tsconfig_filename: value.tsconfig_filename,
      yarn_pnp: value.yarn_pnp,
    }
  }
}
