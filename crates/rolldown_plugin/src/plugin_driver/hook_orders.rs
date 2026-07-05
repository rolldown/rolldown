use oxc_index::IndexVec;
use rolldown_common::PluginIdx;

use crate::{
  __inner::SharedPluginable, HookUsage, PluginHookMeta, PluginOrder, type_aliases::IndexPluginable,
};

#[expect(clippy::struct_field_names)] // Allow all fields to have the same prefix `order_by_`
#[derive(Clone)]
pub struct PluginHookOrders {
  pub order_by_build_start_meta: Vec<PluginIdx>,
  pub order_by_resolve_id_meta: Vec<PluginIdx>,
  pub order_by_resolve_dynamic_import_meta: Vec<PluginIdx>,
  pub order_by_load_meta: Vec<PluginIdx>,
  pub order_by_transform_meta: Vec<PluginIdx>,
  pub order_by_module_parsed_meta: Vec<PluginIdx>,
  pub order_by_build_end_meta: Vec<PluginIdx>,
  pub order_by_render_start_meta: Vec<PluginIdx>,
  pub order_by_banner_meta: Vec<PluginIdx>,
  pub order_by_footer_meta: Vec<PluginIdx>,
  pub order_by_intro_meta: Vec<PluginIdx>,
  pub order_by_outro_meta: Vec<PluginIdx>,
  pub order_by_render_chunk_meta: Vec<PluginIdx>,
  pub order_by_augment_chunk_hash_meta: Vec<PluginIdx>,
  pub order_by_render_error_meta: Vec<PluginIdx>,
  pub order_by_generate_bundle_meta: Vec<PluginIdx>,
  pub order_by_write_bundle_meta: Vec<PluginIdx>,
  pub order_by_close_bundle_meta: Vec<PluginIdx>,
  pub order_by_watch_change_meta: Vec<PluginIdx>,
  pub order_by_close_watcher_meta: Vec<PluginIdx>,
  pub order_by_transform_ast_meta: Vec<PluginIdx>,
}

impl PluginHookOrders {
  pub fn new(
    index_plugins: &IndexPluginable,
    plugin_usage_vec: &IndexVec<PluginIdx, HookUsage>,
  ) -> Self {
    Self {
      order_by_build_start_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::BuildStart,
        |p| p.call_build_start_meta(),
      ),
      order_by_resolve_id_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::ResolveId,
        |p| p.call_resolve_id_meta(),
      ),
      order_by_resolve_dynamic_import_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::ResolveDynamicImport,
        |p| p.call_resolve_dynamic_import_meta(),
      ),
      order_by_load_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::Load,
        |p| p.call_load_meta(),
      ),
      order_by_transform_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::Transform,
        |p| p.call_transform_meta(),
      ),
      order_by_module_parsed_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::ModuleParsed,
        |p| p.call_module_parsed_meta(),
      ),
      order_by_build_end_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::BuildEnd,
        |p| p.call_build_end_meta(),
      ),
      order_by_render_start_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::RenderStart,
        |p| p.call_render_start_meta(),
      ),
      order_by_banner_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::Banner,
        |p| p.call_banner_meta(),
      ),
      order_by_footer_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::Footer,
        |p| p.call_footer_meta(),
      ),
      order_by_intro_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::Intro,
        |p| p.call_intro_meta(),
      ),
      order_by_outro_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::Outro,
        |p| p.call_outro_meta(),
      ),
      order_by_render_chunk_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::RenderChunk,
        |p| p.call_render_chunk_meta(),
      ),
      order_by_augment_chunk_hash_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::AugmentChunkHash,
        |p| p.call_augment_chunk_hash_meta(),
      ),
      order_by_render_error_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::RenderError,
        |p| p.call_render_error_meta(),
      ),
      order_by_generate_bundle_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::GenerateBundle,
        |p| p.call_generate_bundle_meta(),
      ),
      order_by_write_bundle_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::WriteBundle,
        |p| p.call_write_bundle_meta(),
      ),
      order_by_close_bundle_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::CloseBundle,
        |p| p.call_close_bundle_meta(),
      ),
      order_by_watch_change_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::WatchChange,
        |p| p.call_watch_change_meta(),
      ),
      order_by_close_watcher_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::CloseWatcher,
        |p| p.call_close_watcher_meta(),
      ),
      order_by_transform_ast_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        plugin_usage_vec,
        HookUsage::TransformAst,
        |p| p.call_transform_ast_meta(),
      ),
    }
  }

  #[inline(never)]
  fn sort_plugins_by_hook_meta(
    index_plugins: &IndexPluginable,
    plugin_usage_vec: &IndexVec<PluginIdx, HookUsage>,
    hook_usage: HookUsage,
    get_hook_meta: fn(&SharedPluginable) -> Option<PluginHookMeta>,
  ) -> Vec<PluginIdx> {
    let mut pre_plugins = Vec::new();
    let mut post_plugins = Vec::new();
    let mut pin_post_plugins = Vec::new();
    let mut normal_plugins = Vec::with_capacity(index_plugins.len());
    for (idx, plugin) in index_plugins.iter_enumerated() {
      if !plugin_usage_vec[idx].contains(hook_usage) {
        continue;
      }
      let meta = get_hook_meta(plugin);
      match meta {
        None => normal_plugins.push(idx),
        Some(meta) => match meta.order {
          Some(PluginOrder::Pre) => pre_plugins.push(idx),
          Some(PluginOrder::Post) => post_plugins.push(idx),
          Some(PluginOrder::PinPost) => pin_post_plugins.push(idx),
          None => normal_plugins.push(idx),
        },
      }
    }
    // Reverse so first-seen plugin runs last (pinned to the very end)
    pin_post_plugins.reverse();
    [pre_plugins, normal_plugins, post_plugins, pin_post_plugins].concat()
  }
}
