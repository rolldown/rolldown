use rolldown_common::Specifier;
use string_wizard::MagicString;

use crate::bundler::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

use super::chunk::Chunk;

impl Chunk {
  // clippy::too_many_lines: TODO(hyf0): refactor this function
  #[allow(clippy::too_many_lines)]
  pub fn render_imports_for_esm(
    &self,
    graph: &LinkStageOutput,
    chunk_graph: &ChunkGraph,
  ) -> MagicString<'static> {
    let mut s = MagicString::new("");
    // render imports from external modules
    let mut imports_from_external_modules =
      self.imports_from_external_modules.iter().collect::<Vec<_>>();
    imports_from_external_modules
      .sort_unstable_by_key(|(module_id, _)| graph.modules[**module_id].exec_order());
    imports_from_external_modules.into_iter().for_each(|(importee_id, named_imports)| {
      let importee = graph.modules[*importee_id].expect_external();
      let mut is_importee_imported = false;
      let mut import_items = named_imports
        .iter()
        .filter_map(|item| {
          let canonical_ref = graph.symbols.par_canonical_ref_for(item.imported_as);
          let alias = &self.canonical_names[&canonical_ref];
          match &item.imported {
            Specifier::Star => {
              is_importee_imported = true;
              s.append(format!(
                "import * as {alias} from \"{module}\";\n",
                module = importee.resource_id.expect_file().as_str()
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
        s.append(format!(
          "import {{ {} }} from \"{}\";\n",
          import_items.join(", "),
          importee.resource_id.expect_file().as_str()
        ));
      } else if !is_importee_imported {
        // Ensure the side effect
        s.append(format!("import \"{}\";\n", importee.resource_id.expect_file().as_str()));
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
        s.append(format!("import \"./{file_name}\";\n"));
      } else {
        import_items.sort();
        s.append(format!("import {{ {} }} from \"./{file_name}\";\n", import_items.join(", ")));
      }
    });
    s
  }
}
