pub mod build_end;
pub mod build_start;
pub mod hook_load_call_end;
pub mod hook_load_call_start;
pub mod hook_transform_call_end;
pub mod hook_transform_call_start;

#[derive(valuable::Valuable, ts_rs::TS, serde::Serialize)]
#[ts(export)]
#[serde(untagged)]
pub enum Action {
  HookTransformCallStart(hook_transform_call_start::HookTransformCallStart),
  HookTransformCallEnd(hook_transform_call_end::HookTransformCallEnd),
  HookLoadCallStart(hook_load_call_start::HookLoadCallStart),
  HookLoadCallEnd(hook_load_call_end::HookLoadCallEnd),
  BuildStart(build_start::BuildStart),
  BuildEnd(build_end::BuildEnd),
}
