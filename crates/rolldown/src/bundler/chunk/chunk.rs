use oxc::span::Atom;
use rolldown_common::{ModuleId, ResolvedExport, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::{Joiner, JoinerOptions};

use crate::bundler::{
  bitset::BitSet,
  graph::{
    graph::Graph,
    linker::LinkerModuleVec,
    symbols::{get_symbol_final_name, Symbols},
  },
  module::module::ModuleRenderContext,
  options::{
    file_name_template::FileNameRenderOptions, normalized_output_options::NormalizedOutputOptions,
  },
};

use super::{chunk_graph::ChunkGraph, ChunkId};

#[derive(Debug)]
pub struct CrossChunkImportItem {
  pub export_alias: Option<Atom>,
  pub import_ref: SymbolRef,
}

#[derive(Debug, Default)]
pub struct Chunk {
  pub entry_module: Option<ModuleId>,
  pub modules: Vec<ModuleId>,
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub canonical_names: FxHashMap<SymbolRef, Atom>,
  pub exports_str: Option<String>,
  pub bits: BitSet,
  pub imports_from_other_chunks: FxHashMap<ChunkId, Vec<CrossChunkImportItem>>,
  pub exports_to_other_chunks: FxHashMap<SymbolRef, Atom>,
}

impl Chunk {
  pub fn new(
    name: Option<String>,
    entry_module: Option<ModuleId>,
    bits: BitSet,
    modules: Vec<ModuleId>,
  ) -> Self {
    Self { entry_module, modules, name, bits, ..Self::default() }
  }

  pub fn render_file_name(&mut self, output_options: &NormalizedOutputOptions) {
    self.file_name = Some(
      output_options.entry_file_names.render(&FileNameRenderOptions { name: self.name.as_deref() }),
    );
  }

  pub fn initialize_exports(&mut self, linker_modules: &LinkerModuleVec, symbols: &Symbols) {
    let entry = &linker_modules[*self.modules.last().unwrap()];

    // export { };
    if !entry.resolved_exports.is_empty() {
      let mut resolved_exports = entry.resolved_exports.iter().collect::<Vec<_>>();
      resolved_exports.sort_by_key(|(name, _)| name.as_str());
      let mut vars = vec![];
      let export_items = &resolved_exports
        .into_iter()
        .map(|(exported, refer)| match refer {
          ResolvedExport::Symbol(symbol_ref) => {
            let final_name = self
              .canonical_names
              .get(&symbols.par_get_canonical_ref(*symbol_ref))
              .cloned()
              .unwrap_or_else(|| panic!("not found {exported:?}"));
            if final_name == exported {
              format!("{final_name}")
            } else {
              format!("{final_name} as {exported}")
            }
          }
          ResolvedExport::Runtime(export) => {
            let local_symbol_name =
              get_symbol_final_name(export.local.unwrap(), symbols, &self.canonical_names).unwrap();
            let importee_namespace_symbol_name =
              get_symbol_final_name(export.symbol_ref, symbols, &self.canonical_names).unwrap();
            vars.push(format!(
              "var {local_symbol_name} = {importee_namespace_symbol_name}.{exported};",
            ));
            if local_symbol_name == exported {
              format!("{local_symbol_name}")
            } else {
              format!("{local_symbol_name} as {exported}")
            }
          }
        })
        .collect::<Vec<_>>();
      self.exports_str = Some(format!(
        "{}export {{ {} }};",
        if vars.is_empty() { String::new() } else { format!("{}\n", vars.join("\n")) },
        export_items.join(", ")
      ));
    }
  }

  #[allow(clippy::unnecessary_wraps)]
  pub fn render(&self, graph: &Graph, chunk_graph: &ChunkGraph) -> anyhow::Result<String> {
    use rayon::prelude::*;
    let mut joiner = Joiner::with_options(JoinerOptions { separator: Some("\n".to_string()) });
    self.imports_from_other_chunks.iter().for_each(|(chunk_id, items)| {
      let chunk = &chunk_graph.chunks[*chunk_id];
      let mut import_items = items
        .iter()
        .map(|item| {
          let imported = chunk
            .canonical_names
            .get(&graph.symbols.par_get_canonical_ref(item.import_ref))
            .cloned()
            .unwrap();
          let alias = item.export_alias.as_ref().unwrap();
          if imported == alias {
            format!("{imported}")
          } else {
            format!("{imported} as {alias}")
          }
        })
        .collect::<Vec<_>>();
      import_items.sort();
      joiner.append_raw(format!(
        "import {{ {} }} from \"{}\";",
        import_items.join(", "),
        chunk.file_name.as_ref().unwrap()
      ));
    });
    self
      .modules
      .par_iter()
      .copied()
      .map(|id| &graph.modules[id])
      .filter_map(|m| {
        m.render(ModuleRenderContext { canonical_names: &self.canonical_names, graph, chunk_graph })
      })
      .collect::<Vec<_>>()
      .into_iter()
      .for_each(|item| {
        joiner.append(item);
      });
    if let Some(exports) = self.exports_str.clone() {
      joiner.append_raw(exports);
    }

    Ok(joiner.join())
  }
}
