pub mod build_end;
pub mod build_start;
pub mod hook_load_call_end;
pub mod hook_load_call_start;
pub mod hook_resolve_id_call_end;
pub mod hook_resolve_id_call_start;
pub mod hook_transform_call_end;
pub mod hook_transform_call_start;
pub mod module_graph_ready;

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
#[serde(untagged)]
pub enum Meta {
  HookTransformCallStart(hook_transform_call_start::HookTransformCallStart),
  HookTransformCallEnd(hook_transform_call_end::HookTransformCallEnd),
  HookLoadCallStart(hook_load_call_start::HookLoadCallStart),
  HookLoadCallEnd(hook_load_call_end::HookLoadCallEnd),
  BuildStart(build_start::BuildStart),
  BuildEnd(build_end::BuildEnd),
  HookResolveIdCallStart(hook_resolve_id_call_start::HookResolveIdCallStart),
  HookResolveIdCallEnd(hook_resolve_id_call_end::HookResolveIdCallEnd),
  ModuleGraphReady(module_graph_ready::ModuleGraphReady),
}
