use std::sync::Arc;

use arcstr::ArcStr;
use rolldown_common::{
  ImportAttributes, JsModuleMeta, ResolvedExternal, ResolvedId, side_effects::HookSideEffects,
};

#[derive(Debug, Default)]
pub struct HookResolveIdOutput {
  pub id: ArcStr,
  pub external: Option<ResolvedExternal>,
  pub normalize_external_id: Option<bool>,
  pub side_effects: Option<HookSideEffects>,
  pub package_json_path: Option<String>,
  /// Replaces the attributes of the import. `None` keeps them, and an empty value removes them.
  /// Ignored without `experimental.importAttributes`.
  pub attributes: Option<ImportAttributes>,
  /// See [`ResolvedId::module_options_receipt`].
  pub module_options_receipt: Option<u32>,
  /// See [`ResolvedId::js_meta`].
  pub js_meta: Option<Arc<JsModuleMeta>>,
}

impl HookResolveIdOutput {
  pub fn from_id(id: impl Into<ArcStr>) -> Self {
    Self { id: id.into(), ..Default::default() }
  }

  pub fn from_resolved_id(resolved_id: ResolvedId) -> Self {
    Self {
      id: resolved_id.id.raw_id().into(),
      attributes: Some(resolved_id.attributes),
      external: Some(resolved_id.external),
      side_effects: resolved_id.side_effects,
      normalize_external_id: None,
      package_json_path: resolved_id
        .package_json
        .map(|p| p.realpath().to_string_lossy().to_string()),
      module_options_receipt: resolved_id.module_options_receipt,
      js_meta: resolved_id.js_meta,
    }
  }
}
