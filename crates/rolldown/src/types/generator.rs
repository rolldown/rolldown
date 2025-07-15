use oxc_index::IndexVec;
use rolldown_common::{
  Chunk, ChunkIdx, InstantiatedChunk, ModuleRenderOutput, NormalizedBundlerOptions, SymbolRef,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::SharedPluginDriver;
use rolldown_rstr::Rstr;
use rolldown_utils::{ecmascript::property_access_str, indexmap::FxIndexMap};
use rustc_hash::FxHashMap;

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

pub struct GenerateContext<'a> {
  pub chunk_idx: ChunkIdx,
  pub chunk: &'a Chunk,
  pub options: &'a NormalizedBundlerOptions,
  pub link_output: &'a LinkStageOutput,
  pub chunk_graph: &'a ChunkGraph,
  pub plugin_driver: &'a SharedPluginDriver,
  pub warnings: Vec<BuildDiagnostic>,
  pub module_id_to_codegen_ret: Vec<Option<ModuleRenderOutput>>,
  /// The key of the map is exported item symbol,
  /// the value of the map is optional alias. e.g.
  /// - chunkB.js
  /// ```js
  /// export const a = 10000000;
  /// export {a as b}; // symbol_ref points to `a`, and alias is `b`
  /// ```
  pub render_export_items_index_vec: &'a IndexVec<ChunkIdx, FxIndexMap<SymbolRef, Vec<Rstr>>>,
}

impl GenerateContext<'_> {
  /// A `SymbolRef` might be identifier or a property access. This function will return correct string pattern for the symbol.
  pub fn finalized_string_pattern_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    cur_chunk_idx: ChunkIdx,
    canonical_names: &FxHashMap<SymbolRef, Rstr>,
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
      let canonical_ns_name = &canonical_names[&ns_alias.namespace_ref];
      let property_name = &ns_alias.property_name;
      return property_access_str(canonical_ns_name, property_name);
    }

    if self.link_output.module_table[canonical_ref.owner].is_external() {
      let namespace = &canonical_names[&canonical_ref];
      return namespace.to_string();
    }

    match self.options.format {
      rolldown_common::OutputFormat::Cjs => {
        let chunk_idx_of_canonical_symbol = canonical_symbol.chunk_id.unwrap_or_else(|| {
          // Scoped symbols don't get assigned a `ChunkId`. There are skipped for performance reason, because they are surely
          // belong to the chunk they are declared in and won't link to other chunks.
          let symbol_name = canonical_ref.name(symbol_db);
          panic!("{canonical_ref:?} {symbol_name:?} is not in any chunk, which isn't unexpected");
        });

        let is_symbol_in_other_chunk = cur_chunk_idx != chunk_idx_of_canonical_symbol;
        if is_symbol_in_other_chunk {
          // In cjs output, we need convert the `import { foo } from 'foo'; console.log(foo);`;
          // If `foo` is split into another chunk, we need to convert the code `console.log(foo);` to `console.log(require_xxxx.foo);`
          // instead of keeping `console.log(foo)` as we did in esm output. The reason here is we need to keep live binding in cjs output.

          let exported_name = &self.chunk_graph.chunk_table[chunk_idx_of_canonical_symbol]
            .exports_to_other_chunks[&canonical_ref][0];

          let require_binding = &self.chunk_graph.chunk_table[cur_chunk_idx]
            .require_binding_names_for_other_chunks[&chunk_idx_of_canonical_symbol];
          rolldown_utils::ecmascript::property_access_str(require_binding, exported_name)
        } else {
          self.canonical_name_for(canonical_names, canonical_ref).to_string()
        }
      }
      _ => self.canonical_name_for(canonical_names, canonical_ref).to_string(),
    }
  }

  fn canonical_name_for<'name>(
    &self,
    canonical_names: &'name FxHashMap<SymbolRef, Rstr>,
    symbol: SymbolRef,
  ) -> &'name Rstr {
    let symbol_db = &self.link_output.symbol_db;
    symbol_db.canonical_name_for(symbol, canonical_names).unwrap_or_else(|| {
      panic!(
        "canonical name not found for {symbol:?}, original_name: {:?} in module {:?}",
        symbol.name(symbol_db),
        self.link_output.module_table.get(symbol.owner).map_or("unknown", |module| module.id())
      );
    })
  }
}

pub struct GenerateOutput {
  pub chunks: Vec<InstantiatedChunk>,
  pub warnings: Vec<BuildDiagnostic>,
}

pub trait Generator {
  async fn instantiate_chunk(
    ctx: &mut GenerateContext,
  ) -> anyhow::Result<BuildResult<GenerateOutput>>;
}
