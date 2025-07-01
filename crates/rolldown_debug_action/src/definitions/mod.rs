pub mod assets_ready;
pub mod build_end;
pub mod build_start;
pub mod chunk_graph_ready;
pub mod hook_load_call_end;
pub mod hook_load_call_start;
pub mod hook_render_chunk_end;
pub mod hook_render_chunk_start;
pub mod hook_resolve_id_call_end;
pub mod hook_resolve_id_call_start;
pub mod hook_transform_call_end;
pub mod hook_transform_call_start;
pub mod module_graph_ready;
pub mod session_meta;

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
  SessionMeta(session_meta::SessionMeta),
  ChunksInfos(chunk_graph_ready::ChunkGraphReady),
  HookRenderChunkStart(hook_render_chunk_start::HookRenderChunkStart),
  HookRenderChunkEnd(hook_render_chunk_end::HookRenderChunkEnd),
  AssetsReady(assets_ready::AssetsReady),
}
