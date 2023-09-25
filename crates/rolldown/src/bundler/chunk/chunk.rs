use oxc::span::Atom;
use rolldown_common::{ModuleId, Part, SymbolRef};
use rustc_hash::{FxHashMap, FxHashSet};
use string_wizard::{Joiner, JoinerOptions};

use crate::bundler::{
  graph::{graph::Graph, symbols::Symbols},
  module::{module::Module, module_id::ModuleVec, render::RenderModuleContext},
  options::{
    file_name_template::FileNameRenderOptions, normalized_input_options::NormalizedInputOptions,
    normalized_output_options::NormalizedOutputOptions,
  },
};

#[derive(Debug, Default)]
pub struct Chunk {
  pub is_entry: bool,
  pub modules: Vec<ModuleId>,
  pub parts: Vec<Part>,
  pub name: Option<String>,
  pub file_name: Option<String>,
  pub canonical_names: FxHashMap<SymbolRef, Atom>,
  pub exports_str: Option<String>,
}

impl Chunk {
  pub fn new(name: Option<String>, is_entry: bool, modules: Vec<ModuleId>) -> Self {
    Self {
      name,
      is_entry,
      modules,
      ..Default::default()
    }
  }

  pub fn render_file_name(&mut self, output_options: &NormalizedOutputOptions) {
    self.file_name = Some(
      output_options
        .entry_file_names
        .render(FileNameRenderOptions {
          name: self.name.as_deref(),
        }),
    )
  }

  /// - import symbols from other chunks and external modules
  // pub fn generate_cross_chunk_links(&mut self) {}

  pub fn initialize_exports(&mut self, modules: &mut ModuleVec, symbols: &Symbols) {
    let entry = &mut modules[*self.modules.first().unwrap()];

    // export { };
    if !entry.expect_normal().resolved_exports.is_empty() {
      let mut resolved_exports = entry
        .expect_normal()
        .resolved_exports
        .iter()
        .collect::<Vec<_>>();
      resolved_exports.sort_by_key(|(name, _)| name.as_str());
      let mut exports_str = "export { ".to_string();
      exports_str.push_str(
        &resolved_exports
          .into_iter()
          .map(|(exported, refer)| {
            let final_name = self
              .canonical_names
              .get(&symbols.par_get_canonical_ref(refer.local_symbol))
              .cloned()
              .unwrap_or_else(|| panic!("not found {:?}", exported));
            if final_name == exported {
              format!("{}", final_name)
            } else {
              format!("{} as {}", final_name, exported,)
            }
          })
          .collect::<Vec<_>>()
          .join(", "),
      );
      exports_str.push_str(" };");
      self.exports_str = Some(exports_str);
    }
  }

  pub fn collect_parts(&mut self, graph: &Graph) {
    let mut visited = FxHashSet::default();

    fn visit(
      module_id: &ModuleId,
      parts: &mut Vec<Part>,
      visited: &mut FxHashSet<ModuleId>,
      graph: &Graph,
      chunk_modules: &FxHashSet<ModuleId>,
    ) {
      if visited.contains(module_id) {
        return;
      }

      if let Module::Normal(module) = &graph.modules[*module_id] {
        let is_module_in_chunk = chunk_modules.contains(module_id);
        for part in module.parts.iter() {
          if let Some(import_record_id) = &part.import_record_id {
            let import_record = &module.import_records[*import_record_id];
            visit(
              &import_record.resolved_module,
              parts,
              visited,
              graph,
              chunk_modules,
            );
          }
          if is_module_in_chunk {
            if let Some(last_part) = parts.pop() {
              if part.module_id == last_part.module_id {
                parts.push(Part::new(part.module_id, last_part.start, part.end, None));
              } else {
                parts.push(last_part);
                parts.push(part.clone());
              }
            } else {
              parts.push(part.clone());
            }
          }
        }
      }

      visited.insert(*module_id);
    }

    for module_id in self.modules.iter() {
      visit(
        module_id,
        &mut self.parts,
        &mut visited,
        graph,
        &self.modules.iter().copied().collect::<FxHashSet<_>>(),
      );
    }
  }

  pub fn render(
    &self,
    graph: &Graph,
    input_options: &NormalizedInputOptions,
  ) -> anyhow::Result<String> {
    use rayon::prelude::*;
    let mut joiner = Joiner::with_options(JoinerOptions {
      separator: Some("\n".to_string()),
    });
    self
      .parts
      .par_iter()
      .filter_map(|part| {
        let module = &graph.modules[part.module_id];
        module.render(RenderModuleContext {
          symbols: &graph.symbols,
          final_names: &self.canonical_names,
          input_options,
          part,
        })
      })
      .collect::<Vec<_>>()
      .into_iter()
      .for_each(|item| {
        joiner.append(item);
      });
    // self
    //   .modules
    //   .par_iter()
    //   .copied()
    //   .map(|id| &graph.modules[id])
    //   .filter_map(|m| {
    //     m.render(RenderModuleContext {
    //       symbols: &graph.symbols,
    //       final_names: &self.canonical_names,
    //       input_options,
    //     })
    //   })
    //   .collect::<Vec<_>>()
    //   .into_iter()
    //   .for_each(|item| {
    //     joiner.append(item);
    //   });
    if let Some(exports) = self.exports_str.clone() {
      joiner.append_raw(exports);
    }

    Ok(joiner.join())
  }
}
