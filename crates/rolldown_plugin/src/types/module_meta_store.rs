use std::sync::{
  Arc,
  atomic::{AtomicU64, Ordering},
};

use arcstr::ArcStr;
use rolldown_utils::dashmap::FxDashMap;

pub type SharedModuleMetaStore = Arc<ModuleMetaStore>;

#[derive(Debug)]
struct MetaEntry {
  key: ArcStr,
  value: Arc<[u8]>,
  /// The build that set the value last.
  build: u64,
}

/// The meta that Rust plugins keep for each module: bytes under a key, for a module.
///
/// It is separate from the `meta` of JavaScript plugins. JavaScript never reads it, and
/// `ModuleInfo` does not hold it.
///
/// The store lives as long as the bundler and is never cleared, like `moduleOptionMap` on the
/// JavaScript side. So a value can come from an earlier build. Each value keeps the number of
/// the build that set it last, which tells the modules of one build from older ones.
///
/// See internal-docs/persistent-cache/implementation.md.
#[derive(Debug, Default)]
pub struct ModuleMetaStore {
  last_build: AtomicU64,
  modules: FxDashMap<ArcStr, Vec<MetaEntry>>,
}

impl ModuleMetaStore {
  /// The number of a new build. Each plugin driver is one build.
  pub fn start_build(&self) -> u64 {
    self.last_build.fetch_add(1, Ordering::Relaxed) + 1
  }

  /// Replaces the value under `key` of the module. The other keys of the module stay.
  pub fn set(&self, module_id: &str, key: &str, value: Arc<[u8]>, build: u64) {
    // A module that already has meta needs no new copy of its id.
    let mut entries = match self.modules.get_mut(module_id) {
      Some(entries) => entries,
      None => self.modules.entry(module_id.into()).or_default(),
    };
    match entries.iter_mut().find(|entry| entry.key == key) {
      Some(entry) => {
        entry.value = value;
        entry.build = build;
      }
      None => entries.push(MetaEntry { key: key.into(), value, build }),
    }
  }

  /// The value under `key` of the module, from this build or from an earlier one.
  pub fn get(&self, module_id: &str, key: &str) -> Option<Arc<[u8]>> {
    let entries = self.modules.get(module_id)?;
    entries.iter().find(|entry| entry.key == key).map(|entry| Arc::clone(&entry.value))
  }

  /// Whether the value under `key` of the module was set in the build `build`.
  pub fn has_key_in_build(&self, module_id: &str, key: &str, build: u64) -> bool {
    self.modules.get(module_id).is_some_and(|entries| Self::has_key(&entries, key, build))
  }

  /// The modules whose value under `key` was set in the build `build`, in no fixed order.
  pub fn module_ids_with_key(&self, key: &str, build: u64) -> Vec<ArcStr> {
    self
      .modules
      .iter()
      .filter(|entries| Self::has_key(entries.value(), key, build))
      .map(|entries| entries.key().clone())
      .collect()
  }

  /// The length of `module_ids_with_key`, without a copy of the ids.
  pub fn count_modules_with_key(&self, key: &str, build: u64) -> usize {
    self.modules.iter().filter(|entries| Self::has_key(entries.value(), key, build)).count()
  }

  fn has_key(entries: &[MetaEntry], key: &str, build: u64) -> bool {
    entries.iter().any(|entry| entry.build == build && entry.key == key)
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::ModuleMetaStore;

  fn bytes(value: &[u8]) -> Arc<[u8]> {
    Arc::from(value)
  }

  #[test]
  fn a_new_value_replaces_the_value_of_the_same_key_only() {
    let store = ModuleMetaStore::default();
    let build = store.start_build();
    store.set("/a.js", "one", bytes(b"1"), build);
    store.set("/a.js", "two", bytes(b"2"), build);
    store.set("/b.js", "one", bytes(b"b"), build);
    store.set("/a.js", "one", bytes(b"new"), build);

    assert_eq!(store.get("/a.js", "one").as_deref(), Some(b"new".as_slice()));
    assert_eq!(store.get("/a.js", "two").as_deref(), Some(b"2".as_slice()));
    assert_eq!(store.get("/b.js", "one").as_deref(), Some(b"b".as_slice()));
    assert_eq!(store.get("/a.js", "three"), None);
    assert_eq!(store.get("/c.js", "one"), None);
  }

  #[test]
  fn lists_only_the_modules_that_the_build_set() {
    let store = ModuleMetaStore::default();
    let first = store.start_build();
    store.set("/a.js", "mark", bytes(b"1"), first);
    store.set("/gone.js", "mark", bytes(b"1"), first);
    store.set("/a.js", "other", bytes(b"1"), first);
    assert_eq!(store.count_modules_with_key("mark", first), 2);

    let second = store.start_build();
    store.set("/a.js", "mark", bytes(b"1"), second);
    store.set("/gone.js", "other", bytes(b"1"), second);

    assert_eq!(store.count_modules_with_key("mark", second), 1);
    assert_eq!(store.module_ids_with_key("mark", second), ["/a.js"]);
    assert_eq!(store.count_modules_with_key("missing", second), 0);
    // A read gives the value of an earlier build too.
    assert_eq!(store.get("/gone.js", "mark").as_deref(), Some(b"1".as_slice()));
  }

  #[test]
  fn tells_a_value_of_this_build_from_a_value_of_an_earlier_build() {
    let store = ModuleMetaStore::default();
    let first = store.start_build();
    store.set("/a.js", "mark", bytes(b"1"), first);
    store.set("/old.js", "mark", bytes(b"1"), first);
    assert!(store.has_key_in_build("/old.js", "mark", first));

    let second = store.start_build();
    store.set("/a.js", "mark", bytes(b"1"), second);
    store.set("/old.js", "other", bytes(b"1"), second);

    assert!(store.has_key_in_build("/a.js", "mark", second));
    assert!(!store.has_key_in_build("/old.js", "mark", second));
    assert!(store.has_key_in_build("/old.js", "other", second));
    assert!(!store.has_key_in_build("/a.js", "other", second));
    assert!(!store.has_key_in_build("/missing.js", "mark", second));
    // `get` still gives the value of the earlier build.
    assert!(store.get("/old.js", "mark").is_some());
  }
}
