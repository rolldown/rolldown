use napi_derive::napi;
use rolldown_common::{JsModuleOption, NewerJsModuleOptions, SavedSideEffects};

use crate::options::plugin::types::binding_hook_side_effects::BindingHookSideEffects;

/// One top-level key of the `meta` of a module.
#[napi(object)]
pub struct BindingModuleMetaEntry {
  pub key: String,
  /// The JSON text of the value. Not set when the value is `undefined`, which has no JSON text.
  pub json: Option<String>,
}

/// The values that hooks set for a module on the JavaScript side, from saved results that the
/// persistent cache reused. The JavaScript side writes them into its `moduleOptionMap`.
#[napi(object, object_from_js = false)]
pub struct BindingSavedModuleOptions {
  /// The JavaScript side passes it as `after` the next time, to get only newer values.
  pub stamp: f64,
  pub meta: Vec<BindingModuleMetaEntry>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<BindingHookSideEffects>,
}

impl From<NewerJsModuleOptions> for BindingSavedModuleOptions {
  fn from(newer: NewerJsModuleOptions) -> Self {
    let mut meta = Vec::with_capacity(newer.options.len());
    let mut module_side_effects = None;
    for option in newer.options {
      match option {
        JsModuleOption::Meta { key, json } => meta.push(BindingModuleMetaEntry { key, json }),
        JsModuleOption::SideEffects(value) => module_side_effects = Some(value.into()),
      }
    }
    Self { stamp: stamp_to_js(newer.stamp), meta, module_side_effects }
  }
}

/// The newest `moduleSideEffects` value of a module from the saved results that the persistent
/// cache reused. The `deferSyncScanData` callback gets all of them, because the JavaScript side
/// may not have taken them yet.
#[napi(object, object_from_js = false)]
pub struct BindingSavedSideEffects {
  /// The key of the module in `moduleOptionMap`.
  pub id: String,
  /// The JavaScript side has the value when the stamp that it applied for the module is not
  /// smaller.
  pub stamp: f64,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub side_effects: BindingHookSideEffects,
}

impl From<SavedSideEffects> for BindingSavedSideEffects {
  fn from(saved: SavedSideEffects) -> Self {
    Self {
      id: saved.module_key.to_string(),
      stamp: stamp_to_js(saved.stamp),
      side_effects: saved.value.into(),
    }
  }
}

#[expect(clippy::cast_precision_loss, reason = "a stamp stays far below 2^53")]
pub fn stamp_to_js(stamp: u64) -> f64 {
  stamp as f64
}

/// `after` comes from JavaScript as a number. It is `0` or a `stamp` that Rust gave out.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn stamp_from_js(after: f64) -> u64 {
  if after.is_finite() && after > 0.0 { after as u64 } else { 0 }
}
