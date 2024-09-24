use std::{
  ops::Deref,
  sync::{Arc, OnceLock, Weak},
  vec,
};

use rolldown_common::{ModuleTable, SharedFileEmitter, SharedNormalizedBundlerOptions};
use rolldown_resolver::Resolver;

use crate::{
  __inner::SharedPluginable,
  plugin_context::PluginContextImpl,
  type_aliases::{IndexPluginContext, IndexPluginFilter, IndexPluginable},
  types::{hook_filter::HookFilterOptions, plugin_idx::PluginIdx},
  PluginContext, PluginHookMeta, PluginOrder,
};

mod build_hooks;
mod hook_filter;
mod output_hooks;

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: IndexPluginable,
  contexts: IndexPluginContext,
  order_indicates: HookOrderIndicates,
  index_plugin_filters: IndexPluginFilter,
}

impl PluginDriver {
  pub fn new_shared(
    plugins: Vec<SharedPluginable>,
    resolver: &Arc<Resolver>,
    file_emitter: &SharedFileEmitter,
    options: &SharedNormalizedBundlerOptions,
  ) -> SharedPluginDriver {
    Arc::new_cyclic(|plugin_driver| {
      let mut index_plugins = IndexPluginable::with_capacity(plugins.len());
      let mut index_contexts = IndexPluginContext::with_capacity(plugins.len());
      let mut index_plugin_filters = IndexPluginFilter::with_capacity(plugins.len());

      plugins.into_iter().for_each(|plugin| {
        let plugin_idx = index_plugins.push(Arc::clone(&plugin));
        // TODO: Error handling
        index_plugin_filters.push(HookFilterOptions {
          load: plugin.call_load_filter().unwrap(),
          resolve_id: plugin.call_resolve_id_filter().unwrap(),
          transform: plugin.call_transform_filter().unwrap(),
        });
        index_contexts.push(
          PluginContextImpl {
            skipped_resolve_calls: vec![],
            plugin_idx,
            plugin_driver: Weak::clone(plugin_driver),
            resolver: Arc::clone(resolver),
            file_emitter: Arc::clone(file_emitter),
            module_table: OnceLock::default(),
            options: Arc::clone(options),
          }
          .into(),
        );
      });

      Self {
        order_indicates: HookOrderIndicates::new(&index_plugins),
        plugins: index_plugins,
        contexts: index_contexts,
        index_plugin_filters,
      }
    })
  }

  pub fn set_module_table(&self, module_table: &'static ModuleTable) {
    self.contexts.iter().for_each(|ctx| {
      ctx.module_table.set(module_table).expect("module_table is already set before");
    });
  }

  pub fn iter_plugin_with_context_by_order<'me>(
    &'me self,
    ordered_plugins: &'me [PluginIdx],
  ) -> impl Iterator<Item = (PluginIdx, &SharedPluginable, &PluginContext)> + 'me {
    ordered_plugins.iter().copied().map(move |idx| {
      let plugin = &self.plugins[idx];
      let context = &self.contexts[idx];
      (idx, plugin, context)
    })
  }
}

impl Deref for PluginDriver {
  type Target = HookOrderIndicates;
  fn deref(&self) -> &Self::Target {
    &self.order_indicates
  }
}

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
