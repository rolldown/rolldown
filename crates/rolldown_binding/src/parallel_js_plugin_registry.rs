use crate::options::plugin::{BindingPluginOptions, BindingPluginWithIndex};
use dashmap::{DashMap, mapref::entry::Entry};
use napi::{
  Env, Unknown,
  bindgen_prelude::{FromNapiValue, JavaScriptClassExt, JsObjectValue, Object, ObjectFinalize},
};
use napi_derive::napi;
use rolldown_utils::{dashmap::FxDashMap, rustc_hash::FxHashMapExt};
use rustc_hash::FxHashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU16, Ordering};

type PluginsInSingleWorker = Vec<BindingPluginWithIndex>;
type PluginsList = Vec<PluginsInSingleWorker>;
type RegistrySlot = Option<PluginsList>;
pub(crate) type PluginValues = FxHashMap<usize, Vec<BindingPluginOptions>>;

static PLUGINS_MAP: LazyLock<FxDashMap<u16, RegistrySlot>> = LazyLock::new(DashMap::default);
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

    let id = reserve_registry_id(&NEXT_ID, &PLUGINS_MAP).ok_or_else(|| {
      napi::Error::from_reason(
        "All parallel JavaScript plugin registry IDs are currently in use".to_string(),
      )
    })?;

    Ok(Self { id, worker_count })
  }

  pub fn take_plugin_values(&self) -> napi::Result<PluginValues> {
    let mut slot = PLUGINS_MAP.get_mut(&self.id).ok_or_else(|| {
      napi::Error::from_reason(
        "Parallel JavaScript plugin registry is no longer active".to_string(),
      )
    })?;
    take_registered_plugin_values(&mut slot, self.worker_count)
  }
}

impl ObjectFinalize for ParallelJsPluginRegistry {
  fn finalize(&mut self, mut _env: Env) -> napi::Result<()> {
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
  if let Some(mut slot) = PLUGINS_MAP.get_mut(&id)
    && let Some(existing_plugins) = slot.as_mut()
  {
    existing_plugins.push(plugins);
  }
}

fn reserve_registry_id(
  next_id: &AtomicU16,
  plugins_map: &FxDashMap<u16, RegistrySlot>,
) -> Option<u16> {
  for _ in 0..=u32::from(u16::MAX) {
    let id = next_id.fetch_add(1, Ordering::Relaxed);
    if id == 0 {
      continue;
    }
    if let Entry::Vacant(entry) = plugins_map.entry(id) {
      entry.insert(Some(vec![]));
      return Some(id);
    }
  }
  None
}

fn take_registered_plugin_values(
  slot: &mut RegistrySlot,
  worker_count: u16,
) -> napi::Result<PluginValues> {
  let plugins_list = slot.as_ref().ok_or_else(|| {
    napi::Error::from_reason("Parallel JavaScript plugin registry was already consumed".to_string())
  })?;
  if plugins_list.len() != usize::from(worker_count) {
    return Err(napi::Error::from_reason(format!(
      "Parallel JavaScript plugin registry expected {worker_count} worker registrations but received {}",
      plugins_list.len()
    )));
  }

  let plugins_list = slot.take().expect("the registry slot was checked above");
  let mut map: FxHashMap<usize, Vec<BindingPluginOptions>> =
    FxHashMap::with_capacity(plugins_list.first().map_or(0, Vec::len));
  for plugins in plugins_list {
    for plugin in plugins {
      map.entry(plugin.index as usize).or_default().push(plugin.plugin);
    }
  }
  Ok(map)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn registry_id_allocation_skips_zero_live_entries_and_consumed_tombstones() {
    let plugins_map = FxDashMap::default();
    plugins_map.insert(u16::MAX, Some(vec![]));
    let next_id = AtomicU16::new(u16::MAX);

    assert_eq!(reserve_registry_id(&next_id, &plugins_map), Some(1));
    assert!(plugins_map.contains_key(&u16::MAX));
    assert!(plugins_map.contains_key(&1));
    assert!(!plugins_map.contains_key(&0));

    *plugins_map.get_mut(&1).unwrap() = None;
    next_id.store(1, Ordering::Relaxed);
    assert_eq!(reserve_registry_id(&next_id, &plugins_map), Some(2));
  }

  #[test]
  fn registry_consumption_rejects_incomplete_and_repeated_consumers_without_panicking() {
    let mut slot = Some(vec![]);
    let error = take_registered_plugin_values(&mut slot, 1).unwrap_err();
    assert!(error.reason.contains("expected 1 worker registrations"));
    assert!(slot.is_some());

    slot.as_mut().unwrap().push(vec![]);
    assert!(take_registered_plugin_values(&mut slot, 1).unwrap().is_empty());
    let error = take_registered_plugin_values(&mut slot, 1).unwrap_err();
    assert!(error.reason.contains("already consumed"));
  }
}
