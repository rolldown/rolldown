use rolldown::{InnerOptions, ModuleSideEffects};
use rolldown_utils::js_regex::HybridRegex;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingTreeshake {
  pub module_side_effects: String,
}

impl TryFrom<BindingTreeshake> for rolldown::TreeshakeOptions {
  fn try_from(value: BindingTreeshake) -> anyhow::Result<Self> {
    match value.module_side_effects.as_str() {
      "true" => {
        Ok(Self::Option(InnerOptions { module_side_effects: ModuleSideEffects::Boolean(true) }))
      }
      "false" => {
        Ok(Self::Option(InnerOptions { module_side_effects: ModuleSideEffects::Boolean(false) }))
      }
      _ => {
        let regex = HybridRegex::new(&value.module_side_effects)?;
        Ok(Self::Option(InnerOptions { module_side_effects: ModuleSideEffects::Regex(regex) }))
      }
    }
  }

  type Error = anyhow::Error;
}
