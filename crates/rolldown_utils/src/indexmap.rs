use std::hash::BuildHasherDefault;

use indexmap::{IndexMap, IndexSet};
use rustc_hash::FxHasher;

pub type FxIndexMap<K, V> = IndexMap<K, V, BuildHasherDefault<FxHasher>>;
pub type FxIndexSet<T> = IndexSet<T, BuildHasherDefault<FxHasher>>;
