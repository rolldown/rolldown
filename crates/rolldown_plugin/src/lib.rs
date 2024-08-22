mod plugin;
mod plugin_context;
mod plugin_driver;
mod plugin_hook_meta;
mod pluginable;
mod transform_plugin_context;
mod type_aliases;
mod types;
mod utils;

/// Only for usage by the rolldown's crate. Do not use this directly.
#[cfg(feature = "inner")]
pub mod __inner {
  pub use super::utils::resolve_id_with_plugins::resolve_id_with_plugins;
  pub use crate::pluginable::{BoxPluginable, Pluginable, SharedPluginable};
}

pub use crate::{
  plugin::{
    HookAugmentChunkHashReturn, HookInjectionOutputReturn, HookLoadReturn, HookNoopReturn,
    HookRenderChunkReturn, HookResolveIdReturn, HookTransformAstReturn, HookTransformReturn,
    Plugin,
  },
  plugin_context::PluginContext,
  plugin_driver::{PluginDriver, SharedPluginDriver},
  plugin_hook_meta::{PluginHookMeta, PluginOrder},
  transform_plugin_context::TransformPluginContext,
  types::hook_addon_args::HookAddonArgs,
  types::hook_build_end_args::HookBuildEndArgs,
  types::hook_filter::{
    GeneralHookFilter, LoadHookFilter, ResolvedIdHookFilter, TransformHookFilter,
  },
  types::hook_load_args::HookLoadArgs,
  types::hook_load_output::HookLoadOutput,
  types::hook_render_chunk_args::HookRenderChunkArgs,
  types::hook_render_chunk_output::HookRenderChunkOutput,
  types::hook_render_error::HookRenderErrorArgs,
  types::hook_resolve_id_args::HookResolveIdArgs,
  types::hook_resolve_id_output::HookResolveIdOutput,
  types::hook_transform_args::HookTransformArgs,
  types::hook_transform_ast_args::HookTransformAstArgs,
  types::hook_transform_output::HookTransformOutput,
  types::plugin_context_resolve_options::PluginContextResolveOptions,
};

pub use typedmap;
