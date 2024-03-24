use rolldown_common::Specifier;

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

use super::Chunk;

impl Chunk {
  // clippy::too_many_lines: TODO(hyf0): refactor this function
  #[allow(clippy::too_many_lines)]
  pub fn render_imports_for_esm(
    &self,
    graph: &LinkStageOutput,
    chunk_graph: &ChunkGraph,
  ) -> String {
    let mut s = String::new();
    // render imports from external modules
    let mut imports_from_external_modules =
      self.imports_from_external_modules.iter().collect::<Vec<_>>();
    imports_from_external_modules.sort_unstable_by_key(|(module_id, _)| {
      graph.module_table.external_modules[**module_id].exec_order
    });
    imports_from_external_modules.into_iter().for_each(|(importee_id, named_imports)| {
      let importee = &graph.module_table.external_modules[*importee_id];
      let mut is_importee_imported = false;
      let mut import_items = named_imports
        .iter()
        .filter_map(|item| {
          let canonical_ref = graph.symbols.par_canonical_ref_for(item.imported_as);
          let alias = &self.canonical_names[&canonical_ref];
          match &item.imported {
            Specifier::Star => {
              is_importee_imported = true;
              s.push_str(&format!(
                "import * as {alias} from \"{module}\";\n",
                module = &importee.name
              ));
              None
            }
            Specifier::Literal(imported) => Some(if imported == alias {
              format!("{imported}")
            } else {
              format!("{imported} as {alias}")
            }),
          }
        })
        .collect::<Vec<_>>();
      import_items.sort();
      if !import_items.is_empty() {
        s.push_str(&format!(
          "import {{ {} }} from \"{}\";\n",
          import_items.join(", "),
          &importee.name
        ));
      } else if !is_importee_imported {
        // Ensure the side effect
        s.push_str(&format!("import \"{}\";\n", importee.name));
      }
    });

    // render imports from other chunks

    self.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
      let importee_chunk = &chunk_graph.chunks[*exporter_id];
      let mut import_items = items
        .iter()
        .map(|item| {
          let canonical_ref = graph.symbols.par_canonical_ref_for(item.import_ref);
          let local_binding = &self.canonical_names[&canonical_ref];
          let Specifier::Literal(export_alias) = item.export_alias.as_ref().unwrap() else {
            panic!("should not be star import from other chunks")
          };
          if export_alias == local_binding {
            format!("{export_alias}")
          } else {
            format!("{export_alias} as {local_binding}")
          }
        })
        .collect::<Vec<_>>();
      let file_name = importee_chunk
        .file_name
        .as_ref()
        .expect("At this point, file name should already be generated");
      if import_items.is_empty() {
        s.push_str(&format!("import \"./{file_name}\";\n"));
      } else {
        import_items.sort();
        s.push_str(&format!("import {{ {} }} from \"./{file_name}\";\n", import_items.join(", ")));
      }
    });
    s
  }
}
