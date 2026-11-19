use oxc_index::IndexVec;
use oxc_str::CompactStr;
use rolldown_common::{
  Chunk, ChunkIdx, InstantiatedChunk, ModuleIdx, ModuleRenderOutput, NamespaceAlias,
  NormalizedBundlerOptions, OutputExports, PathsOutputOption, SymbolRef, UsedSymbolRefs,
};
use rolldown_error::BuildDiagnostic;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::{ecmascript::property_access_str, indexmap::FxIndexMap};
use rustc_hash::FxHashMap;

use crate::{
  chunk_graph::ChunkGraph,
  stages::{
    generate_stage::{
      CarriedRender, InlineCommonChunksState, InlineReader, SharedCommonJsReads,
      order_wrap_state::{EsmInitTarget, OrderWrapState},
    },
    link_stage::LinkStageOutput,
  },
};

pub struct GenerateContext<'a> {
  pub chunk_idx: ChunkIdx,
  pub chunk: &'a Chunk,
  pub options: &'a NormalizedBundlerOptions,
  pub link_output: &'a LinkStageOutput,
  /// Sealed record of inclusion-fixpoint liveness; see [`UsedSymbolRefs`].
  pub used_symbol_refs: &'a UsedSymbolRefs,
  pub order_wrap_state: &'a OrderWrapState,
  pub chunk_graph: &'a ChunkGraph,
  pub plugin_driver: &'a SharedPluginDriver,
  pub module_id_to_codegen_ret: Vec<Option<ModuleRenderOutput>>,
  /// The key of the map is exported item symbol,
  /// the value of the map is optional alias. e.g.
  /// - chunkB.js
  /// ```js
  /// export const a = 10000000;
  /// export {a as b}; // symbol_ref points to `a`, and alias is `b`
  /// ```
  pub render_export_items_index_vec: &'a IndexVec<ChunkIdx, FxIndexMap<SymbolRef, Vec<CompactStr>>>,
  /// Pre-resolved paths for external modules (always a `FxHashMap` variant).
  /// Used instead of `options.paths` in sync rendering code to avoid deadlocks.
  pub resolved_paths: Option<&'a PathsOutputOption>,
  /// `experimentalInlineCommonChunks` decisions: which chunks are records, who carries and reads
  /// them, and the bridge names.
  pub inline_state: &'a InlineCommonChunksState,
  /// The shared reads of CommonJS properties: the functions that each chunk declares, and the
  /// function that each export calls in place of the property read.
  pub shared_commonjs_reads: &'a SharedCommonJsReads,
  /// The records this file carries, each finalized and printed for this file, dependencies first.
  pub carried_renders: Vec<CarriedRender>,
}

impl GenerateContext<'_> {
  pub fn esm_init_target(&self, module_idx: ModuleIdx) -> Option<EsmInitTarget> {
    self.order_wrap_state.esm_init_target(module_idx, &self.link_output.metas[module_idx])
  }

  /// A `SymbolRef` might be identifier or a property access. This function will return correct string pattern for the symbol.
  pub fn finalized_string_pattern_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    cur_chunk_idx: ChunkIdx,
    canonical_names: &FxHashMap<SymbolRef, CompactStr>,
  ) -> String {
    let symbol_db = &self.link_output.symbol_db;
    if !symbol_ref.is_declared_in_root_scope(symbol_db) {
      // No fancy things on none root scope symbols
      return self.canonical_name_for(canonical_names, symbol_ref).to_string();
    }

    let canonical_ref = symbol_db.canonical_ref_for(symbol_ref);
    let canonical_symbol = symbol_db.get(canonical_ref);
    let namespace_alias = &canonical_symbol.namespace_alias;
    if let Some(ns_alias) = namespace_alias {
      let namespace = self.finalized_namespace_pattern(ns_alias, cur_chunk_idx, canonical_names);
      return property_access_str(&namespace, &ns_alias.property_name);
    }

    if self.link_output.module_table[canonical_ref.owner].is_external() {
      let namespace = symbol_db.canonical_name_for_or_original(canonical_ref, canonical_names);
      return namespace.to_string();
    }

    if let Some(bridged) = self.inline_bridge_pattern(canonical_ref, cur_chunk_idx) {
      return bridged;
    }

    match self.options.format {
      rolldown_common::OutputFormat::Cjs => {
        let chunk_idx_of_canonical_symbol = canonical_symbol.chunk_idx.unwrap_or_else(|| {
          // Scoped symbols don't get assigned a `ChunkIdx`. There are skipped for performance reason, because they are surely
          // belong to the chunk they are declared in and won't link to other chunks.
          let symbol_name = canonical_ref.name(symbol_db);
          let module_id = self.link_output.module_table[canonical_ref.owner].stable_id();
          panic!(
            "Symbol `{symbol_name}` in module `{module_id}` is not in any chunk, which is unexpected"
          );
        });

        let is_symbol_in_other_chunk = cur_chunk_idx != chunk_idx_of_canonical_symbol;
        if is_symbol_in_other_chunk {
          // In cjs output, we need convert the `import { foo } from 'foo'; console.log(foo);`;
          // If `foo` is split into another chunk, we need to convert the code `console.log(foo);` to `console.log(require_xxxx.foo);`
          // instead of keeping `console.log(foo)` as we did in esm output. The reason here is we need to keep live binding in cjs output.
          self.cross_chunk_pattern(canonical_ref, chunk_idx_of_canonical_symbol, cur_chunk_idx)
        } else {
          self.canonical_name_for(canonical_names, canonical_ref).to_string()
        }
      }
      _ => self.canonical_name_for(canonical_names, canonical_ref).to_string(),
    }
  }

  /// The binding `binding_ref` that the chunk `cur_chunk_idx` imports from another chunk for a
  /// shared read (`SharedCommonJsReads::binding`). The pattern reads the binding itself, not the
  /// property of its namespace alias: CJS output reads the export through the `require()` binding
  /// of the chunk, and ESM output uses the name of the import.
  pub fn finalized_binding_pattern(
    &self,
    binding_ref: SymbolRef,
    cur_chunk_idx: ChunkIdx,
    canonical_names: &FxHashMap<SymbolRef, CompactStr>,
  ) -> String {
    let chunk_idx = self
      .link_output
      .symbol_db
      .get(binding_ref)
      .chunk_idx
      .expect("the link pass gives the binding of a shared read to a chunk");
    if matches!(self.options.format, rolldown_common::OutputFormat::Cjs)
      && chunk_idx != cur_chunk_idx
    {
      self.cross_chunk_pattern(binding_ref, chunk_idx, cur_chunk_idx)
    } else {
      self.canonical_name_for(canonical_names, binding_ref).to_string()
    }
  }

  /// The namespace that `ns_alias` reads its property from, as the chunk `cur_chunk_idx` renders it.
  pub fn finalized_namespace_pattern(
    &self,
    ns_alias: &NamespaceAlias,
    cur_chunk_idx: ChunkIdx,
    canonical_names: &FxHashMap<SymbolRef, CompactStr>,
  ) -> String {
    // The namespace itself may be declared in another chunk. Resolve the namespace through the
    // same cross-chunk path as an ordinary symbol before appending the property; otherwise CJS
    // output renders a bare, undeclared local identifier. These are examples:
    // - An entry facade chunk has no modules. Thus the namespace of a re-export from a CommonJS
    //   module stays in the chunk of the entry module (issue #11767).
    // - Generated per-record interop carriers place the namespace beside the CJS importee, while
    //   its re-export facade stays owned by the barrel.
    // - An inline common chunk record owns the namespace.
    let namespace_ref = ns_alias.namespace_ref;
    if self.order_wrap_state.is_order_cjs_carrier_namespace(namespace_ref)
      || self.inline_bridge_pattern(namespace_ref, cur_chunk_idx).is_some()
    {
      self.finalized_string_pattern_for_symbol_ref(namespace_ref, cur_chunk_idx, canonical_names)
    } else if let Some(exporter_chunk_idx) =
      self.chunk_that_exports_to(namespace_ref, cur_chunk_idx)
    {
      // The link pass registers this ref, not its canonical ref: `merge_cjs_namespace` runs after
      // the link pass and can link it to the namespace of another import record of the same
      // module. The module finalizer reads the namespace in the same way.
      self.cross_chunk_pattern(namespace_ref, exporter_chunk_idx, cur_chunk_idx)
    } else {
      self
        .link_output
        .symbol_db
        .canonical_name_for_or_original(namespace_ref, canonical_names)
        .to_string()
    }
  }

  /// The read of `symbol_ref` through the `require()` binding of the chunk `exporter_chunk_idx`.
  /// `symbol_ref` must be a key of `exports_to_other_chunks` of that chunk.
  fn cross_chunk_pattern(
    &self,
    symbol_ref: SymbolRef,
    exporter_chunk_idx: ChunkIdx,
    cur_chunk_idx: ChunkIdx,
  ) -> String {
    let require_binding = &self.chunk_graph.chunk_table[cur_chunk_idx]
      .require_binding_names_for_other_chunks[&exporter_chunk_idx];

    let exporter_chunk = &self.chunk_graph.chunk_table[exporter_chunk_idx];
    let exported_name = &exporter_chunk.exports_to_other_chunks[&symbol_ref][0];
    if exported_name == "default" {
      // Use the exporter chunk's actual `output_exports`, not the user-level
      // `options.exports`. Each chunk decides its own export mode (e.g. under
      // `preserveModules`, one non-entry module can emit `module.exports = ...` while
      // another emits `exports.default = ...`), so the access pattern must match what the
      // exporter actually renders.
      match exporter_chunk.output_exports {
        OutputExports::Default => require_binding.clone(),
        OutputExports::Named => property_access_str(require_binding, exported_name),
        // `Auto` is always resolved away by `determine_export_mode`. `None`
        // is unreachable here because we just indexed
        // `exports_to_other_chunks[symbol_ref]` successfully — a non-empty
        // entry means the chunk has at least one cross-chunk export, which
        // forces `output_exports` to `Default` or `Named` (entry chunks via
        // `determine_export_mode`; common chunks are unconditionally `Named`).
        OutputExports::Auto | OutputExports::None => unreachable!(),
      }
    } else {
      property_access_str(require_binding, exported_name)
    }
  }

  /// The other chunk that declares `symbol_ref`, if the chunk `cur_chunk_idx` imports
  /// `symbol_ref` from it. Then [`Self::cross_chunk_pattern`] has the `require()` binding and the
  /// export name of the symbol. If the link pass did not register such an import, the caller
  /// keeps the local name.
  fn chunk_that_exports_to(
    &self,
    symbol_ref: SymbolRef,
    cur_chunk_idx: ChunkIdx,
  ) -> Option<ChunkIdx> {
    let chunk_idx = self.link_output.symbol_db.get(symbol_ref).chunk_idx?;
    (chunk_idx != cur_chunk_idx
      && self.chunk_graph.chunk_table[cur_chunk_idx]
        .require_binding_names_for_other_chunks
        .contains_key(&chunk_idx)
      && self.chunk_graph.chunk_table[chunk_idx].exports_to_other_chunks.contains_key(&symbol_ref))
    .then_some(chunk_idx)
  }

  /// `bridge.name` when `canonical_ref` is owned by an inline common chunk record that
  /// `cur_chunk_idx` (this file, or a record printed in it) reads through a bridge.
  fn inline_bridge_pattern(
    &self,
    canonical_ref: SymbolRef,
    cur_chunk_idx: ChunkIdx,
  ) -> Option<String> {
    let canonical_ref = self.link_output.symbol_db.canonical_ref_for(canonical_ref);
    let reader = InlineReader { file: self.chunk_idx, reader: cur_chunk_idx };
    let (bridge, export_name) = self.inline_state.bridge_read(
      reader,
      canonical_ref,
      &self.link_output.symbol_db,
      self.chunk_graph,
    )?;
    Some(property_access_str(bridge, export_name))
  }

  fn canonical_name_for<'name>(
    &'name self,
    canonical_names: &'name FxHashMap<SymbolRef, CompactStr>,
    symbol: SymbolRef,
  ) -> &'name str {
    let symbol_db = &self.link_output.symbol_db;
    symbol_db.canonical_name_for_or_original(symbol, canonical_names)
  }
}

pub struct GenerateOutput {
  pub chunk: InstantiatedChunk,
  pub warnings: Vec<BuildDiagnostic>,
}
