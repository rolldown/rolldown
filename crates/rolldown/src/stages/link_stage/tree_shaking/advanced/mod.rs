//! Advanced tree shaking: a path-keyed analysis of the code that the inclusion engine includes.
//! It removes unread object properties, unused call arguments, dead branches, and code whose only
//! effect is a write that nothing reads. It runs in the link stage, before chunking, so every
//! chunk sees the same result.
//!
//! See internal-docs/advanced-tree-shaking/implementation.md.

mod collect;
mod effects;
mod env;
mod facts;
mod value;
mod walk;

use oxc::{ast::ast::Statement, semantic::NodeId};
use rolldown_common::{
  ImportKind, ImportRecordIdx, ModuleIdx, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef,
};
use rolldown_utils::indexmap::{FxIndexMap, FxIndexSet};
use rustc_hash::FxHashSet;

use crate::types::tree_shaking_rewrites::TreeShakingRewrites;

pub use self::env::Env;
use self::{
  collect::{Excluded, RefCollector, Refs},
  effects::Cx,
  env::Site,
  facts::{PathTree, State},
  walk::Walker,
};
use super::include_statements::StmtInclusionVec;

/// The inputs and the facts, for the value, effect, and walk methods.
pub struct Analyzer<'s, 'a> {
  pub env: &'s Env<'a>,
  pub state: &'s mut State<'a>,
}

/// The result of the walks of one included statement.
struct StmtRecord {
  /// Which entries of the statement's `referenced_symbols` the output needs. This is the union
  /// over all walks, because the inclusion engine already followed an entry that an earlier walk
  /// needed.
  kept: Vec<bool>,
  /// Import records that only excluded parts use. From the latest walk.
  excluded_records: Vec<ImportRecordIdx>,
  /// The node ids in excluded parts that `module.imports` maps to a record. From the latest walk.
  excluded_imports: FxHashSet<NodeId>,
  /// From the latest walk.
  rewrites: TreeShakingRewrites,
}

/// What one pass found.
pub struct PassResult {
  /// Statements that have effects now. The inclusion engine must include them.
  pub new_stmts: Vec<(ModuleIdx, StmtInfoIdx)>,
  /// Included statements that need more of their references, or whose excluded import records
  /// changed, with the indexes in `referenced_symbols` that the output needs now.
  pub grown: Vec<(ModuleIdx, StmtInfoIdx, Vec<usize>)>,
  /// A fact changed during the pass, so a result of the pass can be too optimistic.
  pub facts_changed: bool,
}

/// The final decisions, for the write-back after the inclusion fixpoint.
pub struct AdvancedOutput {
  /// For each walked statement that loses a part: which entries of `referenced_symbols` to keep,
  /// and the import records to remove.
  pub stmts: Vec<(ModuleIdx, StmtInfoIdx, Vec<bool>, Vec<ImportRecordIdx>)>,
  pub rewrites: Vec<(ModuleIdx, TreeShakingRewrites)>,
  /// Dynamic imports that only excluded parts contain.
  pub dead_dynamic_imports: Vec<(ModuleIdx, ImportRecordIdx)>,
}

/// The analysis is an optimistic fixpoint, like rollup's. It assumes that nothing is included,
/// read, or written, and it gives up an assumption when included code (or code that an effect
/// check calls free of effects) breaks it. Each pass walks all included statements again with the
/// facts of the previous passes. A pass that changes no fact and finds no new code ends it.
pub struct AdvancedTreeShaking<'a> {
  env: Env<'a>,
  state: State<'a>,
  walked: FxIndexMap<(ModuleIdx, StmtInfoIdx), StmtRecord>,
  /// Statements that the sweep would include, but that have no effects under the current facts.
  candidates: FxIndexSet<(ModuleIdx, StmtInfoIdx)>,
}

impl<'a> AdvancedTreeShaking<'a> {
  pub fn new(env: Env<'a>) -> Self {
    Self {
      env,
      state: State::default(),
      walked: FxIndexMap::default(),
      candidates: FxIndexSet::default(),
    }
  }

  /// Counts every fact change.
  pub fn generation(&self) -> u64 {
    self.state.generation
  }

  /// Whether the analysis walks the statement when the inclusion engine includes it. Import and
  /// re-export statements contain no code.
  pub fn is_walked_stmt(&self, module: ModuleIdx, stmt_idx: StmtInfoIdx) -> bool {
    self.env.module(module).and_then(|facts| facts.stmt(stmt_idx)).is_some_and(|stmt| {
      !matches!(
        stmt,
        Statement::ImportDeclaration(_)
          | Statement::ExportAllDeclaration(_)
          | Statement::ExportFromDeclaration(_)
      )
    })
  }

  /// The inclusion engine included a symbol. When no analyzed reference included it, code that
  /// the analysis does not see uses it, so its value escapes.
  pub fn on_symbol_included(&mut self, canonical: SymbolRef, analyzed: bool) {
    if self.env.module(canonical.owner).is_none() {
      return;
    }
    self.state.mark_included(canonical);
    if !analyzed {
      Analyzer { env: &self.env, state: &mut self.state }
        .add_demand_to(canonical, &PathTree::whole());
    }
  }

  /// Walk a statement that the inclusion engine just included. Returns which entries of its
  /// `referenced_symbols` the engine must follow, and the import records that it must skip.
  pub fn walk_new_stmt(
    &mut self,
    module: ModuleIdx,
    stmt_idx: StmtInfoIdx,
  ) -> (Vec<bool>, Vec<ImportRecordIdx>) {
    self.candidates.shift_remove(&(module, stmt_idx));
    let record = self.walk(module, stmt_idx);
    let result = (record.kept.clone(), record.excluded_records.clone());
    self.walked.insert((module, stmt_idx), record);
    result
  }

  /// Whether the side-effect sweep must include the statement. A statement without effects under
  /// the current facts waits as a candidate, and each pass checks it again.
  pub fn sweep_has_effects(&mut self, module: ModuleIdx, stmt_idx: StmtInfoIdx) -> bool {
    if !self.is_walked_stmt(module, stmt_idx) {
      return true;
    }
    if self.stmt_has_effects(module, stmt_idx) {
      self.candidates.shift_remove(&(module, stmt_idx));
      return true;
    }
    self.candidates.insert((module, stmt_idx));
    false
  }

  /// Check the candidates again, and walk every included statement again. The per-pass caches
  /// start empty.
  pub fn run_pass(&mut self, included: &StmtInclusionVec) -> PassResult {
    self.state.begin_pass();
    let mut new_stmts = Vec::new();
    for (module, stmt_idx) in std::mem::take(&mut self.candidates) {
      if included[module].has_bit(stmt_idx) {
        continue;
      }
      if self.stmt_has_effects(module, stmt_idx) {
        new_stmts.push((module, stmt_idx));
      } else {
        self.candidates.insert((module, stmt_idx));
      }
    }
    let mut grown = Vec::new();
    let walked = self.walked.keys().copied().collect::<Vec<_>>();
    for (module, stmt_idx) in walked {
      let record = self.walk(module, stmt_idx);
      let previous = &mut self.walked[&(module, stmt_idx)];
      let mut newly_kept = Vec::new();
      for (idx, kept) in record.kept.iter().enumerate() {
        if *kept && !previous.kept[idx] {
          previous.kept[idx] = true;
          newly_kept.push(idx);
        }
      }
      let records_changed = previous.excluded_records != record.excluded_records;
      previous.excluded_records = record.excluded_records;
      previous.excluded_imports = record.excluded_imports;
      previous.rewrites = record.rewrites;
      if !newly_kept.is_empty() || records_changed {
        grown.push((module, stmt_idx, newly_kept));
      }
    }
    PassResult { new_stmts, grown, facts_changed: self.state.facts_changed }
  }

  pub fn excluded_records(&self, module: ModuleIdx, stmt_idx: StmtInfoIdx) -> &[ImportRecordIdx] {
    self.walked.get(&(module, stmt_idx)).map_or(&[], |record| record.excluded_records.as_slice())
  }

  /// Whether the import expression `node` is in an excluded part of an included statement.
  pub fn is_import_node_excluded(
    &self,
    module: ModuleIdx,
    stmt_idx: StmtInfoIdx,
    node: NodeId,
  ) -> bool {
    self
      .walked
      .get(&(module, stmt_idx))
      .is_some_and(|record| record.excluded_imports.contains(&node))
  }

  pub fn finish(self) -> AdvancedOutput {
    let mut stmts = Vec::new();
    let mut rewrites = FxIndexMap::<ModuleIdx, TreeShakingRewrites>::default();
    let mut dead_dynamic_imports = Vec::new();
    for ((module_idx, stmt_idx), record) in self.walked {
      if let Some(facts) = self.env.module(module_idx) {
        dead_dynamic_imports.extend(
          record
            .excluded_records
            .iter()
            .filter(|record_idx| {
              facts.module.import_records[**record_idx].kind == ImportKind::DynamicImport
            })
            .map(|record_idx| (module_idx, *record_idx)),
        );
      }
      if !record.rewrites.is_empty() {
        rewrites.entry(module_idx).or_default().merge(record.rewrites);
      }
      if record.kept.contains(&false) || !record.excluded_records.is_empty() {
        stmts.push((module_idx, stmt_idx, record.kept, record.excluded_records));
      }
    }
    AdvancedOutput { stmts, rewrites: rewrites.into_iter().collect(), dead_dynamic_imports }
  }

  fn stmt_has_effects(&mut self, module: ModuleIdx, stmt_idx: StmtInfoIdx) -> bool {
    let Some(stmt) = self.env.module(module).and_then(|facts| facts.stmt(stmt_idx)) else {
      return true;
    };
    let site = Site { module, stmt: stmt_idx, in_function: false };
    Analyzer { env: &self.env, state: &mut self.state }.stmt_effects(stmt, Cx::removal(site))
  }

  /// Walk an included statement, and map its excluded parts to the entries of
  /// `referenced_symbols` and the import records that only those parts use.
  fn walk(&mut self, module: ModuleIdx, stmt_idx: StmtInfoIdx) -> StmtRecord {
    let env = &self.env;
    let facts = env.module(module).expect("a walked statement belongs to an analyzed module");
    let stmt = facts.stmt(stmt_idx).expect("a walked statement maps to the AST");
    let mut an = Analyzer { env, state: &mut self.state };
    let mut walker = Walker {
      an: &mut an,
      module,
      site: Site { module, stmt: stmt_idx, in_function: false },
      excluded: Vec::new(),
      rewrites: TreeShakingRewrites::default(),
    };
    walker.walk_top(stmt);
    let Walker { excluded, rewrites, .. } = walker;
    let references = &env.stmt_infos[module].get(stmt_idx).referenced_symbols;
    if excluded.is_empty() {
      return StmtRecord {
        kept: vec![true; references.len()],
        excluded_records: Vec::new(),
        excluded_imports: FxHashSet::default(),
        rewrites,
      };
    }
    let member_roots = references
      .iter()
      .filter_map(|reference| match reference {
        SymbolOrMemberExprRef::MemberExpr(member) => member.reference_id,
        SymbolOrMemberExprRef::Symbol(_) => None,
      })
      .collect::<FxHashSet<_>>();
    let collect = |parts: &[Excluded<'a>]| {
      let mut collector = RefCollector {
        scopes: facts.scopes,
        imports: &facts.module.imports,
        member_roots: &member_roots,
        refs: Refs::default(),
      };
      for part in parts {
        collector.collect(*part);
      }
      collector.refs
    };
    let excluded_refs = collect(&excluded);
    let all_refs = collect(&[Excluded::Stmt(stmt)]);
    // An entry goes only when every occurrence of it is in an excluded part. An entry that is not
    // an occurrence in the AST, such as one that the link stage added, stays.
    let kept = references
      .iter()
      .map(|reference| match reference {
        SymbolOrMemberExprRef::Symbol(symbol_ref) => {
          let count = |refs: &Refs| refs.symbols.get(&symbol_ref.symbol).copied().unwrap_or(0);
          symbol_ref.owner != module
            || count(&all_refs) == 0
            || count(&excluded_refs) < count(&all_refs)
        }
        SymbolOrMemberExprRef::MemberExpr(member) => {
          member.node_id == NodeId::DUMMY || !excluded_refs.members.contains(&member.node_id)
        }
      })
      .collect();
    let imports = &facts.module.imports;
    let kept_records = all_refs
      .imports
      .iter()
      .filter(|node| !excluded_refs.imports.contains(*node))
      .filter_map(|node| imports.get(node).copied())
      .collect::<FxHashSet<_>>();
    let mut excluded_records = excluded_refs
      .imports
      .iter()
      .filter_map(|node| imports.get(node).copied())
      .filter(|record_idx| !kept_records.contains(record_idx))
      .collect::<Vec<_>>();
    excluded_records.sort_unstable();
    excluded_records.dedup();
    StmtRecord { kept, excluded_records, excluded_imports: excluded_refs.imports, rewrites }
  }
}
