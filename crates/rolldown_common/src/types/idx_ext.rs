use crate::{ChunkIdx, ChunkTable, ModuleIdx, ModuleTable};
use std::hash::BuildHasher;

pub trait IdxDebugExt {
  fn debug(&self, module_table: &ModuleTable, chunk_table: &ChunkTable) -> String;
}

impl IdxDebugExt for ModuleIdx {
  fn debug(&self, module_table: &ModuleTable, _chunk_table: &ChunkTable) -> String {
    module_table[*self].stable_id().to_string()
  }
}

impl IdxDebugExt for ChunkIdx {
  fn debug(&self, module_table: &ModuleTable, chunk_table: &ChunkTable) -> String {
    let chunk = &chunk_table[*self];
    let mut parts = vec![format!("{:?}", self)];
    if let Some(name) = chunk.name.as_ref() {
      parts.push(name.to_string());
    }
    if let Some(entry_module_idx) = chunk.entry_module_idx() {
      parts.push(module_table[entry_module_idx].stable_id().to_string());
    }
    parts.join(" ")
  }
}

impl<T: IdxDebugExt> IdxDebugExt for Vec<T> {
  fn debug(&self, module_table: &ModuleTable, chunk_table: &ChunkTable) -> String {
    let items: Vec<String> =
      self.iter().map(|item| item.debug(module_table, chunk_table)).collect();
    format!("[{}]", items.join(", "))
  }
}

#[expect(
  clippy::disallowed_types,
  reason = "blanket impl generic over the hasher `S`; `FxHashSet` is an alias and can't express this"
)]
impl<T: IdxDebugExt, S: BuildHasher> IdxDebugExt for std::collections::HashSet<T, S> {
  fn debug(&self, module_table: &ModuleTable, chunk_table: &ChunkTable) -> String {
    let items: Vec<String> =
      self.iter().map(|item| item.debug(module_table, chunk_table)).collect();
    format!("{{{}}}", items.join(", "))
  }
}

#[expect(
  clippy::disallowed_types,
  reason = "blanket impl generic over the hasher `S`; `FxHashMap` is an alias and can't express this"
)]
impl<K: IdxDebugExt, V: IdxDebugExt, S: BuildHasher> IdxDebugExt
  for std::collections::HashMap<K, V, S>
{
  fn debug(&self, module_table: &ModuleTable, chunk_table: &ChunkTable) -> String {
    let items: Vec<String> = self
      .iter()
      .map(|(k, v)| {
        format!("{}: {}", k.debug(module_table, chunk_table), v.debug(module_table, chunk_table))
      })
      .collect();
    format!("{{{}}}", items.join(", "))
  }
}
