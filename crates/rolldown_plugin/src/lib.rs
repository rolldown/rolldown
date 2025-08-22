mod generated;
mod plugin;
mod plugin_context;
mod plugin_driver;
mod pluginable;
mod type_aliases;
mod types;
mod utils;

pub use rolldown_common::{Log, LogWithoutPlugin};
pub use typedmap;

/// Only for usage by the rolldown's crate. Do not use this directly.
#[doc(hidden)]
pub mod __inner {
  pub use super::utils::resolve_id_check_external::resolve_id_check_external;
  pub use super::utils::resolve_id_with_plugins::resolve_id_with_plugins;
  pub use crate::pluginable::{BoxPluginable, Pluginable, SharedPluginable};
}

pub use crate::{
  generated::hook_usage::HookUsage,
  plugin::{
    HookAugmentChunkHashReturn, HookInjectionOutputReturn, HookLoadReturn, HookNoopReturn,
    HookRenderChunkReturn, HookResolveIdReturn, HookTransformAstReturn, HookTransformReturn,
    Plugin,
  },
  plugin_context::{
    PluginContext, SharedNativePluginContext, SharedTransformPluginContext, TransformPluginContext,
  },
  plugin_driver::{PluginDriver, SharedPluginDriver},
  pluginable::Pluginable,
  types::custom_field::CustomField,
  types::hook_addon_args::HookAddonArgs,
  types::hook_build_end_args::HookBuildEndArgs,
  types::hook_build_start_args::HookBuildStartArgs,
  types::hook_close_bundle_args::HookCloseBundleArgs,
  types::hook_generate_bundle_args::HookGenerateBundleArgs,
  types::hook_load_args::HookLoadArgs,
  types::hook_load_output::HookLoadOutput,
  types::hook_render_chunk_args::HookRenderChunkArgs,
  types::hook_render_chunk_output::HookRenderChunkOutput,
  types::hook_render_error::HookRenderErrorArgs,
  types::hook_render_start_args::HookRenderStartArgs,
  types::hook_resolve_id_args::HookResolveIdArgs,
  types::hook_resolve_id_output::HookResolveIdOutput,
  types::hook_transform_args::HookTransformArgs,
  types::hook_transform_ast_args::HookTransformAstArgs,
  types::hook_transform_output::HookTransformOutput,
  types::hook_write_bundle_args::HookWriteBundleArgs,
  types::plugin_context_resolve_options::PluginContextResolveOptions,
  types::plugin_hook_meta::{PluginHookMeta, PluginOrder},
};
