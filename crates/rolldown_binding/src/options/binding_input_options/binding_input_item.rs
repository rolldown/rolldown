#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingInputItem {
  pub name: Option<String>,
  pub import: String,
}

impl From<BindingInputItem> for rolldown::InputItem {
  fn from(value: BindingInputItem) -> Self {
    Self { name: value.name, import: value.import }
  }
}
