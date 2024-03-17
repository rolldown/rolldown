use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingInputItem {
  pub name: Option<String>,
  pub import: String,
}

impl From<BindingInputItem> for rolldown::InputItem {
  fn from(value: BindingInputItem) -> Self {
    Self { name: value.name, import: value.import }
  }
}
