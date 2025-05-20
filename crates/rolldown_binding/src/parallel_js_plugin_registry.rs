use crate::options::plugin::{BindingPluginOptions, BindingPluginWithIndex};
use dashmap::DashMap;
use napi::{
  Env, Unknown,
  bindgen_prelude::{FromNapiValue, JavaScriptClassExt, JsObjectValue, Object, ObjectFinalize},
};
use napi_derive::napi;
use rolldown_utils::{dashmap::FxDashMap, rustc_hash::FxHashMapExt};
use rustc_hash::FxHashMap;
use std::sync::LazyLock;
use std::sync::atomic::{self, AtomicU16};

type PluginsInSingleWorker = Vec<BindingPluginWithIndex>;
type PluginsList = Vec<PluginsInSingleWorker>;
pub(crate) type PluginValues = FxHashMap<usize, Vec<BindingPluginOptions>>;

static PLUGINS_MAP: LazyLock<FxDashMap<u16, PluginsList>> = LazyLock::new(DashMap::default);
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
      FxHashMap::with_capacity(plugins_list[0].len());
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
    unsafe {
      let unknown = Unknown::from_napi_value(env, napi_val)?;
      if !ParallelJsPluginRegistry::instance_of(&Env::from_raw(env), &unknown)? {
        return Err(napi::Error::from_status(napi::Status::GenericFailure));
      }

      let object: Object = unknown.cast()?;
      let id: u16 = object.get_named_property_unchecked("id")?;
      let worker_count: u16 = object.get_named_property_unchecked("workerCount")?;
      Ok(Self { id, worker_count })
    }
  }
}

#[napi]
pub fn register_plugins(id: u16, plugins: Vec<BindingPluginWithIndex>) {
  if let Some(mut existing_plugins) = PLUGINS_MAP.get_mut(&id) {
    existing_plugins.push(plugins);
  }
}
