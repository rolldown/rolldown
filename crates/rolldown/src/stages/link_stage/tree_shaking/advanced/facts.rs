use oxc::semantic::{NodeId, SymbolId};
use oxc_str::CompactStr;
use rolldown_common::{ModuleIdx, SymbolRef};
use rustc_hash::{FxHashMap, FxHashSet};

use super::value::Value;

/// The deepest property path that the analysis keeps apart. A deeper path counts as the whole
/// value below that level.
const MAX_PATH_DEPTH: u32 = 8;

/// A set of property paths below a binding or a value. `whole` covers every path below the node,
/// which is what an escape means: unknown code can read and write anything below it. An empty
/// tree means that code uses the value itself but reads no property of it.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PathTree {
  pub whole: bool,
  pub children: FxHashMap<CompactStr, PathTree>,
}

impl PathTree {
  pub fn whole() -> Self {
    Self { whole: true, children: FxHashMap::default() }
  }

  /// `sub` below `key`.
  pub fn prefixed(key: &str, sub: PathTree) -> Self {
    let mut children = FxHashMap::default();
    children.insert(CompactStr::new(key), sub);
    Self { whole: false, children }
  }

  /// The demand on the value at `key`, if any.
  pub fn at(&self, key: &str) -> Option<PathTree> {
    if self.whole { Some(Self::whole()) } else { self.children.get(key).cloned() }
  }

  /// Merge `other` into `self`. Returns whether `self` grew.
  pub fn merge(&mut self, other: &PathTree) -> bool {
    if self.whole {
      return false;
    }
    if other.whole {
      self.whole = true;
      self.children.clear();
      return true;
    }
    let mut changed = false;
    for (key, sub) in &other.children {
      match self.children.get_mut(key) {
        Some(existing) => changed |= existing.merge(sub),
        None => {
          self.children.insert(key.clone(), sub.clone());
          changed = true;
        }
      }
    }
    changed
  }

  /// Replace the paths below `depth` levels with `whole`.
  fn cap(&mut self, depth: u32) {
    if self.whole || self.children.is_empty() {
      return;
    }
    if depth == 0 {
      self.whole = true;
      self.children.clear();
      return;
    }
    for sub in self.children.values_mut() {
      sub.cap(depth - 1);
    }
  }
}

/// Facts about a top-level binding of an analyzed module. Keyed by its canonical ref.
#[derive(Debug, Default)]
pub struct BindingFacts {
  /// The paths that included code reads. `whole` means the binding escaped.
  pub demand: PathTree,
  /// A write to the binding exists in code that the analysis visited, so its value is unknown.
  pub reassigned: bool,
  /// The inclusion engine included the binding.
  pub included: bool,
}

/// Facts about an object literal, keyed by its module and node id.
#[derive(Debug, Default)]
pub struct EntityFacts {
  /// Unknown code can read and write the object, so every property is unknown.
  pub escaped: bool,
  /// Properties that a visited write changed.
  pub deopted_keys: FxHashSet<CompactStr>,
  /// A visited write with an unknown key exists.
  pub all_keys_deopted: bool,
}

/// Syntactic facts about functions and object literals. They never change, so they stay valid
/// for the whole build.
#[derive(Default)]
pub struct SyntaxCache {
  /// Whether a function (or the methods of an object literal) reads `this` or `super`.
  pub uses_this: FxHashMap<(ModuleIdx, NodeId), bool>,
  /// Whether a function reads its own `arguments` object.
  pub uses_arguments: FxHashMap<(ModuleIdx, NodeId), bool>,
}

/// The mutable state of the analysis. Every fact only grows, which makes the fixpoint
/// terminate.
#[derive(Default)]
pub struct State<'a> {
  pub bindings: FxHashMap<SymbolRef, BindingFacts>,
  pub entities: FxHashMap<(ModuleIdx, NodeId), EntityFacts>,
  /// Locals (parameters included) that included code reads.
  pub used_locals: FxHashSet<(ModuleIdx, SymbolId)>,
  /// A fact changed since the start of the pass.
  pub facts_changed: bool,
  /// Counts every fact change, so a caller can see whether an action changed a fact.
  pub generation: u64,
  /// Per-pass caches. Facts can change during a pass, so a cached result can be too
  /// optimistic. A pass that changes a fact always causes one more pass, and that pass starts
  /// with empty caches.
  pub call_effects: FxHashMap<(ModuleIdx, NodeId), bool>,
  pub return_values: FxHashMap<(ModuleIdx, NodeId), Value<'a>>,
  /// Functions and bindings whose analysis is in progress, to stop recursion.
  pub active_calls: FxHashSet<(ModuleIdx, NodeId)>,
  pub active_returns: FxHashSet<(ModuleIdx, NodeId)>,
  pub active_bindings: FxHashSet<SymbolRef>,
  pub active_deopts: FxHashSet<SymbolRef>,
  /// A write deopt went too deep to follow, so no object literal has a known property.
  pub all_objects_deopted: bool,
  /// An effect check assumed that a function in progress has no effects. A result that
  /// depends on that assumption must not go into the cache.
  pub used_assumption: bool,
  pub depth: u32,
  pub syntax: SyntaxCache,
}

impl State<'_> {
  pub fn begin_pass(&mut self) {
    self.facts_changed = false;
    self.call_effects.clear();
    self.return_values.clear();
  }

  fn changed(&mut self) {
    self.facts_changed = true;
    self.generation += 1;
  }

  pub fn binding_mut(&mut self, symbol_ref: SymbolRef) -> &mut BindingFacts {
    self.bindings.entry(symbol_ref).or_default()
  }

  pub fn mark_included(&mut self, symbol_ref: SymbolRef) {
    let facts = self.binding_mut(symbol_ref);
    if !facts.included {
      facts.included = true;
      self.changed();
    }
  }

  pub fn mark_reassigned(&mut self, symbol_ref: SymbolRef) {
    let facts = self.binding_mut(symbol_ref);
    if !facts.reassigned {
      facts.reassigned = true;
      self.changed();
    }
  }

  pub fn add_demand(&mut self, symbol_ref: SymbolRef, demand: &PathTree) {
    let facts = self.binding_mut(symbol_ref);
    if facts.demand.whole {
      return;
    }
    let mut demand = demand.clone();
    demand.cap(MAX_PATH_DEPTH);
    if facts.demand.merge(&demand) {
      self.changed();
    }
  }

  pub fn escape_entity(&mut self, key: (ModuleIdx, NodeId)) {
    let facts = self.entities.entry(key).or_default();
    if !facts.escaped {
      facts.escaped = true;
      self.changed();
    }
  }

  pub fn deopt_entity_key(&mut self, key: (ModuleIdx, NodeId), prop: Option<&str>) {
    let facts = self.entities.entry(key).or_default();
    let changed = match prop {
      Some(prop) => !facts.all_keys_deopted && facts.deopted_keys.insert(CompactStr::new(prop)),
      None => !std::mem::replace(&mut facts.all_keys_deopted, true),
    };
    if changed {
      self.changed();
    }
  }

  pub fn deopt_all_objects(&mut self) {
    if !self.all_objects_deopted {
      self.all_objects_deopted = true;
      self.changed();
    }
  }

  pub fn mark_local_used(&mut self, key: (ModuleIdx, SymbolId)) {
    if self.used_locals.insert(key) {
      self.changed();
    }
  }

  pub fn is_included(&self, symbol_ref: SymbolRef) -> bool {
    self.bindings.get(&symbol_ref).is_some_and(|facts| facts.included)
  }

  pub fn is_reassigned(&self, symbol_ref: SymbolRef) -> bool {
    self.bindings.get(&symbol_ref).is_some_and(|facts| facts.reassigned)
  }

  pub fn is_escaped(&self, symbol_ref: SymbolRef) -> bool {
    self.bindings.get(&symbol_ref).is_some_and(|facts| facts.demand.whole)
  }

  pub fn demand(&self, symbol_ref: SymbolRef) -> PathTree {
    self.bindings.get(&symbol_ref).map(|facts| facts.demand.clone()).unwrap_or_default()
  }

  pub fn entity(&self, key: (ModuleIdx, NodeId)) -> Option<&EntityFacts> {
    self.entities.get(&key)
  }
}
