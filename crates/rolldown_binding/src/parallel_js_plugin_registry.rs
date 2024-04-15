use crate::options::plugin::{BindingPluginOptions, BindingPluginWithIndex};
use dashmap::DashMap;
use napi::bindgen_prelude::External;
use napi_derive::napi;
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use std::{
  hash::BuildHasherDefault,
  sync::{
    atomic::{self, AtomicU16},
    Arc, Mutex, Weak,
  },
};

type PluginsInSingleWorker = Vec<BindingPluginWithIndex>;
type PluginsList = Vec<PluginsInSingleWorker>;
pub(crate) type PluginValues = FxHashMap<usize, Vec<BindingPluginOptions>>;

static REGISTRY_MAP: Lazy<DashMap<u16, Weak<ParallelJsPluginRegistry>>> =
  Lazy::new(DashMap::default);
static NEXT_ID: AtomicU16 = AtomicU16::new(1);

pub struct ParallelJsPluginRegistry {
  pub id: u16,
  pub worker_count: u16,
  plugins: Mutex<Option<PluginsList>>,
}

#[napi]
impl ParallelJsPluginRegistry {
  pub fn take_plugin_values(&self) -> PluginValues {
    let plugins_list = self.plugins.lock().unwrap().take().expect("plugin list already taken");

    let mut map: FxHashMap<usize, Vec<BindingPluginOptions>> =
      FxHashMap::with_capacity_and_hasher(plugins_list[0].len(), BuildHasherDefault::default());
    for plugins in plugins_list {
      for plugin in plugins {
        map.entry(plugin.index as usize).or_default().push(plugin.plugin);
      }
    }

    map
  }
}

impl Drop for ParallelJsPluginRegistry {
  fn drop(&mut self) {
    REGISTRY_MAP.remove(&self.id);
  }
}

#[napi]
pub fn create_parallel_js_plugin_registry(
  worker_count: u16,
) -> napi::Result<External<Arc<ParallelJsPluginRegistry>>> {
  if worker_count == 0 {
    return Err(napi::Error::from_reason("worker count should be bigger than 0"));
  }

  let id = NEXT_ID.fetch_add(1, atomic::Ordering::Relaxed);
  let plugins: PluginsList = vec![];
  let registry =
    Arc::new(ParallelJsPluginRegistry { id, worker_count, plugins: Mutex::new(Some(plugins)) });
  REGISTRY_MAP.insert(id, Arc::downgrade(&registry));

  Ok(External::new(registry))
}

#[napi]
pub fn get_registry_id(registry: &External<Arc<ParallelJsPluginRegistry>>) -> u16 {
  registry.as_ref().id
}

#[napi]
pub fn register_plugins(id: u16, plugins: PluginsInSingleWorker) {
  if let Some(registry) = REGISTRY_MAP.get_mut(&id).and_then(|x| x.upgrade()) {
    if let Some(existing_plugins) = registry.plugins.lock().unwrap().as_mut() {
      existing_plugins.push(plugins);
    }
  }
}
