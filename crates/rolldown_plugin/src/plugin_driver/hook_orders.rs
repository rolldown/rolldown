use oxc_index::IndexVec;

use crate::{
  __inner::SharedPluginable, HookUsage, PluginHookMeta, PluginOrder, type_aliases::IndexPluginable,
  types::plugin_idx::PluginIdx,
};

#[allow(clippy::struct_field_names)] // Allow all fields to have the same prefix `order_by_`
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
      order_by_build_start_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::BuildStart).then(|| p.call_build_start_meta())
      }),
      order_by_resolve_id_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::ResolveId).then(|| p.call_resolve_id_meta())
      }),
      order_by_resolve_dynamic_import_meta: Self::sort_plugins_by_hook_meta(
        index_plugins,
        |i, p| {
          plugin_usage_vec[i]
            .contains(HookUsage::ResolveDynamicImport)
            .then(|| p.call_resolve_dynamic_import_meta())
        },
      ),
      order_by_load_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::Load).then(|| p.call_load_meta())
      }),
      order_by_transform_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::Transform).then(|| p.call_transform_meta())
      }),
      order_by_module_parsed_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::ModuleParsed).then(|| p.call_module_parsed_meta())
      }),
      order_by_build_end_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::BuildEnd).then(|| p.call_build_end_meta())
      }),
      order_by_render_start_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::RenderStart).then(|| p.call_render_start_meta())
      }),
      order_by_banner_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::Banner).then(|| p.call_banner_meta())
      }),
      order_by_footer_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::Footer).then(|| p.call_footer_meta())
      }),
      order_by_intro_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::Intro).then(|| p.call_intro_meta())
      }),
      order_by_outro_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::Outro).then(|| p.call_outro_meta())
      }),
      order_by_render_chunk_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::RenderChunk).then(|| p.call_render_chunk_meta())
      }),
      order_by_augment_chunk_hash_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i]
          .contains(HookUsage::AugmentChunkHash)
          .then(|| p.call_augment_chunk_hash_meta())
      }),
      order_by_render_error_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::RenderError).then(|| p.call_render_error_meta())
      }),
      order_by_generate_bundle_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i]
          .contains(HookUsage::GenerateBundle)
          .then(|| p.call_generate_bundle_meta())
      }),
      order_by_write_bundle_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::WriteBundle).then(|| p.call_write_bundle_meta())
      }),
      order_by_close_bundle_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::CloseBundle).then(|| p.call_close_bundle_meta())
      }),
      order_by_watch_change_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::WatchChange).then(|| p.call_watch_change_meta())
      }),
      order_by_close_watcher_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::CloseWatcher).then(|| p.call_close_watcher_meta())
      }),
      order_by_transform_ast_meta: Self::sort_plugins_by_hook_meta(index_plugins, |i, p| {
        plugin_usage_vec[i].contains(HookUsage::TransformAst).then(|| p.call_transform_ast_meta())
      }),
    }
  }

  fn sort_plugins_by_hook_meta(
    index_plugins: &IndexPluginable,
    get_hook_meta: impl Fn(PluginIdx, &SharedPluginable) -> Option<Option<PluginHookMeta>>,
  ) -> Vec<PluginIdx> {
    let mut pre_plugins = Vec::new();
    let mut post_plugins = Vec::new();
    let mut normal_plugins = Vec::with_capacity(index_plugins.len());
    for (idx, plugin) in index_plugins.iter_enumerated() {
      let Some(meta) = get_hook_meta(idx, plugin) else { continue };
      match meta {
        None => normal_plugins.push(idx),
        Some(meta) => match meta.order {
          Some(PluginOrder::Pre) => pre_plugins.push(idx),
          Some(PluginOrder::Post) => post_plugins.push(idx),
          None => normal_plugins.push(idx),
        },
      }
    }
    [pre_plugins, normal_plugins, post_plugins].concat()
  }
}
