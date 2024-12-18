use oxc_transform_napi::JsxOptions;
use rolldown::Jsx;

#[napi_derive::napi]
pub enum BindingJsx {
  Disable,
  Preserve,
  Enable(JsxOptions),
}

impl From<BindingJsx> for Jsx {
  fn from(value: BindingJsx) -> Self {
    match value {
      BindingJsx::Disable => Jsx::Disable,
      BindingJsx::Preserve => Jsx::Preserve,
      BindingJsx::Enable(options) => Jsx::Enable(options.into()),
    }
  }
}
