use napi::bindgen_prelude::*;
use rolldown_common::{InlineConstConfig, InlineConstMode, InlineConstOption};
use rolldown_error::BuildDiagnostic;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingInlineConstConfig {
  pub mode: Option<String>,
  pub pass: Option<u32>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingOptimization {
  #[napi(ts_type = "boolean | BindingInlineConstConfig")]
  pub inline_const: Option<Either<bool, BindingInlineConstConfig>>,
  pub pife_for_module_wrappers: Option<bool>,
}

impl TryFrom<BindingOptimization> for rolldown_common::OptimizationOption {
  type Error = BuildDiagnostic;

  fn try_from(value: BindingOptimization) -> std::result::Result<Self, Self::Error> {
    let inline_const = match value.inline_const {
      Some(Either::A(bool_val)) => Some(InlineConstOption::Bool(bool_val)),
      Some(Either::B(config_val)) => {
        let mode = if let Some(mode_str) = config_val.mode.as_ref() {
          match mode_str.as_str() {
            "all" => Some(InlineConstMode::All),
            "smart" => Some(InlineConstMode::Smart),
            _ => {
              return Err(BuildDiagnostic::napi_error(napi::Error::from_reason(
                "Invalid value for inline_const.mode: expected 'all' or 'smart'".to_string(),
              )));
            }
          }
        } else {
          None
        };

        Some(InlineConstOption::Config(InlineConstConfig {
          mode,
          pass: config_val.pass.unwrap_or(1),
        }))
      }
      None => None,
    };

    Ok(Self { inline_const, pife_for_module_wrappers: value.pife_for_module_wrappers })
  }
}
