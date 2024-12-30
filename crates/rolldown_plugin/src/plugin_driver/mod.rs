use std::{
  ops::Deref,
  sync::{Arc, Weak},
  vec,
};

use arcstr::ArcStr;
use dashmap::{DashMap, DashSet};
use rolldown_common::{
  ModuleId, ModuleInfo, ModuleLoaderMsg, SharedFileEmitter, SharedNormalizedBundlerOptions,
};
use rolldown_resolver::Resolver;
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use tokio::sync::Mutex;

use crate::{
  __inner::SharedPluginable,
  plugin_context::{LoadCallback, PluginContextImpl},
  type_aliases::{IndexPluginContext, IndexPluginable},
  types::plugin_idx::PluginIdx,
  PluginContext, PluginHookMeta, PluginOrder,
};

mod build_hooks;
mod output_hooks;
mod watch_hooks;

pub type SharedPluginDriver = Arc<PluginDriver>;

pub struct PluginDriver {
  plugins: IndexPluginable,
  contexts: IndexPluginContext,
  order_indicates: HookOrderIndicates,
  file_emitter: SharedFileEmitter,
  pub watch_files: Arc<FxDashSet<ArcStr>>,
  pub modules: Arc<FxDashMap<ArcStr, Arc<ModuleInfo>>>,
  pub context_load_modules: Arc<FxDashMap<ArcStr, LoadCallback>>,
  pub(crate) tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>>>,
}

impl PluginDriver {
  pub fn new_shared(
    plugins: Vec<SharedPluginable>,
    resolver: &Arc<Resolver>,
    file_emitter: &SharedFileEmitter,
    options: &SharedNormalizedBundlerOptions,
  ) -> SharedPluginDriver {
    let watch_files = Arc::new(DashSet::default());
    let modules = Arc::new(DashMap::default());
    let context_load_modules = Arc::new(DashMap::default());
    let tx = Arc::new(Mutex::new(None));

    Arc::new_cyclic(|plugin_driver| {
      let mut index_plugins = IndexPluginable::with_capacity(plugins.len());
      let mut index_contexts = IndexPluginContext::with_capacity(plugins.len());

      plugins.into_iter().for_each(|plugin| {
        let plugin_idx = index_plugins.push(Arc::clone(&plugin));
        index_contexts.push(
          PluginContextImpl {
            skipped_resolve_calls: vec![],
            plugin_idx,
            plugin_driver: Weak::clone(plugin_driver),
            resolver: Arc::clone(resolver),
            file_emitter: Arc::clone(file_emitter),
            modules: Arc::clone(&modules),
            options: Arc::clone(options),
            watch_files: Arc::clone(&watch_files),
            context_load_modules: Arc::clone(&context_load_modules),
            tx: Arc::clone(&tx),
          }
          .into(),
        );
      });

      Self {
        order_indicates: HookOrderIndicates::new(&index_plugins),
        plugins: index_plugins,
        contexts: index_contexts,
        file_emitter: Arc::clone(file_emitter),
        watch_files,
        modules,
        context_load_modules,
        tx,
      }
    })
  }

  pub fn clear(&self) {
    self.watch_files.clear();
    self.modules.clear();
    self.file_emitter.clear();
  }

  pub fn set_module_info(&self, module_id: &ModuleId, module_info: Arc<ModuleInfo>) {
    self.modules.insert(module_id.resource_id().into(), module_info);
  }

  pub async fn set_context_load_modules_tx(
    &self,
    tx: Option<tokio::sync::mpsc::Sender<ModuleLoaderMsg>>,
  ) {
    let mut tx_guard = self.tx.lock().await;
    *tx_guard = tx;
  }

  pub async fn mark_context_load_modules_loaded(&self, module_id: &ModuleId) -> anyhow::Result<()> {
    if let Some((_, callback)) = self.context_load_modules.remove(module_id.resource_id()) {
      callback().await?;
    }
    Ok(())
  }

  pub fn iter_plugin_with_context_by_order<'me>(
    &'me self,
    ordered_plugins: &'me [PluginIdx],
  ) -> impl Iterator<Item = (PluginIdx, &'me SharedPluginable, &'me PluginContext)> + 'me {
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
