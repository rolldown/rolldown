use std::convert::Infallible;

use oxc::span::Span;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, ImportKind, ImportRecordIdx, ModuleIdx, ModuleTable, NormalModule,
};
use rolldown_error::{BuildDiagnostic, ImportChainNote, RequireTla};
use rolldown_utils::{
  IndexBitSet,
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::ComputeTlaPass;

// See internal-docs/pass-based-pipeline/implementation.md for the pass contract and lifecycle.

#[derive(Debug, Default)]
pub(in crate::stages::link_stage) struct TlaScanFacts {
  tla_module_count: usize,
  tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
}

impl TlaScanFacts {
  pub(in crate::stages::link_stage) fn new(
    tla_module_count: usize,
    tla_keyword_span_map: FxHashMap<ModuleIdx, Span>,
  ) -> Self {
    Self { tla_module_count, tla_keyword_span_map }
  }
}

#[derive(Debug)]
pub(in crate::stages::link_stage) struct TlaFacts {
  module_count: usize,
  modules: IndexBitSet<ModuleIdx>,
}

impl TlaFacts {
  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.module_count
  }

  pub(in crate::stages::link_stage) fn modules(&self) -> impl Iterator<Item = ModuleIdx> + '_ {
    self.modules.index_of_one()
  }
}

#[derive(Clone, Copy)]
enum TlaVisitState {
  NotVisited,
  Visiting,
  /// `Some(module_idx)` = the module that actually contains the top-level await.
  /// `None` = no TLA in this module or its dependencies.
  Visited(Option<ModuleIdx>),
}

fn import_span_for(module: &NormalModule, target: ImportRecordIdx) -> Span {
  module.import_records.get(target).map_or(Span::empty(0), |rec| rec.importer_span)
}

// Known limitation: on a cycle hit we return `None` for the visiting edge and
// then memoize modules along the current DFS path even though a later branch of
// the same parent might still discover TLA through them. If a subsequent
// `require(...)` lookup lands on one of those prematurely memoized siblings we
// silently miss the error. A proper fix would delay memoization until the
// enclosing SCC resolves.
fn find_tla_source(
  module_idx: ModuleIdx,
  module_table: &ModuleTable,
  visited: &mut IndexVec<ModuleIdx, TlaVisitState>,
) -> Option<ModuleIdx> {
  match visited[module_idx] {
    TlaVisitState::Visited(result) => return result,
    TlaVisitState::Visiting => return None,
    TlaVisitState::NotVisited => {}
  }

  visited[module_idx] = TlaVisitState::Visiting;
  let module = &module_table[module_idx];
  let is_self_tla = module
    .as_normal()
    .is_some_and(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait));
  if is_self_tla {
    visited[module_idx] = TlaVisitState::Visited(Some(module_idx));
    return Some(module_idx);
  }

  let tla_source = module
    .import_records()
    .iter()
    .filter(|rec| std::matches!(rec.kind, ImportKind::Import))
    .find_map(|rec| {
      rec
        .resolved_module
        .and_then(|dep_idx| find_tla_source(module_table[dep_idx].idx(), module_table, visited))
    });

  visited[module_idx] = TlaVisitState::Visited(tla_source);
  tla_source
}

fn build_import_chain(
  start_idx: ModuleIdx,
  tla_source_idx: ModuleIdx,
  module_table: &ModuleTable,
  visited: &IndexVec<ModuleIdx, TlaVisitState>,
) -> Vec<ImportChainNote> {
  let mut chain = Vec::new();
  let mut seen: FxHashSet<ModuleIdx> = FxHashSet::default();
  let mut current_idx = start_idx;

  // `seen` guards against cycles: cycle back-edges are memoized as
  // `Some(tla_source_idx)` just like forward edges, so a naive first-match
  // would walk into the cycle. Prefer the direct edge to `tla_source_idx`
  // when one exists so the chain stays short.
  while current_idx != tla_source_idx && seen.insert(current_idx) {
    let module = &module_table[current_idx];
    let Some(normal) = module.as_normal() else {
      break;
    };

    let mut direct = None;
    let mut indirect = None;
    for (rec_idx, rec) in normal.import_records.iter_enumerated() {
      if !std::matches!(rec.kind, ImportKind::Import) {
        continue;
      }
      let Some(dep_idx) = rec.resolved_module else { continue };
      let dep_module_idx = module_table[dep_idx].idx();
      if seen.contains(&dep_module_idx) {
        continue;
      }
      if dep_module_idx == tla_source_idx {
        let importee = &module_table[dep_idx];
        direct = Some((dep_module_idx, import_span_for(normal, rec_idx), importee.stable_id()));
        break;
      }
      if indirect.is_none()
        && std::matches!(
          visited[dep_module_idx],
          TlaVisitState::Visited(Some(source)) if source == tla_source_idx
        )
      {
        let importee = &module_table[dep_idx];
        indirect = Some((dep_module_idx, import_span_for(normal, rec_idx), importee.stable_id()));
      }
    }

    let Some((next_idx, import_span, importee_stable_id)) = direct.or(indirect) else {
      break;
    };

    chain.push(ImportChainNote {
      importer_stable_id: module.stable_id().as_arc_str().clone(),
      importer_source: normal.source.clone(),
      importee_stable_id: importee_stable_id.as_arc_str().clone(),
      import_span,
    });
    current_idx = next_idx;
  }

  chain
}

impl Pass for ComputeTlaPass {
  type InputRead<'a> = &'a ModuleTable;
  type InputOwned = TlaScanFacts;
  type OutputRead = TlaFacts;
  type OutputOwned = ();
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    module_table: Self::InputRead<'_>,
    scan_facts: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let TlaScanFacts { tla_module_count, tla_keyword_span_map } = scan_facts;
    let module_count = module_table.modules.len();
    if tla_module_count == 0 {
      return Ok(token.finish(TlaFacts { module_count, modules: IndexBitSet::default() }, ()));
    }

    let mut visited = oxc_index::index_vec![TlaVisitState::NotVisited; module_count];
    let mut modules = IndexBitSet::new(module_count);

    module_table.modules.iter().filter_map(|module| module.as_normal()).for_each(|module| {
      let tla_source = find_tla_source(module.idx, module_table, &mut visited);
      if tla_source.is_some() {
        modules.set_bit(module.idx);
      }

      // Check for require() of TLA modules — this is forbidden.
      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        if !std::matches!(rec.kind, ImportKind::Require) {
          continue;
        }
        let Some(resolved_module_idx) = rec.resolved_module else { continue };
        let dep_idx = module_table[resolved_module_idx].idx();
        let Some(tla_source_idx) = find_tla_source(dep_idx, module_table, &mut visited) else {
          continue;
        };

        let require_span = import_span_for(module, rec_idx);
        let import_chain = build_import_chain(dep_idx, tla_source_idx, module_table, &visited);

        let tla_module = &module_table[tla_source_idx];
        // `find_tla_source` only returns modules whose `ast_usage` contains
        // `TopLevelAwait`, and the scanner always records a keyword span for
        // those modules, so this map lookup must hit.
        let tla_keyword_span = tla_keyword_span_map.get(&tla_source_idx).copied();
        std::debug_assert!(
          tla_keyword_span.is_some(),
          "tla_keyword_span missing for TLA source module {tla_source_idx:?}"
        );
        let tla_keyword_span = tla_keyword_span.unwrap_or(Span::empty(0));

        cx.push(BuildDiagnostic::require_tla(RequireTla {
          importer_stable_id: module.stable_id.as_arc_str().clone(),
          importer_source: module.source.clone(),
          require_span,
          tla_source_stable_id: tla_module.stable_id().as_arc_str().clone(),
          tla_source_text: tla_module.as_normal().map(|m| m.source.clone()).unwrap_or_default(),
          tla_keyword_span,
          import_chain,
        }));
      }
    });

    Ok(token.finish(TlaFacts { module_count, modules }, ()))
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::Span;
  use rolldown_common::{EcmaModuleAstUsage, ImportKind, ModuleIdx, ModuleTable};
  use rolldown_error::BuildDiagnostic;
  use rolldown_utils::pass::{PassPipelineCtx, Sealed, run_infallible_pass};
  use rustc_hash::FxHashMap;

  use super::super::test_utils::{module_idx, module_table, normal_module};
  use super::{ComputeTlaPass, TlaFacts, TlaScanFacts};

  fn scan_facts(table: &ModuleTable) -> TlaScanFacts {
    let mut count = 0;
    let mut spans = FxHashMap::default();
    for (idx, module) in table.modules.iter_enumerated() {
      if module
        .as_normal()
        .is_some_and(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait))
      {
        count += 1;
        spans.insert(idx, Span::new(1, 2));
      }
    }
    TlaScanFacts::new(count, spans)
  }

  fn run(
    table: &ModuleTable,
    scan_facts: TlaScanFacts,
  ) -> (Sealed<TlaFacts>, Vec<BuildDiagnostic>) {
    let mut pipeline = PassPipelineCtx::new();
    let (facts, ()) = run_infallible_pass(ComputeTlaPass, &mut pipeline, table, scan_facts);
    (facts, pipeline.into_diagnostics().into_iter().collect())
  }

  fn fact_modules(facts: &TlaFacts) -> Vec<ModuleIdx> {
    facts.modules().collect()
  }

  fn assert_sealed(_: &Sealed<TlaFacts>) {}

  #[test]
  fn zero_tla_fast_path_returns_an_empty_sealed_fact() {
    let table = module_table(vec![normal_module(
      0,
      false,
      vec![(ImportKind::Require, None, Span::new(10, 11))],
    )]);
    let (facts, diagnostics) = run(&table, TlaScanFacts::default());

    assert_sealed(&facts);
    assert_eq!(facts.module_count(), 1);
    assert!(fact_modules(&facts).is_empty());
    assert!(diagnostics.is_empty());
  }

  #[test]
  fn computes_transitive_tla_facts_without_following_require_edges() {
    let table = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Require, Some(1), Span::new(10, 11))]),
      normal_module(1, false, vec![(ImportKind::Import, Some(2), Span::new(20, 21))]),
      normal_module(2, true, Vec::new()),
    ]);
    let (facts, diagnostics) = run(&table, scan_facts(&table));

    assert_eq!(fact_modules(&facts), vec![module_idx(1), module_idx(2)]);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].to_string().contains("transitive dependency \"m2.js\""));
    let rendered = diagnostics[0].to_diagnostic();
    assert_eq!(rendered.get_primary_location(), Some(("m0.js".to_string(), 1, 10, 10)));
    assert!(rendered.convert_to_string(false).contains("m1.js\" imports the file \"m2.js"));
  }

  #[test]
  fn preserves_module_and_import_record_diagnostic_order() {
    let table = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Require, Some(2), Span::new(10, 11)),
          (ImportKind::Require, Some(3), Span::new(12, 13)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Require, Some(3), Span::new(14, 15))]),
      normal_module(2, true, Vec::new()),
      normal_module(3, true, Vec::new()),
    ]);
    let (facts, diagnostics) = run(&table, scan_facts(&table));

    assert_eq!(fact_modules(&facts), vec![module_idx(2), module_idx(3)]);
    let messages = diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(messages.len(), 3);
    assert!(messages[0].contains("m2.js"));
    assert!(messages[1].contains("m3.js"));
    assert!(messages[2].contains("m3.js"));
    let locations = diagnostics
      .iter()
      .map(|diagnostic| diagnostic.to_diagnostic().get_primary_location())
      .collect::<Vec<_>>();
    assert_eq!(
      locations,
      vec![
        Some(("m0.js".to_string(), 1, 10, 10)),
        Some(("m0.js".to_string(), 1, 12, 12)),
        Some(("m1.js".to_string(), 1, 14, 14)),
      ]
    );
  }

  #[test]
  fn preserves_the_known_cycle_memoization_limitation() {
    let table = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Import, Some(1), Span::new(10, 11)),
          (ImportKind::Import, Some(2), Span::new(12, 13)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Import, Some(0), Span::new(20, 21))]),
      normal_module(2, true, Vec::new()),
      normal_module(3, false, vec![(ImportKind::Require, Some(1), Span::new(30, 31))]),
    ]);
    let (facts, diagnostics) = run(&table, scan_facts(&table));

    // Module 1 can reach module 2 through module 0, but the current DFS memoizes
    // the cycle sibling as `None`. Phase 2 deliberately preserves that bug.
    assert_eq!(fact_modules(&facts), vec![module_idx(0), module_idx(2)]);
    assert!(diagnostics.is_empty());
  }
}
