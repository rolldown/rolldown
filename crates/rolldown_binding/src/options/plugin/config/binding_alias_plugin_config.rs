use rolldown_plugin_alias::{Alias, AliasPlugin};

use crate::types::binding_string_or_regex::BindingStringOrRegex;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingAliasPluginConfig {
  pub entries: Vec<BindingAliasPluginAlias>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingAliasPluginAlias {
  pub find: BindingStringOrRegex,
  pub replacement: String,
}

impl TryFrom<BindingAliasPluginConfig> for AliasPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingAliasPluginConfig) -> Result<Self, Self::Error> {
    let mut ret = Vec::with_capacity(value.entries.len());
    for item in value.entries {
      ret.push(Alias { find: item.find.into(), replacement: item.replacement });
    }

    Ok(Self { entries: ret })
  }
}
