use rolldown_plugin_vite_alias::{Alias, ViteAliasPlugin};

use crate::types::binding_string_or_regex::BindingStringOrRegex;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingViteAliasPluginConfig {
  pub entries: Vec<BindingViteAliasPluginAlias>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingViteAliasPluginAlias {
  pub find: BindingStringOrRegex,
  pub replacement: String,
}

impl TryFrom<BindingViteAliasPluginConfig> for ViteAliasPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingViteAliasPluginConfig) -> Result<Self, Self::Error> {
    let mut ret = Vec::with_capacity(value.entries.len());
    for item in value.entries {
      ret.push(Alias { find: item.find.into(), replacement: item.replacement });
    }

    Ok(Self { entries: ret })
  }
}
