use std::ops::{Deref, DerefMut};

use oxc::semantic::{ScopeId, Scoping, SymbolId};
use oxc_index::{Idx, IndexVec};
use rolldown_rstr::Rstr;
use rolldown_std_utils::OptionExt;
use rustc_hash::FxHashMap;

use crate::{AstScopes, ChunkIdx, ModuleIdx, SymbolRef};

use super::namespace_alias::NamespaceAlias;

#[derive(Debug, Clone)]
pub struct SymbolRefDataClassic {
  /// For case `import {a} from 'foo.cjs';console.log(a)`, the symbol `a` reference to `module.exports.a` of `foo.cjs`.
  /// So we will transform the code into `console.log(foo_ns.a)`. `foo_ns` is the namespace symbol of `foo.cjs and `a` is the property name.
  /// We use `namespace_alias` to represent this situation. If `namespace_alias` is not `None`, then this symbol must be rewritten to a property access.
  pub namespace_alias: Option<NamespaceAlias>,
  /// The symbol that this symbol is linked to.
  pub link: Option<SymbolRef>,
  /// The chunk that this symbol is defined in.
  pub chunk_id: Option<ChunkIdx>,
}

bitflags::bitflags! {
  #[derive(Debug, Default, Clone, Copy)]
  pub struct SymbolRefFlags: u8 {
    const IS_NOT_REASSIGNED = 1;
    /// If this symbol is declared by `const`. Eg. `const a = 1;`
    const IS_CONST = 1 << 1;
  }
}

#[derive(Debug)]
pub struct SymbolRefDbForModule {
  pub owner_idx: ModuleIdx,
  root_scope_id: ScopeId,
  pub ast_scopes: AstScopes,
  // Only some symbols would be cared about, so we use a hashmap to store the flags.
  pub flags: FxHashMap<SymbolId, SymbolRefFlags>,
  pub classic_data: IndexVec<SymbolId, SymbolRefDataClassic>,
}

impl Default for SymbolRefDbForModule {
  fn default() -> Self {
    Self {
      owner_idx: ModuleIdx::new(0),
      root_scope_id: ScopeId::new(0),
      ast_scopes: AstScopes::new(Scoping::default()),
      flags: FxHashMap::default(),
      classic_data: IndexVec::default(),
    }
  }
}

impl SymbolRefDbForModule {
  pub fn new(scoping: Scoping, owner_idx: ModuleIdx, top_level_scope_id: ScopeId) -> Self {
    Self {
      owner_idx,
      root_scope_id: top_level_scope_id,
      classic_data: IndexVec::from_vec(vec![
        SymbolRefDataClassic {
          link: None,
          chunk_id: None,
          namespace_alias: None,
        };
        scoping.symbols_len()
      ]),

      ast_scopes: AstScopes::new(scoping),
      flags: FxHashMap::default(),
    }
  }

  /// The `facade` means the symbol is actually not exist in the AST.
  pub fn create_facade_root_symbol_ref(&mut self, name: &str) -> SymbolRef {
    let symbol_id = self.ast_scopes.create_facade_root_symbol_ref(name);

    SymbolRef::from((self.owner_idx, symbol_id))
  }

  /// This method is used to hide the `SymbolTable::create_symbol` method since
  /// `SymbolRefDbForModule` impl `Deref` for `SymbolTable`.
  #[deprecated = "Use `create_facade_root_symbol_ref` instead"]
  pub fn create_symbol(&mut self) {
    panic!("Use `create_facade_root_symbol_ref` instead");
  }

  /// # Panics
  /// - If the symbol is not declared in the module.
  pub fn get_classic_data(&self, symbol_id: SymbolId) -> &SymbolRefDataClassic {
    if symbol_id.index() < self.ast_scopes.real_symbol_length() {
      return &self.classic_data[symbol_id];
    }
    self
      .ast_scopes
      .facade_scoping
      .facade_symbol_classic_data
      .get(&symbol_id)
      .unwrap_or_else(|| panic!("No symbol found for {:?} -> {symbol_id:?}", self.owner_idx))
  }

  pub fn get_classic_data_mut(&mut self, symbol_id: SymbolId) -> &mut SymbolRefDataClassic {
    if symbol_id.index() < self.ast_scopes.real_symbol_length() {
      return &mut self.classic_data[symbol_id];
    }
    self.ast_scopes.facade_scoping.facade_symbol_classic_data.get_mut(&symbol_id).unwrap()
  }
}

impl Deref for SymbolRefDbForModule {
  type Target = AstScopes;

  fn deref(&self) -> &Self::Target {
    &self.ast_scopes
  }
}

impl DerefMut for SymbolRefDbForModule {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.ast_scopes
  }
}

// Information about symbols for all modules
#[derive(Debug, Default)]
pub struct SymbolRefDb {
  inner: IndexVec<ModuleIdx, Option<SymbolRefDbForModule>>,
}

impl SymbolRefDb {
  #[must_use]
  pub fn clone_without_scoping(&self) -> SymbolRefDb {
    let mut vec = IndexVec::with_capacity(self.inner.len());
    for inner in &self.inner {
      vec.push(inner.as_ref().map(|inner| SymbolRefDbForModule {
        owner_idx: inner.owner_idx,
        root_scope_id: inner.root_scope_id,
        ast_scopes: inner.clone_facade_only(),
        flags: inner.flags.clone(),
        classic_data: inner.classic_data.clone(),
      }));
    }
    Self { inner: vec }
  }
}

impl std::ops::Index<ModuleIdx> for SymbolRefDb {
  type Output = Option<SymbolRefDbForModule>;

  fn index(&self, index: ModuleIdx) -> &Self::Output {
    self.inner.index(index)
  }
}

impl std::ops::IndexMut<ModuleIdx> for SymbolRefDb {
  fn index_mut(&mut self, index: ModuleIdx) -> &mut Self::Output {
    self.inner.index_mut(index)
  }
}

impl SymbolRefDb {
  fn ensure_exact_capacity(&mut self, module_idx: ModuleIdx) {
    let new_len = module_idx.index() + 1;
    if self.inner.len() < new_len {
      self.inner.resize_with(new_len, || None);
    }
  }

  pub fn inner(self) -> IndexVec<ModuleIdx, Option<SymbolRefDbForModule>> {
    self.inner
  }

  pub fn store_local_db(&mut self, module_id: ModuleIdx, local_db: SymbolRefDbForModule) {
    self.ensure_exact_capacity(module_id);

    self.inner[module_id] = Some(local_db);
  }

  pub fn create_facade_root_symbol_ref(&mut self, owner: ModuleIdx, name: &str) -> SymbolRef {
    self.ensure_exact_capacity(owner);
    self.inner[owner].unpack_ref_mut().create_facade_root_symbol_ref(name)
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
    &'name self,
    refer: SymbolRef,
    canonical_names: &'name FxHashMap<SymbolRef, Rstr>,
  ) -> &'name str {
    let canonical_ref = self.canonical_ref_for(refer);
    canonical_names
      .get(&canonical_ref)
      .map_or_else(move || refer.name(self), rolldown_rstr::Rstr::as_str)
  }

  pub fn get(&self, refer: SymbolRef) -> &SymbolRefDataClassic {
    self.inner[refer.owner].unpack_ref().get_classic_data(refer.symbol)
  }

  pub fn get_mut(&mut self, refer: SymbolRef) -> &mut SymbolRefDataClassic {
    self.inner[refer.owner].unpack_ref_mut().get_classic_data_mut(refer.symbol)
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

  pub fn is_declared_in_root_scope(&self, refer: SymbolRef) -> bool {
    let local_db = self.inner[refer.owner].unpack_ref();
    local_db.ast_scopes.symbol_scope_id(refer.symbol) == local_db.root_scope_id
  }
}

pub trait GetLocalDb {
  fn local_db(&self, owner: ModuleIdx) -> &SymbolRefDbForModule;
}

pub trait GetLocalDbMut {
  fn local_db_mut(&mut self, owner: ModuleIdx) -> &mut SymbolRefDbForModule;
}

impl GetLocalDb for SymbolRefDb {
  fn local_db(&self, owner: ModuleIdx) -> &SymbolRefDbForModule {
    self.inner[owner].unpack_ref()
  }
}

impl GetLocalDbMut for SymbolRefDb {
  fn local_db_mut(&mut self, owner: ModuleIdx) -> &mut SymbolRefDbForModule {
    self.inner[owner].unpack_ref_mut()
  }
}

impl GetLocalDb for SymbolRefDbForModule {
  fn local_db(&self, owner: ModuleIdx) -> &SymbolRefDbForModule {
    debug_assert!(self.owner_idx == owner);
    self
  }
}

impl GetLocalDbMut for SymbolRefDbForModule {
  fn local_db_mut(&mut self, owner: ModuleIdx) -> &mut SymbolRefDbForModule {
    debug_assert!(self.owner_idx == owner);
    self
  }
}
