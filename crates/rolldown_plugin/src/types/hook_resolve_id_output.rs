use arcstr::ArcStr;
use rolldown_common::{ResolvedExternal, ResolvedId, side_effects::HookSideEffects};

#[derive(Debug, Default)]
pub struct HookResolveIdOutput {
  pub id: ArcStr,
  pub external: Option<ResolvedExternal>,
  pub normalize_external_id: Option<bool>,
  pub side_effects: Option<HookSideEffects>,
  pub package_json_path: Option<String>,
}

impl HookResolveIdOutput {
  pub fn from_id(id: impl Into<ArcStr>) -> Self {
    Self { id: id.into(), ..Default::default() }
  }

  pub fn from_resolved_id(resolved_id: ResolvedId) -> Self {
    Self {
      id: resolved_id.id,
      external: Some(resolved_id.external),
      side_effects: resolved_id.side_effects,
      normalize_external_id: None,
      package_json_path: resolved_id.package_json.map(|p| p.realpath.to_string_lossy().to_string()),
    }
  }
}
