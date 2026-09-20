use arcstr::ArcStr;
use itertools::Itertools;
use oxc_str::CompactStr;
use rolldown_common::{Chunk, ChunkIdx, OutputFormat, SymbolRef};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  chunk_graph::ChunkGraph,
  stages::generate_stage::{GenerateStage, order_wrap_state::OrderWrapState},
  utils::{
    chunk::deconflict_chunk_symbols::{InlineDeconflictPlan, deconflict_chunk_symbols},
    external_import_interop::ChunkAssignments,
  },
};

impl GenerateStage<'_> {
  /// Deconflicts the records first, one after another, so a record can share the import binding
  /// names of the records it will be printed next to and avoid the rest. Returns the plan every
  /// carrier applies when the ordinary per-chunk deconflict runs afterwards.
  ///
  /// A record's factory contributes exactly its import bindings to the module scope of a file;
  /// everything else it declares lives inside the factory function. So a file must avoid a
  /// record's import names and the globals the record reads, and a record must avoid the same
  /// two sets for every file it ends up in and every record it is printed next to.
  pub(in crate::stages::generate_stage) fn deconflict_inline_records(
    &mut self,
    chunk_graph: &mut ChunkGraph,
    order_state: &OrderWrapState,
    order_live_symbols: &FxHashSet<SymbolRef>,
    format: OutputFormat,
    index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
  ) -> FxHashMap<ChunkIdx, InlineDeconflictPlan> {
    let records = self.inline_state.record_indices().collect_vec();
    if records.is_empty() {
      return FxHashMap::default();
    }
    let symbol_db = &self.link_output.symbol_db;
    let unresolved_references_of = |chunk: &Chunk| -> Vec<CompactStr> {
      chunk
        .modules
        .iter()
        .filter_map(|module_idx| self.link_output.symbol_db[*module_idx].as_ref())
        .flat_map(|db| db.ast_scopes.scoping().root_unresolved_references().keys())
        .map(|name| CompactStr::new(name))
        .collect()
    };
    let imported_canonical_refs = |chunk: &Chunk| -> FxHashSet<SymbolRef> {
      chunk
        .imports_from_other_chunks
        .values()
        .flatten()
        .map(|item| symbol_db.canonical_ref_for(item.import_ref))
        .collect()
    };

    let mut carriers_of: FxHashMap<ChunkIdx, Vec<ChunkIdx>> = FxHashMap::default();
    let mut co_carried: FxHashMap<ChunkIdx, FxIndexSet<ChunkIdx>> = FxHashMap::default();
    for carrier in self.inline_state.carriers() {
      let carried = self.inline_state.carried_by(carrier);
      for record in carried {
        carriers_of.entry(*record).or_default().push(carrier);
        co_carried
          .entry(*record)
          .or_default()
          .extend(carried.iter().copied().filter(|other| other != record));
      }
    }

    // Record -> the (symbol, name) of each of its import bindings, filled as records are done.
    let mut import_names: FxHashMap<ChunkIdx, Vec<(SymbolRef, CompactStr)>> = FxHashMap::default();
    for record in records {
      let mut plan = InlineDeconflictPlan::default();
      let own_imports = imported_canonical_refs(&chunk_graph.chunk_table[record]);
      for other in co_carried.get(&record).into_iter().flatten() {
        let Some(names) = import_names.get(other) else { continue };
        for (symbol_ref, name) in names {
          if own_imports.contains(symbol_ref) {
            plan.preassigned.push((*symbol_ref, name.clone()));
          } else {
            plan.reserved.push(name.clone());
          }
        }
      }
      for carrier in carriers_of.get(&record).into_iter().flatten() {
        plan.reserved.extend(unresolved_references_of(&chunk_graph.chunk_table[*carrier]));
        for other in self.inline_state.carried_by(*carrier) {
          if *other != record {
            plan.reserved.extend(unresolved_references_of(&chunk_graph.chunk_table[*other]));
          }
        }
      }
      plan.bridges = self.bridge_targets(record);

      let ChunkGraph { chunk_table, module_to_chunk, post_chunk_optimization_operations, .. } =
        &mut *chunk_graph;
      let chunk_assignments =
        ChunkAssignments::new(&*module_to_chunk, &*post_chunk_optimization_operations);
      let chunk = &mut chunk_table[record];
      let output = deconflict_chunk_symbols(
        record,
        chunk,
        self.link_output,
        order_state,
        order_live_symbols,
        format,
        index_chunk_id_to_name,
        chunk_assignments,
        &plan,
        true,
      );
      let mut names = Vec::new();
      let mut seen = FxHashSet::default();
      for item in chunk.imports_from_other_chunks.values().flatten() {
        let canonical_ref = symbol_db.canonical_ref_for(item.import_ref);
        if seen.insert(canonical_ref)
          && let Some(name) = chunk.canonical_names.get(&canonical_ref)
        {
          names.push((canonical_ref, name.clone()));
        }
      }
      import_names.insert(record, names);
      self.inline_state.set_bridge_names(record, output.bridge_names);
      self.inline_state.set_record_exports_param(
        record,
        output.exports_param.expect("a record deconflict names its exports parameter"),
      );
    }

    let mut plans = FxHashMap::default();
    for carrier in self.inline_state.carriers() {
      let mut plan = InlineDeconflictPlan::default();
      let own_imports = imported_canonical_refs(&chunk_graph.chunk_table[carrier]);
      for record in self.inline_state.carried_by(carrier) {
        for (symbol_ref, name) in &import_names[record] {
          if own_imports.contains(symbol_ref) {
            plan.preassigned.push((*symbol_ref, name.clone()));
          } else {
            plan.reserved.push(name.clone());
          }
        }
        plan.reserved.extend(unresolved_references_of(&chunk_graph.chunk_table[*record]));
      }
      plan.bridges = self.bridge_targets(carrier);
      plans.insert(carrier, plan);
    }
    plans
  }

  fn bridge_targets(&self, reader: ChunkIdx) -> Vec<(ChunkIdx, ArcStr)> {
    self
      .inline_state
      .readers_of(reader)
      .iter()
      .map(|record| {
        (*record, self.inline_state.record(*record).expect("reader targets a record").id.clone())
      })
      .collect()
  }
}
