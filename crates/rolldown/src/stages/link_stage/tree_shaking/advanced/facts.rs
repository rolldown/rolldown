use oxc::{
  ast::ast::{Argument, BindingPattern, Expression},
  semantic::{NodeId, SymbolId},
};
use oxc_str::CompactStr;
use rolldown_common::{ModuleIdx, SymbolRef};
use rolldown_utils::indexmap::FxIndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  env::Site,
  value::{FnNode, FnValue, Value},
};

/// A function or an object literal: its module and node id.
pub type NodeKey = (ModuleIdx, NodeId);
/// A binding inside a function: its module and symbol.
pub type LocalKey = (ModuleIdx, SymbolId);
/// A list of property keys. `None` is an unknown key.
pub type KeyPath = Vec<Option<CompactStr>>;

/// The deepest property path that the analysis keeps apart. A deeper path counts as the whole
/// value below that level.
const MAX_PATH_DEPTH: u32 = 8;

/// A set of property paths below a binding or a value. `whole` covers every path below the node,
/// which is what an escape means: unknown code can read and write anything below it. An empty
/// tree means that code uses the value itself but reads no property of it.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PathTree {
  pub whole: bool,
  /// Code reads each own key of the value but uses no value at a key that `children` does not
  /// name. A rest element of an object pattern does this, so a getter at any key runs.
  pub every_key: bool,
  pub children: FxHashMap<CompactStr, PathTree>,
}

impl PathTree {
  pub fn whole() -> Self {
    Self { whole: true, every_key: false, children: FxHashMap::default() }
  }

  /// `sub` below `key`.
  pub fn prefixed(key: &str, sub: PathTree) -> Self {
    let mut children = FxHashMap::default();
    children.insert(CompactStr::new(key), sub);
    Self { whole: false, every_key: false, children }
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
      self.every_key = false;
      self.children.clear();
      return true;
    }
    let mut changed = other.every_key && !self.every_key;
    self.every_key |= other.every_key;
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

/// Facts about an object literal, an array literal, a function, or a class, keyed by its module
/// and node id.
#[derive(Debug, Default)]
pub struct EntityFacts {
  /// Unknown code can read and write the object, so every property is unknown. For a function,
  /// unknown code can also call it, so its parameters are unknown and its return values escape.
  pub escaped: bool,
  /// The paths that included code can read from the objects that the literal creates. It is the
  /// union of the demands of the walks of the literal.
  pub demand: PathTree,
  /// Properties that a visited write or `delete` changed.
  pub deopted_keys: FxHashSet<CompactStr>,
  /// Properties that a visited `delete` can remove. Then a read or a write can reach the
  /// prototype.
  pub deleted_keys: FxHashSet<CompactStr>,
  /// A visited write with an unknown key exists.
  pub all_keys_deopted: bool,
  /// `Object.freeze` can freeze the objects, so a write or a `delete` can throw.
  pub frozen: bool,
}

/// Syntactic facts about functions and object literals. They never change, so they stay valid
/// for the whole build.
#[derive(Default)]
pub struct SyntaxCache {
  /// Whether a function (or the methods of an object literal) reads `this` or `super`.
  pub uses_this: FxHashMap<(ModuleIdx, NodeId), bool>,
  /// Whether a function reads its own `arguments` object.
  pub uses_arguments: FxHashMap<(ModuleIdx, NodeId), bool>,
  /// Whether code of a class can reach the class object in a way that the analysis does not
  /// track.
  pub class_opaque: FxHashMap<(ModuleIdx, NodeId), bool>,
}

/// A call of a known function that can run.
#[derive(Debug, Clone, Copy)]
pub struct CallRecord<'a> {
  pub site: Site,
  pub args: &'a [Argument<'a>],
}

/// A parameter of a function.
#[derive(Debug, Clone, Copy)]
pub struct ParamInfo<'a> {
  pub module: ModuleIdx,
  pub func: FnNode<'a>,
  /// The position of the parameter. For the rest parameter, the number of other parameters.
  pub index: usize,
  /// The pattern of the parameter, when the binding is inside a destructuring pattern.
  pub pattern: Option<&'a BindingPattern<'a>>,
  pub rest: bool,
  /// The parameter has a default value.
  pub has_default: bool,
}

/// The initializer of a binding inside a function.
#[derive(Debug, Clone, Copy)]
pub struct LocalInit<'a> {
  pub init: &'a Expression<'a>,
  /// The pattern of the declaration, when the binding is inside a destructuring pattern.
  pub pattern: Option<&'a BindingPattern<'a>>,
  pub site: Site,
}

/// The mutable state of the analysis. Every fact only grows, which makes the fixpoint
/// terminate.
#[derive(Default)]
#[expect(clippy::struct_excessive_bools)]
pub struct State<'a> {
  /// Facts about top-level bindings (keyed by the canonical ref) and about bindings inside
  /// functions (keyed by their own ref).
  pub bindings: FxHashMap<SymbolRef, BindingFacts>,
  pub entities: FxHashMap<NodeKey, EntityFacts>,
  /// Locals (parameters included) that included code reads.
  pub used_locals: FxHashSet<LocalKey>,
  /// For each known function, the calls of it that can run: calls in included code, and calls
  /// that an effect check or a value evaluation saw. Keyed by the call.
  pub calls: FxHashMap<NodeKey, FxIndexMap<NodeKey, CallRecord<'a>>>,
  /// For each function, the demand on the values that its calls return.
  pub return_demands: FxHashMap<NodeKey, PathTree>,
  /// For each function, the write deopts of its parameters: an argument position and the path
  /// below the argument. They apply to the argument of each call.
  pub arg_deopts: FxHashMap<NodeKey, Vec<(usize, KeyPath)>>,
  pub params: FxHashMap<LocalKey, ParamInfo<'a>>,
  pub local_inits: FxHashMap<LocalKey, LocalInit<'a>>,
  /// Function declarations inside functions and blocks.
  pub local_fns: FxHashMap<LocalKey, FnValue<'a>>,
  /// Functions whose parameters are in `params`.
  pub registered_fns: FxHashSet<NodeKey>,
  /// Modules with a direct `eval` in included code.
  pub live_evals: FxHashSet<ModuleIdx>,
  /// A fact changed since the start of the pass.
  pub facts_changed: bool,
  /// Counts every fact change, so a caller can see whether an action changed a fact.
  pub generation: u64,
  /// Per-pass caches. Facts can change during a pass, so a cached result can be too
  /// optimistic. A pass that changes a fact always causes one more pass, and that pass starts
  /// with empty caches.
  pub call_effects: FxHashMap<NodeKey, bool>,
  /// The effects of `new` of a function or a class, keyed by the function or the class.
  pub construct_effects: FxHashMap<NodeKey, bool>,
  pub return_values: FxHashMap<NodeKey, Value<'a>>,
  pub param_values: FxHashMap<LocalKey, Value<'a>>,
  /// Functions and bindings whose analysis is in progress, to stop recursion.
  pub active_calls: FxHashSet<NodeKey>,
  pub active_constructs: FxHashSet<NodeKey>,
  pub active_returns: FxHashSet<NodeKey>,
  pub active_bindings: FxHashSet<SymbolRef>,
  pub active_deopts: FxHashSet<SymbolRef>,
  pub active_params: FxHashSet<LocalKey>,
  /// A write deopt went too deep to follow, so no object literal has a known property.
  pub all_objects_deopted: bool,
  /// The deopt in progress is the one of a `delete`.
  pub deleting: bool,
  /// An effect check assumed that a function in progress has no effects. A result that
  /// depends on that assumption must not go into the cache.
  pub used_assumption: bool,
  pub depth: u32,
  pub syntax: SyntaxCache,
}

impl<'a> State<'a> {
  pub fn begin_pass(&mut self) {
    self.facts_changed = false;
    self.clear_caches();
  }

  fn clear_caches(&mut self) {
    self.call_effects.clear();
    self.construct_effects.clear();
    self.return_values.clear();
    self.param_values.clear();
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

  pub fn escape_entity(&mut self, key: NodeKey) {
    let facts = self.entities.entry(key).or_default();
    if !facts.escaped {
      facts.escaped = true;
      self.changed();
    }
  }

  pub fn add_entity_demand(&mut self, key: NodeKey, demand: &PathTree) {
    let facts = self.entities.entry(key).or_default();
    if facts.demand.whole {
      return;
    }
    let mut demand = demand.clone();
    demand.cap(MAX_PATH_DEPTH);
    if facts.demand.merge(&demand) {
      self.changed();
    }
  }

  pub fn deopt_entity_key(&mut self, key: NodeKey, prop: Option<&str>) {
    let deleting = self.deleting;
    let facts = self.entities.entry(key).or_default();
    let mut changed = match prop {
      Some(prop) => !facts.all_keys_deopted && facts.deopted_keys.insert(CompactStr::new(prop)),
      None => !std::mem::replace(&mut facts.all_keys_deopted, true),
    };
    if deleting && let Some(prop) = prop {
      changed |= facts.deleted_keys.insert(CompactStr::new(prop));
    }
    if changed {
      self.changed();
    }
  }

  pub fn freeze_entity(&mut self, key: NodeKey) {
    let facts = self.entities.entry(key).or_default();
    if !facts.frozen {
      facts.frozen = true;
      self.changed();
    }
  }

  pub fn deopt_all_objects(&mut self) {
    if !self.all_objects_deopted {
      self.all_objects_deopted = true;
      self.changed();
    }
  }

  /// Returns whether the module is new.
  pub fn mark_live_eval(&mut self, module: ModuleIdx) -> bool {
    if self.live_evals.insert(module) {
      self.changed();
      return true;
    }
    false
  }

  pub fn mark_local_used(&mut self, key: LocalKey) {
    if self.used_locals.insert(key) {
      self.changed();
    }
  }

  /// Returns whether the call is new. A new call can change the value of a parameter, so the
  /// per-pass caches are cleared.
  pub fn record_call(&mut self, func: NodeKey, call: NodeKey, record: CallRecord<'a>) -> bool {
    let calls = self.calls.entry(func).or_default();
    if calls.contains_key(&call) {
      return false;
    }
    calls.insert(call, record);
    self.clear_caches();
    self.changed();
    true
  }

  pub fn add_return_demand(&mut self, func: NodeKey, demand: &PathTree) {
    let current = self.return_demands.entry(func).or_default();
    if current.whole {
      return;
    }
    let mut demand = demand.clone();
    demand.cap(MAX_PATH_DEPTH);
    if current.merge(&demand) {
      self.changed();
    }
  }

  /// Returns whether the deopt is new.
  pub fn add_arg_deopt(
    &mut self,
    func: NodeKey,
    index: usize,
    path: &[Option<CompactStr>],
  ) -> bool {
    let deopts = self.arg_deopts.entry(func).or_default();
    if deopts.iter().any(|(i, p)| *i == index && p.as_slice() == path) {
      return false;
    }
    deopts.push((index, path.to_vec()));
    self.changed();
    true
  }

  /// A write deopt follows the initializer of a local, so a new initializer is a new fact.
  pub fn register_local_init(&mut self, key: LocalKey, init: LocalInit<'a>) {
    if let std::collections::hash_map::Entry::Vacant(entry) = self.local_inits.entry(key) {
      entry.insert(init);
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

  pub fn entity(&self, key: NodeKey) -> Option<&EntityFacts> {
    self.entities.get(&key)
  }

  pub fn return_demand(&self, func: NodeKey) -> PathTree {
    self.return_demands.get(&func).cloned().unwrap_or_default()
  }
}
