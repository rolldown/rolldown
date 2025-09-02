use std::{
  any::{Any, TypeId},
  sync::Arc,
};

use rolldown_utils::dashmap::FxDashMap;

#[derive(Debug, Default)]
pub struct PluginContextMeta {
  inner: FxDashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl PluginContextMeta {
  pub fn insert<T: Any + Send + Sync>(&self, value: Arc<T>) {
    self.inner.insert(TypeId::of::<T>(), value);
  }

  pub fn get<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
    self.inner.get(&TypeId::of::<T>()).and_then(|v| v.clone().downcast::<T>().ok())
  }

  pub fn get_or_insert_default<T: Any + Send + Sync + Default>(&self) -> Arc<T> {
    self
      .inner
      .entry(TypeId::of::<T>())
      .or_insert_with(|| Arc::new(T::default()))
      .clone()
      .downcast::<T>()
      .expect("PluginContextMeta: type mismatch for inserted value")
  }
}
