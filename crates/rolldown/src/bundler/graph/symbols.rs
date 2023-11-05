use index_vec::IndexVec;
use oxc::{
  semantic::{ScopeId, SymbolId, SymbolTable},
  span::Atom,
};
use rolldown_common::{ModuleId, Specifier, SymbolRef};
use rustc_hash::FxHashMap;

use crate::bundler::chunk::ChunkId;

#[derive(Debug)]
pub struct NamespaceAlias {
  pub property_name: Atom,
  pub namespace_ref: SymbolRef,
}

#[derive(Debug)]
pub struct Symbol {
  /// For case `import {a} from 'foo.cjs';console.log(a)`, the symbol `a` reference to `module.exports.a` of `foo.cjs`.
  /// So we will transform the code into `console.log(foo_ns.a)`. `foo_ns` is the namespace symbol of `foo.cjs and `a` is the property name.
  /// We use `namespace_alias` to represent this situation. If `namespace_alias` is not `None`, then this symbol must be rewritten to a property access.
  pub namespace_alias: Option<NamespaceAlias>,
  pub name: Atom,
  /// The symbol that this symbol is linked to.
  pub link: Option<SymbolRef>,
  /// The chunk that this symbol is defined in.
  pub chunk_id: Option<ChunkId>,
  // FIXME: should not place those fields here
  // Only for external modules
  pub exported_as: Option<Specifier>,
}

#[derive(Debug, Default)]
pub struct AstSymbol {
  pub names: IndexVec<SymbolId, Atom>,
  pub scope_ids: IndexVec<SymbolId, ScopeId>,
}

impl AstSymbol {
  pub fn from_symbol_table(table: SymbolTable) -> Self {
    debug_assert!(table.references.is_empty());
    Self { names: table.names, scope_ids: table.scope_ids }
  }

  pub fn create_symbol(&mut self, name: Atom, scope_id: ScopeId) -> SymbolId {
    self.scope_ids.push(scope_id);
    self.names.push(name)
  }

  pub fn scope_id_for(&self, symbol_id: SymbolId) -> ScopeId {
    self.scope_ids[symbol_id]
  }
}

// Information about symbols for all modules
#[derive(Debug, Default)]
pub struct Symbols {
  inner: IndexVec<ModuleId, IndexVec<SymbolId, Symbol>>,
}

impl Symbols {
  pub fn new(tables: IndexVec<ModuleId, AstSymbol>) -> Self {
    let inner = tables
      .into_iter()
      .map(|table| {
        table
          .names
          .into_iter()
          .map(|name| Symbol {
            name,
            link: None,
            chunk_id: None,
            namespace_alias: None,
            exported_as: None,
          })
          .collect()
      })
      .collect();

    Self { inner }
  }

  pub fn create_symbol(&mut self, owner: ModuleId, name: Atom) -> SymbolRef {
    let symbol_id = self.inner[owner].push(Symbol {
      name,
      link: None,
      chunk_id: None,
      namespace_alias: None,
      exported_as: None,
    });
    SymbolRef { owner, symbol: symbol_id }
  }

  /// Make a point to b
  pub fn union(&mut self, a: SymbolRef, b: SymbolRef) {
    // a link to b
    let root_a = self.canonical_ref_for(a);
    let root_b = self.canonical_ref_for(b);
    if root_a == root_b {
      return;
    }
    self.get_mut(root_a).link = Some(root_b);
  }

  pub fn get_original_name(&self, refer: SymbolRef) -> &Atom {
    &self.get(refer).name
  }

  pub fn canonical_name_for<'name>(
    &self,
    refer: SymbolRef,
    canonical_names: &'name FxHashMap<SymbolRef, Atom>,
  ) -> &'name Atom {
    let canonical_ref = self.par_canonical_ref_for(refer);
    &canonical_names[&canonical_ref]
  }

  pub fn get(&self, refer: SymbolRef) -> &Symbol {
    &self.inner[refer.owner][refer.symbol]
  }

  pub fn get_mut(&mut self, refer: SymbolRef) -> &mut Symbol {
    &mut self.inner[refer.owner][refer.symbol]
  }

  pub fn canonical_ref_for(&mut self, target: SymbolRef) -> SymbolRef {
    let canonical = self.par_canonical_ref_for(target);
    if target != canonical {
      // update the link to the canonical so that the next time we can get the canonical directly
      self.get_mut(target).link = Some(canonical);
    }
    canonical
  }

  // Used for the situation where rust require `&self`
  pub fn par_canonical_ref_for(&self, target: SymbolRef) -> SymbolRef {
    let mut canonical = target;
    while let Some(founded) = self.get(canonical).link {
      debug_assert!(founded != target);
      canonical = founded;
    }
    canonical
  }
}
