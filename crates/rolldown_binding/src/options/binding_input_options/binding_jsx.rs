use oxc_transform_napi::JsxOptions;
use rolldown::Jsx;

/// TODO: support `preserve-react` mode
#[napi_derive::napi]
pub enum BindingJsx {
  Disable,
  Preserve,
  React,
  ReactJsx,
  Enable(JsxOptions),
}

impl From<BindingJsx> for Jsx {
  fn from(value: BindingJsx) -> Self {
    use oxc::transformer;
    match value {
      BindingJsx::Disable => Jsx::Disable,
      BindingJsx::Preserve => Jsx::Preserve,
      BindingJsx::Enable(options) => Jsx::Enable(options.into()),
      BindingJsx::React => {
        let options = oxc::transformer::JsxOptions {
          runtime: transformer::JsxRuntime::Classic,
          ..Default::default()
        };
        Jsx::Enable(options)
      }
      BindingJsx::ReactJsx => Jsx::Enable(transformer::JsxOptions::default()),
    }
  }
}
