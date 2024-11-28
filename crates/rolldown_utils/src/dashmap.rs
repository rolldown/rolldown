use dashmap::{DashMap, DashSet};
use rustc_hash::FxBuildHasher;

pub type FxDashMap<K, V> = DashMap<K, V, FxBuildHasher>;
pub type FxDashSet<V> = DashSet<V, FxBuildHasher>;
