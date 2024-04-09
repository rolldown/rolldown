use crate::options::plugin::{BindingPluginOptions, BindingPluginWithIndex};
use napi::{
  bindgen_prelude::{FromNapiValue, Object, ObjectFinalize},
  Env, JsUnknown,
};
use napi_derive::napi;
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use std::{
  hash::BuildHasherDefault,
  sync::{
    atomic::{self, AtomicU16},
    Mutex,
  },
};

type PluginsInSingleWorker = Vec<BindingPluginWithIndex>;
type PluginsList = Vec<PluginsInSingleWorker>;
pub(crate) type PluginValues = FxHashMap<usize, Vec<BindingPluginOptions>>;

static PLUGINS_MAP: Lazy<Mutex<FxHashMap<u16, PluginsList>>> = Lazy::new(Mutex::default);
static NEXT_ID: AtomicU16 = AtomicU16::new(1);

#[napi(custom_finalize)]
pub struct ThreadSafePluginRegistry {
  #[napi(writable = false)]
  pub id: u16,
  #[napi(writable = false)]
  pub worker_count: u16,
}

#[napi]
impl ThreadSafePluginRegistry {
  #[napi(constructor)]
  pub fn new(worker_count: u16) -> napi::Result<Self> {
    if worker_count == 0 {
      return Err(napi::Error::from_reason("worker count should be bigger than 0"));
    }

    let id = NEXT_ID.fetch_add(1, atomic::Ordering::Relaxed);

    let mut map = PLUGINS_MAP.lock().unwrap();
    map.insert(id, vec![]);

    Ok(Self { id, worker_count })
  }

  pub fn take_plugin_values(&self) -> PluginValues {
    let mut map = PLUGINS_MAP.lock().unwrap();
    let plugins_list = map.remove(&self.id).expect("plugin list already taken");

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

impl ObjectFinalize for ThreadSafePluginRegistry {
  fn finalize(self, mut _env: Env) -> napi::Result<()> {
    let mut map = PLUGINS_MAP.lock().unwrap();
    map.remove(&self.id);

    Ok(())
  }
}

impl FromNapiValue for ThreadSafePluginRegistry {
  unsafe fn from_napi_value(
    env: napi::sys::napi_env,
    napi_val: napi::sys::napi_value,
  ) -> napi::Result<Self> {
    let unknown = JsUnknown::from_napi_value(env, napi_val)?;
    if !ThreadSafePluginRegistry::instance_of(env.into(), &unknown)? {
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
  let mut map = PLUGINS_MAP.lock().unwrap();
  if let Some(existing_plugins) = map.get_mut(&id) {
    existing_plugins.push(plugins);
  }
}
