use crate::options::plugin::{BindingPluginOptions, BindingPluginWithIndex};
use dashmap::DashMap;
use napi::{
  bindgen_prelude::{FromNapiValue, Object, ObjectFinalize},
  Env, JsUnknown,
};
use napi_derive::napi;
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use std::{
  hash::BuildHasherDefault,
  sync::atomic::{self, AtomicU16},
};

type PluginsInSingleWorker = Vec<BindingPluginWithIndex>;
type PluginsList = Vec<PluginsInSingleWorker>;
pub(crate) type PluginValues = FxHashMap<usize, Vec<BindingPluginOptions>>;

static PLUGINS_MAP: Lazy<DashMap<u16, PluginsList>> = Lazy::new(DashMap::default);
static NEXT_ID: AtomicU16 = AtomicU16::new(1);

#[napi(custom_finalize)]
pub struct ParallelJsPluginRegistry {
  #[napi(writable = false)]
  pub id: u16,
  #[napi(writable = false)]
  pub worker_count: u16,
}

#[napi]
impl ParallelJsPluginRegistry {
  #[napi(constructor)]
  pub fn new(worker_count: u16) -> napi::Result<Self> {
    if worker_count == 0 {
      return Err(napi::Error::from_reason("worker count should be bigger than 0"));
    }

    let id = NEXT_ID.fetch_add(1, atomic::Ordering::Relaxed);
    PLUGINS_MAP.insert(id, vec![]);

    Ok(Self { id, worker_count })
  }

  pub fn take_plugin_values(&self) -> PluginValues {
    let plugins_list = PLUGINS_MAP.remove(&self.id).expect("plugin list already taken").1;

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

impl ObjectFinalize for ParallelJsPluginRegistry {
  fn finalize(self, mut _env: Env) -> napi::Result<()> {
    PLUGINS_MAP.remove(&self.id);
    Ok(())
  }
}

impl FromNapiValue for ParallelJsPluginRegistry {
  unsafe fn from_napi_value(
    env: napi::sys::napi_env,
    napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    let unknown = JsUnknown::from_napi_value(env, napi_val)?;
    if !ParallelJsPluginRegistry::instance_of(env.into(), &unknown)? {
      return Err(napi::Error::from_status(napi::Status::GenericFailure));
    }

    let object: Object = unknown.cast();
    let id: u16 = object.get_named_property_unchecked("id")?;
    let worker_count: u16 = object.get_named_property_unchecked("workerCount")?;
    Ok(Self { id, worker_count })
  }
}

#[napi]
pub fn register_plugins(id: u16, plugins: PluginsInSingleWorker) {
  if let Some(mut existing_plugins) = PLUGINS_MAP.get_mut(&id) {
    existing_plugins.push(plugins);
  }
}
