//! Thin wrappers around [`papaya`] concurrent collections that keep the
//! ergonomics rolldown previously relied on from `dashmap`, while removing the
//! `dashmap` dependency (and its private `hashbrown 0.14` copy) from the graph.
//!
//! Unlike `dashmap`, `papaya` is lock-free and all access goes through a
//! *guard* obtained via [`papaya::HashMap::pin`]. References returned by the map
//! borrow that guard, so they cannot outlive it. To preserve the call sites that
//! used to hold a `dashmap::Ref`, the helpers here pin internally and hand back
//! owned/cloned values. When finer-grained control is required, call
//! [`FxDashMap::pin`] / [`FxDashSet::pin`] directly to operate within a single
//! guard scope.

use std::{borrow::Borrow, fmt, hash::Hash};

use papaya::{HashMap, HashSet};
use rustc_hash::FxBuildHasher;

/// Re-exported so downstream crates can drive atomic read-modify-write loops via
/// [`FxDashMap::pin`] without taking a direct dependency on `papaya`.
pub use papaya::{Compute, Operation};

/// Concurrent hash map backed by [`papaya`] using the fast `FxHasher`.
pub struct FxDashMap<K, V>(HashMap<K, V, FxBuildHasher>);

// Manual `Debug` impl: `papaya`'s collections only implement `Debug` under
// `K: Hash + Eq` (+ `V: Debug`), which would leak those bounds onto every
// `#[derive(Debug)]` struct that embeds these maps. Print a placeholder instead.
impl<K, V> fmt::Debug for FxDashMap<K, V> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("FxDashMap").finish_non_exhaustive()
  }
}

impl<K, V> Default for FxDashMap<K, V> {
  fn default() -> Self {
    Self(HashMap::builder().hasher(FxBuildHasher).build())
  }
}

impl<K, V> Clone for FxDashMap<K, V>
where
  K: Clone + Hash + Eq,
  V: Clone,
{
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<K, V> FxDashMap<K, V>
where
  K: Hash + Eq,
{
  /// Pin the map to obtain a guard for performing multiple operations within a
  /// single guard scope. See [`papaya::HashMapRef`] for the available methods.
  #[inline]
  pub fn pin(&self) -> papaya::HashMapRef<'_, K, V, FxBuildHasher, papaya::LocalGuard<'_>> {
    self.0.pin()
  }

  /// Access the underlying [`papaya::HashMap`].
  #[inline]
  pub fn inner(&self) -> &HashMap<K, V, FxBuildHasher> {
    &self.0
  }

  /// Inserts a key-value pair, returning the previous value (cloned) if any.
  #[inline]
  pub fn insert(&self, key: K, value: V) -> Option<V>
  where
    V: Clone,
  {
    self.0.pin().insert(key, value).cloned()
  }

  /// Inserts a key-value pair, ignoring any previous value. Avoids cloning when
  /// the caller does not need the old value.
  #[inline]
  pub fn insert_and_forget(&self, key: K, value: V) {
    self.0.pin().insert(key, value);
  }

  /// Returns a clone of the value for `key`, if present.
  #[inline]
  pub fn get<Q>(&self, key: &Q) -> Option<V>
  where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
    V: Clone,
  {
    self.0.pin().get(key).cloned()
  }

  /// Runs `f` with a shared reference to the value for `key`, if present,
  /// returning its result. The guard is held for the duration of `f`.
  #[inline]
  pub fn with<Q, F, R>(&self, key: &Q, f: F) -> Option<R>
  where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
    F: FnOnce(&V) -> R,
  {
    self.0.pin().get(key).map(f)
  }

  #[inline]
  pub fn contains_key<Q>(&self, key: &Q) -> bool
  where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
  {
    self.0.pin().contains_key(key)
  }

  /// Removes `key`, returning the removed value (cloned) if present.
  #[inline]
  pub fn remove<Q>(&self, key: &Q) -> Option<V>
  where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
    V: Clone,
  {
    self.0.pin().remove(key).cloned()
  }

  /// Atomically returns a clone of the existing value or inserts the default.
  #[inline]
  pub fn get_or_insert_default(&self, key: K) -> V
  where
    V: Clone + Default,
  {
    self.0.pin().get_or_insert_with(key, V::default).clone()
  }

  /// Atomically returns a clone of the existing value or inserts the value
  /// produced by `f`.
  #[inline]
  pub fn get_or_insert_with<F>(&self, key: K, f: F) -> V
  where
    V: Clone,
    F: FnOnce() -> V,
  {
    self.0.pin().get_or_insert_with(key, f).clone()
  }

  /// Returns a cloned snapshot of all key-value pairs. Because `papaya`
  /// references borrow a pin guard, this materializes the entries so they can be
  /// consumed after the guard is released. Iteration order is unspecified.
  #[inline]
  pub fn iter_cloned(&self) -> Vec<(K, V)>
  where
    K: Clone,
    V: Clone,
  {
    self.0.pin().iter().map(|(k, v)| (k.clone(), v.clone())).collect()
  }

  #[inline]
  pub fn len(&self) -> usize {
    self.0.pin().len()
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.0.pin().is_empty()
  }

  #[inline]
  pub fn clear(&self) {
    self.0.pin().clear();
  }
}

/// Concurrent hash set backed by [`papaya`] using the fast `FxHasher`.
pub struct FxDashSet<K>(HashSet<K, FxBuildHasher>);

impl<K> fmt::Debug for FxDashSet<K> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("FxDashSet").finish_non_exhaustive()
  }
}

impl<K> Default for FxDashSet<K> {
  fn default() -> Self {
    Self(HashSet::builder().hasher(FxBuildHasher).build())
  }
}

impl<K> Clone for FxDashSet<K>
where
  K: Clone + Hash + Eq,
{
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<K> FxDashSet<K>
where
  K: Hash + Eq,
{
  /// Pin the set to obtain a guard for performing multiple operations within a
  /// single guard scope. See [`papaya::HashSetRef`] for the available methods.
  #[inline]
  pub fn pin(&self) -> papaya::HashSetRef<'_, K, FxBuildHasher, papaya::LocalGuard<'_>> {
    self.0.pin()
  }

  /// Inserts `value`, returning `true` if it was newly inserted.
  #[inline]
  pub fn insert(&self, value: K) -> bool {
    self.0.pin().insert(value)
  }

  #[inline]
  pub fn contains<Q>(&self, value: &Q) -> bool
  where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
  {
    self.0.pin().contains(value)
  }

  /// Removes `value`, returning `true` if it was present.
  #[inline]
  pub fn remove<Q>(&self, value: &Q) -> bool
  where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
  {
    self.0.pin().remove(value)
  }

  /// Returns a cloned snapshot of all elements. Because `papaya` references
  /// borrow a pin guard, this materializes the elements so they can be consumed
  /// after the guard is released. Iteration order is unspecified.
  #[inline]
  pub fn iter_cloned(&self) -> Vec<K>
  where
    K: Clone,
  {
    self.0.pin().iter().cloned().collect()
  }

  #[inline]
  pub fn len(&self) -> usize {
    self.0.pin().len()
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.0.pin().is_empty()
  }

  #[inline]
  pub fn clear(&self) {
    self.0.pin().clear();
  }
}
