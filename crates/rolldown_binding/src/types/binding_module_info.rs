use std::sync::Arc;

use napi::Either;
use napi_derive::napi;
use rolldown_common::{ExportsKind, side_effects::DeterminedSideEffects};

use crate::options::plugin::types::binding_shared_string::BindingSharedString;

#[napi]
pub struct BindingModuleInfo {
  inner: Arc<rolldown_common::ModuleInfo>,
  #[napi(ts_type = "string")]
  pub id: BindingSharedString,
  #[napi(ts_type = "Array<string>")]
  pub importers: Vec<BindingSharedString>,
  #[napi(ts_type = "Array<string>")]
  pub dynamic_importers: Vec<BindingSharedString>,
  #[napi(ts_type = "Array<string>")]
  pub imported_ids: Vec<BindingSharedString>,
  #[napi(ts_type = "Array<string>")]
  pub dynamically_imported_ids: Vec<BindingSharedString>,
  pub exports: Vec<String>,
  pub is_entry: bool,
  #[napi(ts_type = "'es' | 'cjs' | 'unknown'")]
  pub input_format: String,
}

#[napi]
impl BindingModuleInfo {
  pub fn new(inner: Arc<rolldown_common::ModuleInfo>) -> Self {
    let input_format = match inner.input_format {
      ExportsKind::Esm => "es",
      ExportsKind::CommonJs => "cjs",
      ExportsKind::None => "unknown",
    };
    Self {
      id: BindingSharedString::from(inner.id.as_arc_str().clone()),
      importers: inner
        .importers
        .iter()
        .map(|id| BindingSharedString::from(id.as_arc_str().clone()))
        .collect(),
      dynamic_importers: inner
        .dynamic_importers
        .iter()
        .map(|id| BindingSharedString::from(id.as_arc_str().clone()))
        .collect(),
      imported_ids: inner
        .imported_ids
        .iter()
        .map(|id| BindingSharedString::from(id.as_arc_str().clone()))
        .collect(),
      dynamically_imported_ids: inner
        .dynamically_imported_ids
        .iter()
        .map(|id| BindingSharedString::from(id.as_arc_str().clone()))
        .collect(),
      is_entry: inner.is_entry,
      exports: inner.exports.iter().map(ToString::to_string).collect(),
      input_format: input_format.to_string(),
      inner,
    }
  }

  #[napi(getter)]
  pub fn code(&self) -> Option<&str> {
    self.inner.code.as_deref()
  }
}

pub fn to_binding_module_side_effects(
  side_effects: DeterminedSideEffects,
) -> Either<bool, &'static str> {
  match side_effects {
    DeterminedSideEffects::UserDefined(value) | DeterminedSideEffects::Analyzed(value) => {
      Either::A(value)
    }
    DeterminedSideEffects::NoTreeshake => Either::B("no-treeshake"),
  }
}

/// The JS `treeshake.moduleSideEffects` function, which the getter and setter of
/// `ModuleInfo.moduleSideEffects` pass when the option is a function.
pub type BindingModuleSideEffectsOptionFn<'a> =
  napi::bindgen_prelude::Function<'a, napi::bindgen_prelude::FnArgs<(String, bool)>, Option<bool>>;

/// Calls `call_option`, or gives no result when the option is not a function.
pub fn call_module_side_effects_option(
  call_option: Option<&BindingModuleSideEffectsOptionFn<'_>>,
  raw_id: &str,
  external: bool,
) -> anyhow::Result<Option<bool>> {
  match call_option {
    Some(call_option) => Ok(call_option.call((raw_id.to_string(), external).into())?),
    None => Ok(None),
  }
}
