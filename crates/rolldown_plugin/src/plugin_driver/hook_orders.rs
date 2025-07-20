use crate::{
  __inner::SharedPluginable, PluginHookMeta, PluginOrder, type_aliases::IndexPluginable,
  types::plugin_idx::PluginIdx,
};

#[allow(clippy::struct_field_names)] // Allow all fields to have the same prefix `order_by_`
#[derive(Clone)]
pub struct HookOrderIndicates {
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

impl HookOrderIndicates {
  pub fn new(index_plugins: &IndexPluginable) -> Self {
    Self {
      order_by_build_start_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_build_start_meta()
      }),
      order_by_resolve_id_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_resolve_id_meta()
      }),
      order_by_resolve_dynamic_import_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_resolve_dynamic_import_meta()
      }),
      order_by_load_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| p.call_load_meta()),
      order_by_transform_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_transform_meta()
      }),
      order_by_module_parsed_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_module_parsed_meta()
      }),
      order_by_build_end_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_build_end_meta()
      }),
      order_by_render_start_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_render_start_meta()
      }),
      order_by_banner_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_banner_meta()
      }),
      order_by_footer_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_footer_meta()
      }),
      order_by_intro_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| p.call_intro_meta()),
      order_by_outro_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| p.call_outro_meta()),
      order_by_render_chunk_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_render_chunk_meta()
      }),
      order_by_augment_chunk_hash_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_augment_chunk_hash_meta()
      }),
      order_by_render_error_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_render_error_meta()
      }),
      order_by_generate_bundle_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_generate_bundle_meta()
      }),
      order_by_write_bundle_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_write_bundle_meta()
      }),
      order_by_close_bundle_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_close_bundle_meta()
      }),
      order_by_watch_change_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_watch_change_meta()
      }),
      order_by_close_watcher_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_close_watcher_meta()
      }),
      order_by_transform_ast_meta: Self::sort_plugins_by_hook_meta(index_plugins, |p| {
        p.call_transform_ast_meta()
      }),
    }
  }

  fn sort_plugins_by_hook_meta(
    index_plugins: &IndexPluginable,
    get_hook_meta: impl Fn(&SharedPluginable) -> Option<PluginHookMeta>,
  ) -> Vec<PluginIdx> {
    let mut pre_plugins = vec![];
    let mut normal_plugins = vec![];
    let mut post_plugins = vec![];

    for (idx, plugin) in index_plugins.iter_enumerated() {
      let meta = get_hook_meta(plugin);
      if let Some(meta) = meta {
        match meta.order {
          Some(PluginOrder::Pre) => pre_plugins.push(idx),
          Some(PluginOrder::Post) => post_plugins.push(idx),
          None => normal_plugins.push(idx),
        }
      } else {
        normal_plugins.push(idx);
      }
    }
    pre_plugins.extend(normal_plugins);
    pre_plugins.extend(post_plugins);
    pre_plugins
  }
}
