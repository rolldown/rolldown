use itertools::Itertools;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  ChunkIdx, EcmaViewMeta, ExportsKind, ImportKind, ImportRecordIdx, Module, ModuleIdx,
  SymbolOrMemberExprRef, SymbolRef, UsedSymbolRefsBuilder, WrapKind,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

#[derive(Debug)]
pub(super) struct OrderAnalysis {
  pub(super) roots: Vec<RootOrderAnalysis>,
  pub(super) plan: OrderWrapPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OrderWrapReason {
  /// The module is missing from one order or executes before an expected predecessor.
  DirectViolation,
  /// V1 lowering moves all later sensitive modules behind the same init boundary.
  SensitiveSuffix,
  /// The module statically imports another module selected by the plan.
  StaticImporter,
  /// A top-level statement reads an export owned by a selected module.
  TopLevelReader,
}

impl OrderWrapReason {
  pub(super) fn as_static_str(self) -> &'static str {
    match self {
      Self::DirectViolation => "direct-violation",
      Self::SensitiveSuffix => "sensitive-suffix",
      Self::StaticImporter => "static-importer",
      Self::TopLevelReader => "top-level-reader",
    }
  }
}

#[derive(Debug, Default)]
pub(super) struct OrderWrapPlan {
  reasons_by_module: FxHashMap<ModuleIdx, Vec<OrderWrapReason>>,
}

impl OrderWrapPlan {
  fn insert(&mut self, module_idx: ModuleIdx, reason: OrderWrapReason) -> bool {
    let reasons = self.reasons_by_module.entry(module_idx).or_default();
    let is_new_module = reasons.is_empty();
    if !reasons.contains(&reason) {
      reasons.push(reason);
    }
    is_new_module
  }

  pub(super) fn contains(&self, module_idx: &ModuleIdx) -> bool {
    self.reasons_by_module.contains_key(module_idx)
  }

  pub(super) fn is_empty(&self) -> bool {
    self.reasons_by_module.is_empty()
  }

  pub(super) fn len(&self) -> usize {
    self.reasons_by_module.len()
  }

  pub(super) fn modules(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.reasons_by_module.keys().copied()
  }

  pub(super) fn reasons(&self, module_idx: ModuleIdx) -> &[OrderWrapReason] {
    self.reasons_by_module.get(&module_idx).map(Vec::as_slice).unwrap_or_default()
  }
}

#[derive(Debug)]
pub(super) struct RootOrderAnalysis {
  pub(super) root: ModuleIdx,
  pub(super) expected_order: Vec<ModuleIdx>,
  pub(super) actual_order: Vec<ModuleIdx>,
  pub(super) at_risk: FxHashSet<ModuleIdx>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
  Unvisited,
  Visiting,
  Done,
}

impl GenerateStage<'_> {
  pub(super) fn analyze_execution_order(
    &mut self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &UsedSymbolRefsBuilder,
  ) -> Option<OrderAnalysis> {
    if !self.options.is_strict_execution_order_enabled() {
      return None;
    }

    let import_edges = self.predicted_static_import_edges(chunk_graph, used_symbol_refs);
    let mut all_at_risk = FxHashSet::default();
    let mut roots = Vec::new();

    for &root in self.link_output.entries.keys() {
      if !self.link_output.metas[root].is_included {
        continue;
      }
      let Some(&root_chunk) = chunk_graph.entry_module_to_entry_chunk.get(&root) else {
        continue;
      };

      let expected_order = self.expected_order_for_root(root);
      let actual_order = self.actual_order_for_root(root, root_chunk, chunk_graph, &import_edges);
      let at_risk = self.at_risk_modules(&expected_order, &actual_order);
      all_at_risk.extend(at_risk.iter().copied());
      roots.push(RootOrderAnalysis { root, expected_order, actual_order, at_risk });
    }

    let plan = self.build_order_wrap_plan(all_at_risk, &roots);
    let analysis = OrderAnalysis { roots, plan };
    analysis.log_summary(self);
    Some(analysis)
  }

  #[cfg(test)]
  #[expect(dead_code)]
  fn analyze_execution_order_for_test(
    &mut self,
    chunk_graph: &ChunkGraph,
    used_symbol_refs: &UsedSymbolRefsBuilder,
  ) -> Option<OrderAnalysis> {
    self.analyze_execution_order(chunk_graph, used_symbol_refs)
  }

  fn expected_order_for_root(&self, root: ModuleIdx) -> Vec<ModuleIdx> {
    let mut states = index_vec![VisitState::Unvisited; self.link_output.module_table.modules.len()];
    let mut order = Vec::new();

    if !self.link_output.module_table[root].is_normal() {
      return order;
    }

    states[root] = VisitState::Visiting;
    let mut stack = vec![(root, 0usize)];
    while let Some((module_idx, next_import_idx)) = stack.last_mut() {
      let Some(module) = self.link_output.module_table[*module_idx].as_normal() else {
        states[*module_idx] = VisitState::Done;
        stack.pop();
        continue;
      };

      let mut descended = false;
      while *next_import_idx < module.import_records.len() {
        let rec_idx =
          ImportRecordIdx::from_raw(u32::try_from(*next_import_idx).expect("import index fits"));
        let rec = &module.import_records[rec_idx];
        *next_import_idx += 1;

        if rec.kind != ImportKind::Import {
          continue;
        }
        let Some(importee_idx) = rec.resolved_module else { continue };
        if !self.link_output.module_table[importee_idx].is_normal() {
          continue;
        }

        match states[importee_idx] {
          VisitState::Unvisited => {
            states[importee_idx] = VisitState::Visiting;
            stack.push((importee_idx, 0));
            descended = true;
            break;
          }
          VisitState::Visiting | VisitState::Done => {}
        }
      }

      if descended {
        continue;
      }

      let (done_module_idx, _) = stack.pop().expect("stack has a last item");
      if states[done_module_idx] != VisitState::Done {
        states[done_module_idx] = VisitState::Done;
        if self.link_output.metas[done_module_idx].is_included {
          order.push(done_module_idx);
        }
      }
    }

    order
  }

  fn actual_order_for_root(
    &self,
    root: ModuleIdx,
    root_chunk: ChunkIdx,
    chunk_graph: &ChunkGraph,
    import_edges: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
  ) -> Vec<ModuleIdx> {
    let mut chunk_states = index_vec![VisitState::Unvisited; chunk_graph.chunk_table.len()];
    let mut module_states =
      index_vec![VisitState::Unvisited; self.link_output.module_table.modules.len()];
    let mut order = Vec::new();

    self.visit_actual_chunk(
      root_chunk,
      chunk_graph,
      import_edges,
      &mut chunk_states,
      &mut module_states,
      &mut order,
    );
    if !self.link_output.metas[root].original_wrap_kind().is_none() {
      self.execute_actual_module(root, &mut module_states, &mut order);
    }

    order
  }

  fn visit_actual_chunk(
    &self,
    chunk_idx: ChunkIdx,
    chunk_graph: &ChunkGraph,
    import_edges: &IndexVec<ChunkIdx, FxHashSet<ChunkIdx>>,
    chunk_states: &mut IndexVec<ChunkIdx, VisitState>,
    module_states: &mut IndexVec<ModuleIdx, VisitState>,
    order: &mut Vec<ModuleIdx>,
  ) {
    match chunk_states[chunk_idx] {
      VisitState::Done | VisitState::Visiting => return,
      VisitState::Unvisited => {}
    }
    chunk_states[chunk_idx] = VisitState::Visiting;

    let mut imports = import_edges[chunk_idx].iter().copied().collect_vec();
    imports
      .sort_unstable_by_key(|importee_chunk| chunk_graph.chunk_table[*importee_chunk].exec_order);
    for importee_chunk in imports {
      self.visit_actual_chunk(
        importee_chunk,
        chunk_graph,
        import_edges,
        chunk_states,
        module_states,
        order,
      );
    }

    for &module_idx in &chunk_graph.chunk_table[chunk_idx].modules {
      if self.link_output.metas[module_idx].original_wrap_kind().is_none() {
        self.execute_actual_module(module_idx, module_states, order);
      }
    }

    chunk_states[chunk_idx] = VisitState::Done;
  }

  fn execute_actual_module(
    &self,
    module_idx: ModuleIdx,
    module_states: &mut IndexVec<ModuleIdx, VisitState>,
    order: &mut Vec<ModuleIdx>,
  ) {
    match module_states[module_idx] {
      VisitState::Done | VisitState::Visiting => return,
      VisitState::Unvisited => {}
    }
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return;
    };
    if !self.link_output.metas[module_idx].is_included {
      return;
    }

    module_states[module_idx] = VisitState::Visiting;
    for rec in &module.import_records {
      if !matches!(rec.kind, ImportKind::Import | ImportKind::Require) {
        continue;
      }
      let Some(importee_idx) = rec.resolved_module else { continue };
      if self.link_output.metas[importee_idx].original_wrap_kind().is_none() {
        continue;
      }
      self.execute_actual_module(importee_idx, module_states, order);
    }
    module_states[module_idx] = VisitState::Done;
    order.push(module_idx);
  }

  fn at_risk_modules(
    &self,
    expected_order: &[ModuleIdx],
    actual_order: &[ModuleIdx],
  ) -> FxHashSet<ModuleIdx> {
    let expected_sensitive_order = self.sensitive_order(expected_order);
    let actual_sensitive_order = self.sensitive_order(actual_order);
    let expected_sensitive_set = expected_sensitive_order.iter().copied().collect::<FxHashSet<_>>();
    let actual_sensitive_set = actual_sensitive_order.iter().copied().collect::<FxHashSet<_>>();
    let actual_positions = actual_sensitive_order
      .iter()
      .copied()
      .enumerate()
      .map(|(position, module_idx)| (module_idx, position))
      .collect::<FxHashMap<_, _>>();
    let mut at_risk = FxHashSet::default();

    for module_idx in expected_sensitive_set.symmetric_difference(&actual_sensitive_set) {
      if self.is_order_wrap_eligible(*module_idx) {
        at_risk.insert(*module_idx);
      }
    }

    // `symmetric_difference` intentionally covers BOTH directions.
    // `actual ∖ expected` is the true phantom (runs under a root the source never reaches).
    // `expected ∖ actual` is NOT always empty either, and the reason is a mismatch between two
    // notions of "side effect": actual-order reachability is built from tree-shaking side effects
    // (`add_side_effect_imports_for_module` skips importees whose `side_effects().has_side_effects()`
    // is false), while sensitivity here uses the ordering notion. A module that is order-sensitive
    // but tree-shaking-side-effect-free — e.g. a `/*#__PURE__*/` call that actually writes a global
    // — therefore gets no bare side-effect chunk edge, so it is unreachable in the predicted actual
    // order under a root that imports it only for that (tree-shaking-absent) side effect, yet it is
    // in `expected`. Catching it here over-wraps it; it then runs correctly via the init chain.
    // See `strip_plain_chunk_imports` (common.js writes globalThis.value under a pure annotation;
    // "missing" under page-b, wrapped, runs via init_common).
    for module_idx in premature_sensitive_modules(&expected_sensitive_order, &actual_positions) {
      if self.is_order_wrap_eligible(module_idx) {
        at_risk.insert(module_idx);
      }
    }

    at_risk
  }

  fn sensitive_order(&self, order: &[ModuleIdx]) -> Vec<ModuleIdx> {
    let mut seen = FxHashSet::default();
    let mut sensitive_order = Vec::new();
    for module_idx in order.iter().copied().filter(|idx| self.is_order_sensitive(*idx)) {
      if seen.insert(module_idx) {
        sensitive_order.push(module_idx);
      }
    }
    sensitive_order
  }

  fn build_order_wrap_plan(
    &self,
    at_risk: FxHashSet<ModuleIdx>,
    roots: &[RootOrderAnalysis],
  ) -> OrderWrapPlan {
    let source_reachable = self.source_reachable_modules(roots);
    let mut plan = OrderWrapPlan::default();
    for module_idx in
      at_risk.into_iter().filter(|module_idx| self.is_order_wrap_eligible(*module_idx))
    {
      plan.insert(module_idx, OrderWrapReason::DirectViolation);
    }

    loop {
      let mut changed = false;
      changed |= self.close_expected_sensitive_suffixes(roots, &mut plan);

      let current = plan.modules().collect::<FxHashSet<_>>();

      for module in self.link_output.module_table.modules.iter().filter_map(Module::as_normal) {
        if !source_reachable.contains(&module.idx)
          || !self.is_order_wrap_closure_eligible(module.idx)
          || plan.contains(&module.idx)
        {
          continue;
        }
        changed |= record_order_wrap_closure_reasons(
          &mut plan,
          module.idx,
          self.statically_imports_wrapped_member(module.idx, &current),
          self.top_level_reads_wrapped_export(module.idx, &current),
        );
      }

      if !changed {
        break;
      }
    }

    // The fixed point only needs to revisit modules that are not yet selected. Once membership is
    // stable, record any additional reasons that also apply to existing members without extending
    // the convergence loop.
    let final_members = plan.modules().collect::<FxHashSet<_>>();
    for module_idx in plan.modules().collect_vec() {
      record_order_wrap_closure_reasons(
        &mut plan,
        module_idx,
        self.statically_imports_wrapped_member(module_idx, &final_members),
        self.top_level_reads_wrapped_export(module_idx, &final_members),
      );
    }

    plan
  }

  fn close_expected_sensitive_suffixes(
    &self,
    roots: &[RootOrderAnalysis],
    plan: &mut OrderWrapPlan,
  ) -> bool {
    let mut changed = false;

    for root in roots {
      let expected_sensitive_order = self.sensitive_order(&root.expected_order);
      let Some(first_wrapped_idx) =
        expected_sensitive_order.iter().position(|module_idx| plan.contains(module_idx))
      else {
        continue;
      };

      // V1 init calls run after the eager chunk body. Once a root wraps an earlier sensitive
      // module, every later eager sensitive module for that root must move behind the same init
      // boundary.
      for module_idx in expected_sensitive_order[first_wrapped_idx..].iter().copied() {
        if self.is_order_wrap_closure_eligible(module_idx) {
          changed |= plan.insert(module_idx, OrderWrapReason::SensitiveSuffix);
        }
      }
    }

    changed
  }

  fn source_reachable_modules(&self, roots: &[RootOrderAnalysis]) -> FxHashSet<ModuleIdx> {
    let mut reachable = FxHashSet::default();
    let mut stack = roots.iter().map(|root| root.root).collect_vec();
    while let Some(module_idx) = stack.pop() {
      if !reachable.insert(module_idx) {
        continue;
      }
      let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
        continue;
      };
      for rec in module.import_records.iter().rev() {
        if rec.kind != ImportKind::Import {
          continue;
        }
        let Some(importee_idx) = rec.resolved_module else { continue };
        if self.link_output.module_table[importee_idx].is_normal() {
          stack.push(importee_idx);
        }
      }
    }
    reachable
  }

  fn statically_imports_wrapped_member(
    &self,
    module_idx: ModuleIdx,
    current: &FxHashSet<ModuleIdx>,
  ) -> bool {
    self.link_output.module_table[module_idx].as_normal().is_some_and(|module| {
      module.import_records.iter().any(|rec| {
        rec.kind == ImportKind::Import
          && rec.resolved_module.is_some_and(|importee_idx| current.contains(&importee_idx))
      })
    })
  }

  fn top_level_reads_wrapped_export(
    &self,
    module_idx: ModuleIdx,
    current: &FxHashSet<ModuleIdx>,
  ) -> bool {
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    let meta = &self.link_output.metas[module_idx];
    if !meta.is_included || !module.meta.contains(EcmaViewMeta::TopLevelImportRead) {
      return false;
    }
    let stmt_infos = &self.link_output.stmt_infos[module_idx];
    stmt_infos.iter_enumerated_without_namespace_stmt().any(|(_, stmt_info)| {
      stmt_info.referenced_symbols.iter().any(|reference_ref| {
        self.reference_touches_wrapped_export(module_idx, reference_ref, current)
      })
    })
  }

  fn reference_touches_wrapped_export(
    &self,
    module_idx: ModuleIdx,
    reference_ref: &SymbolOrMemberExprRef,
    current: &FxHashSet<ModuleIdx>,
  ) -> bool {
    match reference_ref {
      SymbolOrMemberExprRef::Symbol(symbol_ref) => {
        self.symbol_touches_wrapped_export(*symbol_ref, current)
      }
      SymbolOrMemberExprRef::MemberExpr(member_expr_ref) => member_expr_ref
        .resolution(&self.link_output.metas[module_idx].resolved_member_expr_refs)
        .map_or_else(
          || self.symbol_touches_wrapped_export(member_expr_ref.object_ref, current),
          |resolution| {
            resolution
              .resolved
              .is_some_and(|symbol_ref| self.symbol_touches_wrapped_export(symbol_ref, current))
              || resolution
                .depended_refs
                .iter()
                .any(|symbol_ref| self.symbol_touches_wrapped_export(*symbol_ref, current))
          },
        ),
    }
  }

  fn symbol_touches_wrapped_export(
    &self,
    symbol_ref: SymbolRef,
    current: &FxHashSet<ModuleIdx>,
  ) -> bool {
    let canonical_ref = self.link_output.symbol_db.canonical_ref_for(symbol_ref);
    current.contains(&symbol_ref.owner)
      || current.contains(&canonical_ref.owner)
      || self
        .link_output
        .normal_symbol_exports_chain_map
        .get(&symbol_ref)
        .is_some_and(|refs| refs.iter().any(|ref_| current.contains(&ref_.owner)))
      || self
        .link_output
        .normal_symbol_exports_chain_map
        .get(&canonical_ref)
        .is_some_and(|refs| refs.iter().any(|ref_| current.contains(&ref_.owner)))
  }

  fn is_order_sensitive(&self, module_idx: ModuleIdx) -> bool {
    if module_idx == self.link_output.runtime.id() {
      return false;
    }
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    let meta = &self.link_output.metas[module.idx];
    if !meta.is_included {
      return false;
    }

    let has_intrinsic_effect = module.meta.contains(EcmaViewMeta::ExecutionOrderSensitive);

    has_intrinsic_effect || self.eagerly_triggers_interop_module(module_idx)
  }

  /// An included top-level `import` of an interop-wrapped importee (a CJS module, or an ESM module
  /// reached via `require`) lowers to an eager `require_*()` / `__toESM(require_*())` call inside
  /// *this* module's own body. The trigger is order-sensitive even when tree shaking classifies the
  /// importee as side-effect-free: a retained import can still compute an observed export value.
  ///
  /// The interop wrapper controls *how* the importee is represented, not *when* it runs: its trigger
  /// stays inline in the importer's chunk body, so code splitting can displace it. The importee
  /// itself is not order-wrap-eligible (it is already interop-wrapped), so the only way to defer its
  /// trigger is to order-wrap the carrier hosting it. Marking the carrier order-sensitive lets the
  /// existing premature / suffix / importer closure decide that wrap; without it the carrier is
  /// invisible and a displaced interop evaluation goes unwrapped.
  fn eagerly_triggers_interop_module(&self, module_idx: ModuleIdx) -> bool {
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    let meta = &self.link_output.metas[module_idx];
    if !meta.is_included {
      return false;
    }
    self.link_output.stmt_infos[module_idx].iter_enumerated_without_namespace_stmt().any(
      |(stmt_info_idx, stmt_info)| {
        meta.stmt_info_included.has_bit(stmt_info_idx)
          && stmt_info.import_records.iter().any(|rec_idx| {
            let rec = &module.import_records[*rec_idx];
            // Only static `import` records run eagerly at the importer's position; a `require`
            // inside a function body is call-time and must not be treated as a top-level trigger.
            rec.kind == ImportKind::Import
              && rec.resolved_module.is_some_and(|importee_idx| {
                self.link_output.module_table[importee_idx].as_normal().is_some_and(|_| {
                  !matches!(
                    self.link_output.metas[importee_idx].original_wrap_kind(),
                    WrapKind::None
                  )
                })
              })
          })
      },
    )
  }

  fn is_order_wrap_eligible(&self, module_idx: ModuleIdx) -> bool {
    if module_idx == self.link_output.runtime.id() {
      return false;
    }
    if self.link_output.module_table[module_idx].as_normal().is_none() {
      return false;
    }
    self.link_output.metas[module_idx].is_included
      && self.is_order_wrap_closure_eligible(module_idx)
  }

  fn is_order_wrap_closure_eligible(&self, module_idx: ModuleIdx) -> bool {
    if module_idx == self.link_output.runtime.id() {
      return false;
    }
    if !self.link_output.metas[module_idx].is_included {
      return false;
    }
    let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
      return false;
    };
    matches!(module.exports_kind, ExportsKind::Esm | ExportsKind::None)
      && matches!(self.link_output.metas[module_idx].original_wrap_kind(), WrapKind::None)
  }
}

fn premature_sensitive_modules(
  expected_sensitive_order: &[ModuleIdx],
  actual_positions: &FxHashMap<ModuleIdx, usize>,
) -> FxHashSet<ModuleIdx> {
  let mut premature_modules = FxHashSet::default();
  let mut latest_predecessor_actual_position = None::<usize>;

  for module_idx in expected_sensitive_order {
    let Some(&actual_position) = actual_positions.get(module_idx) else {
      continue;
    };

    if latest_predecessor_actual_position.is_some_and(|latest| actual_position < latest) {
      premature_modules.insert(*module_idx);
    }
    latest_predecessor_actual_position = Some(
      latest_predecessor_actual_position
        .map_or(actual_position, |latest| latest.max(actual_position)),
    );
  }

  premature_modules
}

fn record_order_wrap_closure_reasons(
  plan: &mut OrderWrapPlan,
  module_idx: ModuleIdx,
  statically_imports_wrapped_member: bool,
  top_level_reads_wrapped_export: bool,
) -> bool {
  let mut changed = false;
  if statically_imports_wrapped_member {
    changed |= plan.insert(module_idx, OrderWrapReason::StaticImporter);
  }
  if top_level_reads_wrapped_export {
    changed |= plan.insert(module_idx, OrderWrapReason::TopLevelReader);
  }
  changed
}

impl OrderAnalysis {
  fn log_summary(&self, stage: &GenerateStage<'_>) {
    let root_violation_counts = self
      .roots
      .iter()
      .map(|root| {
        (
          stage.link_output.module_table[root.root].id().to_string(),
          root.at_risk.len(),
          root.expected_order.len(),
          root.actual_order.len(),
        )
      })
      .collect_vec();
    let order_wrap_members = self
      .plan
      .modules()
      .sorted_unstable_by_key(|idx| stage.link_output.module_table[*idx].exec_order())
      .map(|idx| {
        (stage.link_output.module_table[idx].id().to_string(), self.plan.reasons(idx).to_vec())
      })
      .collect_vec();

    tracing::debug!(
      target: "rolldown::order_analysis",
      root_count = self.roots.len(),
      order_wrap_count = self.plan.len(),
      root_violation_counts = ?root_violation_counts,
      order_wrap_members = ?order_wrap_members,
      "strict execution order analysis"
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn m(index: usize) -> ModuleIdx {
    ModuleIdx::new(index)
  }

  #[test]
  fn premature_sensitive_modules_marks_modules_that_run_before_expected_predecessors() {
    let expected = vec![m(0), m(1), m(2), m(3)];
    let actual_positions = [(m(0), 2), (m(1), 0), (m(2), 1), (m(3), 3)].into_iter().collect();

    assert_eq!(
      premature_sensitive_modules(&expected, &actual_positions),
      [m(1), m(2)].into_iter().collect()
    );
  }

  #[test]
  fn premature_sensitive_modules_ignores_stable_order() {
    let expected = vec![m(0), m(1), m(2)];
    let actual_positions = [(m(0), 0), (m(1), 1), (m(2), 2)].into_iter().collect();

    assert!(premature_sensitive_modules(&expected, &actual_positions).is_empty());
  }

  #[test]
  fn order_wrap_plan_preserves_all_reasons_for_a_module() {
    let mut plan = OrderWrapPlan::default();

    assert!(plan.insert(m(0), OrderWrapReason::DirectViolation));
    assert!(!plan.insert(m(0), OrderWrapReason::DirectViolation));
    assert!(!plan.insert(m(0), OrderWrapReason::SensitiveSuffix));

    assert_eq!(
      plan.reasons(m(0)),
      &[OrderWrapReason::DirectViolation, OrderWrapReason::SensitiveSuffix]
    );
  }

  #[test]
  fn closure_reasons_are_recorded_for_an_already_selected_module() {
    let mut plan = OrderWrapPlan::default();
    plan.insert(m(0), OrderWrapReason::DirectViolation);

    assert!(!record_order_wrap_closure_reasons(&mut plan, m(0), true, true));
    assert_eq!(
      plan.reasons(m(0)),
      &[
        OrderWrapReason::DirectViolation,
        OrderWrapReason::StaticImporter,
        OrderWrapReason::TopLevelReader,
      ]
    );
  }
}
