use napi::bindgen_prelude::*;
use rolldown_common::InlineConstOption;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingOptimization {
  #[napi(ts_type = "boolean | 'safe'")]
  pub inline_const: Option<Either<bool, String>>,
  pub pife_for_module_wrappers: Option<bool>,
}

impl TryFrom<BindingOptimization> for rolldown_common::OptimizationOption {
  type Error = napi::Error;

  fn try_from(value: BindingOptimization) -> std::result::Result<Self, Self::Error> {
    Ok(Self {
      inline_const: value
        .inline_const
        .map(|either| match either {
          Either::A(bool_val) => Ok(InlineConstOption::Bool(bool_val)),
          Either::B(string_val) => {
            if string_val.as_str() == "safe" {
              Ok(InlineConstOption::Safe)
            } else {
              Err(napi::Error::from_reason(
                "Invalid value for inline_const: expected `'safe'` or `boolean`".to_string(),
              ))
            }
          }
        })
        .transpose()?,
      pife_for_module_wrappers: value.pife_for_module_wrappers,
    })
  }
}
