use oxc::index::IndexVec;
use oxc::semantic::SymbolTable;
use oxc::{semantic::SymbolId, span::CompactStr as CompactString};
use rolldown_rstr::Rstr;
use rustc_hash::FxHashMap;

use crate::{ChunkIdx, ModuleIdx, SymbolRef};

use super::namespace_alias::NamespaceAlias;

#[derive(Debug)]
pub struct SymbolRefDataClassic {
  /// For case `import {a} from 'foo.cjs';console.log(a)`, the symbol `a` reference to `module.exports.a` of `foo.cjs`.
  /// So we will transform the code into `console.log(foo_ns.a)`. `foo_ns` is the namespace symbol of `foo.cjs and `a` is the property name.
  /// We use `namespace_alias` to represent this situation. If `namespace_alias` is not `None`, then this symbol must be rewritten to a property access.
  pub namespace_alias: Option<NamespaceAlias>,
  pub name: CompactString,
  /// The symbol that this symbol is linked to.
  pub link: Option<SymbolRef>,
  /// The chunk that this symbol is defined in.
  pub chunk_id: Option<ChunkIdx>,
}

bitflags::bitflags! {
  #[derive(Debug, Default)]
  pub struct SymbolRefFlags: u8 {
    const IS_NOT_REASSIGNED = 1;
    /// If this symbol is declared by `const`. Eg. `const a = 1;`
    const IS_CONST = 1 << 1;
  }
}

#[derive(Debug, Default)]
pub struct SymbolRefDbForModule {
  // Only some symbols would be cared about, so we use a hashmap to store the flags.
  pub flags: FxHashMap<SymbolId, SymbolRefFlags>,
  pub classic_data: IndexVec<SymbolId, SymbolRefDataClassic>,
}

impl SymbolRefDbForModule {
  pub fn fill_classic_data(&mut self, ast_symbols: SymbolTable) {
    self.classic_data = ast_symbols
      .names
      .into_iter()
      .map(|name| SymbolRefDataClassic { name, link: None, chunk_id: None, namespace_alias: None })
      .collect();
  }
}

// Information about symbols for all modules
#[derive(Debug, Default)]
pub struct SymbolRefDb {
  inner: IndexVec<ModuleIdx, SymbolRefDbForModule>,
}

impl SymbolRefDb {
  fn ensure_exact_capacity(&mut self, module_idx: ModuleIdx) {
    let new_len = module_idx.index() + 1;
    if self.inner.len() < new_len {
      self.inner.resize_with(new_len, SymbolRefDbForModule::default);
    }
  }

  pub fn store_local_db(&mut self, module_id: ModuleIdx, local_db: SymbolRefDbForModule) {
    self.ensure_exact_capacity(module_id);

    self.inner[module_id] = local_db;
  }

  pub fn create_symbol(&mut self, owner: ModuleIdx, name: CompactString) -> SymbolRef {
    self.ensure_exact_capacity(owner);
    let symbol_id = self.inner[owner].classic_data.push(SymbolRefDataClassic {
      name,
      link: None,
      chunk_id: None,
      namespace_alias: None,
    });
    SymbolRef { owner, symbol: symbol_id }
  }

  /// Make `base` point to `target`
  pub fn link(&mut self, base: SymbolRef, target: SymbolRef) {
    let base_root = self.find_mut(base);
    let target_root = self.find_mut(target);
    if base_root == target_root {
      // already linked
      return;
    }
    self.get_mut(base_root).link = Some(target_root);
  }

  pub fn canonical_name_for<'name>(
    &self,
    refer: SymbolRef,
    canonical_names: &'name FxHashMap<SymbolRef, Rstr>,
  ) -> &'name Rstr {
    let canonical_ref = self.canonical_ref_for(refer);
    canonical_names.get(&canonical_ref).unwrap_or_else(|| {
      panic!(
        "canonical name not found for {canonical_ref:?}, original_name: {:?}",
        refer.name(self)
      );
    })
  }

  pub fn get(&self, refer: SymbolRef) -> &SymbolRefDataClassic {
    &self.inner[refer.owner].classic_data[refer.symbol]
  }

  pub fn get_mut(&mut self, refer: SymbolRef) -> &mut SymbolRefDataClassic {
    &mut self.inner[refer.owner].classic_data[refer.symbol]
  }

  /// https://en.wikipedia.org/wiki/Disjoint-set_data_structure
  /// See Path halving
  pub fn find_mut(&mut self, target: SymbolRef) -> SymbolRef {
    let mut canonical = target;
    while let Some(parent) = self.get_mut(canonical).link {
      self.get_mut(canonical).link = self.get_mut(parent).link;
      canonical = parent;
    }

    canonical
  }

  // Used for the situation where rust require `&self`
  pub fn canonical_ref_for(&self, target: SymbolRef) -> SymbolRef {
    let mut canonical = target;
    while let Some(founded) = self.get(canonical).link {
      debug_assert!(founded != target);
      canonical = founded;
    }
    canonical
  }

  pub(crate) fn get_flags(&self, refer: SymbolRef) -> Option<&SymbolRefFlags> {
    self.inner[refer.owner].flags.get(&refer.symbol)
  }
}
