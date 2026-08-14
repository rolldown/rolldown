use std::collections::HashMap;

use rolldown_plugin_replace::ReplaceOptions;
use rustc_hash::FxBuildHasher;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingReplacePluginConfig {
  // It's ok we use `HashMap` here, because we don't care about the order of the keys.
  pub values: HashMap<String, String, FxBuildHasher>,
  #[napi(ts_type = "[string, string]")]
  pub delimiters: Option<Vec<String>>,
  pub prevent_assignment: Option<bool>,
  pub object_guards: Option<bool>,
  pub sourcemap: Option<bool>,
}

impl TryFrom<BindingReplacePluginConfig> for ReplaceOptions {
  type Error = napi::Error;

  fn try_from(config: BindingReplacePluginConfig) -> Result<Self, Self::Error> {
    let delimiters = match config.delimiters {
      None => None,
      Some(raw) if raw.len() == 2 => Some((raw[0].clone(), raw[1].clone())),
      Some(raw) => {
        return Err(napi::Error::new(
          napi::Status::InvalidArg,
          format!("`delimiters` expects a tuple of two strings, but got {} element(s)", raw.len()),
        ));
      }
    };
    Ok(Self {
      values: config.values,
      delimiters,
      prevent_assignment: config.prevent_assignment.unwrap_or(false),
      object_guards: config.object_guards.unwrap_or(false),
      sourcemap: config.sourcemap.unwrap_or(false),
    })
  }
}
