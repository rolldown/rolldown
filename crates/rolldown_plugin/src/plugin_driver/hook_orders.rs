use rolldown_common::PluginIdx;

use crate::{HookUsage, PluginHookMeta, PluginOrder, Pluginable, type_aliases::IndexPluginable};

macro_rules! define_hook_orders {
  ($($field:ident: $usage:ident => $meta:ident,)*) => {
    #[derive(Clone)]
    pub struct PluginHookOrders {
      $(pub $field: Vec<PluginIdx>,)*
    }

    impl PluginHookOrders {
      pub fn new(index_plugins: &IndexPluginable) -> Self {
        Self {
          $($field: Self::sort_plugins_by_hook_meta(
            index_plugins,
            HookUsage::$usage,
            Pluginable::$meta,
          ),)*
        }
      }
    }
  };
}

define_hook_orders! {
  order_by_build_start_meta: BuildStart => call_build_start_meta,
  order_by_resolve_id_meta: ResolveId => call_resolve_id_meta,
  order_by_resolve_dynamic_import_meta: ResolveDynamicImport => call_resolve_dynamic_import_meta,
  order_by_load_meta: Load => call_load_meta,
  order_by_transform_meta: Transform => call_transform_meta,
  order_by_module_parsed_meta: ModuleParsed => call_module_parsed_meta,
  order_by_build_end_meta: BuildEnd => call_build_end_meta,
  order_by_render_start_meta: RenderStart => call_render_start_meta,
  order_by_banner_meta: Banner => call_banner_meta,
  order_by_footer_meta: Footer => call_footer_meta,
  order_by_intro_meta: Intro => call_intro_meta,
  order_by_outro_meta: Outro => call_outro_meta,
  order_by_render_chunk_meta: RenderChunk => call_render_chunk_meta,
  order_by_augment_chunk_hash_meta: AugmentChunkHash => call_augment_chunk_hash_meta,
  order_by_resolve_file_url_meta: ResolveFileUrl => call_resolve_file_url_meta,
  order_by_render_error_meta: RenderError => call_render_error_meta,
  order_by_generate_bundle_meta: GenerateBundle => call_generate_bundle_meta,
  order_by_write_bundle_meta: WriteBundle => call_write_bundle_meta,
  order_by_close_bundle_meta: CloseBundle => call_close_bundle_meta,
  order_by_watch_change_meta: WatchChange => call_watch_change_meta,
  order_by_hot_update_meta: HotUpdate => call_hot_update_meta,
  order_by_close_watcher_meta: CloseWatcher => call_close_watcher_meta,
  order_by_transform_ast_meta: TransformAst => call_transform_ast_meta,
}

impl PluginHookOrders {
  #[inline(never)]
  fn sort_plugins_by_hook_meta(
    index_plugins: &IndexPluginable,
    hook_usage: HookUsage,
    get_hook_meta: fn(&Pluginable) -> Option<PluginHookMeta>,
  ) -> Vec<PluginIdx> {
    let mut pre_plugins = Vec::new();
    let mut post_plugins = Vec::new();
    let mut pin_post_plugins = Vec::new();
    let mut normal_plugins = Vec::with_capacity(index_plugins.len());
    for (idx, plugin) in index_plugins.iter_enumerated() {
      if !plugin.call_hook_usage().contains(hook_usage) {
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
