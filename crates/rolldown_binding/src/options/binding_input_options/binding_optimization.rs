use rolldown::{InlineConstOption, InlineConstOptionInner};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingOptimization {
  pub inline_const: Option<BindingInlineConst>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingInlineConst {
  pub pass: Option<u32>,
}

impl From<BindingOptimization> for rolldown_common::OptimizationOption {
  fn from(value: BindingOptimization) -> Self {
    Self {
      inline_const: Some(InlineConstOption::Option(InlineConstOptionInner {
        pass: value
          .inline_const
          .and_then(|item| {
            let pass = item.pass?;
            Some(u8::try_from(pass).expect("should not greater than 255 nor less than 0"))
          })
          .unwrap_or(1),
      })),
    }
  }
}
