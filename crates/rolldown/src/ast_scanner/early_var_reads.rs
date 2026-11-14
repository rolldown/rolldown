//! Finds root `var` bindings that code reads before their declarator assigns them. See
//! `internal-docs/inline-const/design.md`.

use oxc::semantic::{Scoping, SymbolId};
use rustc_hash::{FxHashMap, FxHashSet};

/// Where code that reads or references a root binding runs.
#[derive(Debug, Clone, Copy)]
pub enum RunsAt {
  /// Module-level code at this source position.
  Position(u32),
  /// The body of this root function, which runs when module-level code references the function.
  Body(SymbolId),
}

#[derive(Debug, Default)]
pub struct EarlyVarReads {
  /// Where the code inside the outermost function or class being visited runs.
  nested: Option<RunsAt>,
  /// How many functions and classes enclose the current position.
  depth: u32,
  /// References to root bindings, with where each one runs.
  refs: Vec<(SymbolId, RunsAt)>,
  /// Root functions whose bodies are tracked as `RunsAt::Body`.
  bodies: FxHashSet<SymbolId>,
  /// Root `var`s mapped to the end of their declarator, where they get their value.
  assigned_at: FxHashMap<SymbolId, u32>,
}

impl EarlyVarReads {
  pub fn is_nested(&self) -> bool {
    self.depth > 0
  }

  /// Enters a function or class. `outermost` is where its code runs, given only when no function
  /// or class encloses it.
  pub fn enter(&mut self, outermost: Option<RunsAt>) {
    if let Some(runs_at) = outermost {
      if let RunsAt::Body(function) = runs_at {
        self.bodies.insert(function);
      }
      self.nested = Some(runs_at);
    }
    self.depth += 1;
  }

  pub fn leave(&mut self) {
    self.depth -= 1;
    if self.depth == 0 {
      self.nested = None;
    }
  }

  /// Records a reference to a root binding at `position`.
  pub fn add_ref(&mut self, symbol_id: SymbolId, position: u32) {
    let runs_at = self.nested.unwrap_or(RunsAt::Position(position));
    self.refs.push((symbol_id, runs_at));
  }

  /// Records that a root `var` gets its value where its declarator ends.
  pub fn add_var_declarator(&mut self, symbol_id: SymbolId, end: u32) {
    self.assigned_at.insert(symbol_id, end);
  }

  /// Returns the root `var`s that code reads before they get their value.
  pub fn finish(self, scoping: &Scoping) -> FxHashSet<SymbolId> {
    // The earliest module-level position at which each root function body can run.
    let mut first_run: FxHashMap<SymbolId, u32> = FxHashMap::default();
    let mut callees: FxHashMap<SymbolId, Vec<SymbolId>> = FxHashMap::default();
    for &(symbol_id, runs_at) in &self.refs {
      if !self.bodies.contains(&symbol_id) {
        continue;
      }
      match runs_at {
        RunsAt::Position(position) => {
          first_run
            .entry(symbol_id)
            .and_modify(|first| *first = (*first).min(position))
            .or_insert(position);
        }
        RunsAt::Body(caller) => callees.entry(caller).or_default().push(symbol_id),
      }
    }
    let mut stack: Vec<SymbolId> = first_run.keys().copied().collect();
    while let Some(caller) = stack.pop() {
      let position = first_run[&caller];
      for &callee in callees.get(&caller).into_iter().flatten() {
        if first_run.get(&callee).is_none_or(|&first| position < first) {
          first_run.insert(callee, position);
          stack.push(callee);
        }
      }
    }

    self
      .refs
      .iter()
      .filter_map(|&(symbol_id, runs_at)| {
        if !scoping.symbol_flags(symbol_id).is_function_scoped_declaration() {
          return None;
        }
        let position = match runs_at {
          RunsAt::Position(position) => position,
          RunsAt::Body(function) => *first_run.get(&function)?,
        };
        let assigned_at = self
          .assigned_at
          .get(&symbol_id)
          .copied()
          .unwrap_or_else(|| scoping.symbol_span(symbol_id).start);
        (position < assigned_at).then_some(symbol_id)
      })
      .collect()
  }
}
