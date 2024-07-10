use rolldown_common::{Chunk, OutputFormat, Specifier};

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput, SharedOptions};

// clippy::too_many_lines: TODO(hyf0): refactor this function
#[allow(clippy::too_many_lines)]
pub fn render_chunk_imports(
  chunk: &Chunk,
  graph: &LinkStageOutput,
  chunk_graph: &ChunkGraph,
  options: &SharedOptions,
) -> String {
  let mut s = String::new();

  let render_import_specifier = |imported: &str, alias: &str| match options.format {
    OutputFormat::Esm => {
      if imported == alias {
        imported.to_string()
      } else {
        format!("{imported} as {alias}")
      }
    }
    OutputFormat::Cjs => {
      if imported == alias {
        imported.to_string()
      } else {
        format!("{imported}: {alias}")
      }
    }
    OutputFormat::App => {
      unreachable!("App format doesn't need to generate imports")
    }
  };

  let render_import_stmt = |import_items: &[String],
                            importee_module_specifier: &str,
                            output: &mut String| match options.format {
    OutputFormat::Esm => {
      output.push_str(&format!(
        "import {{ {} }} from \"{importee_module_specifier}\";\n",
        import_items.join(", "),
      ));
    }
    OutputFormat::Cjs => {
      output.push_str(&format!(
        "const {{ {} }} = require(\"{importee_module_specifier}\");\n",
        import_items.join(", "),
      ));
    }
    OutputFormat::App => {
      unreachable!("App format doesn't need to generate imports")
    }
  };

  let render_plain_import =
    |importee_module_specifier: &str, output: &mut String| match options.format {
      OutputFormat::Esm => {
        output.push_str(&format!("import \"{importee_module_specifier}\";\n"));
      }
      OutputFormat::Cjs => {
        output.push_str(&format!("require(\"{importee_module_specifier}\");\n"));
      }
      OutputFormat::App => {
        unreachable!("App format doesn't need to generate imports")
      }
    };

  // render imports from other chunks

  chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &chunk_graph.chunks[*exporter_id];
    let mut import_items = items
      .iter()
      .map(|item| {
        let canonical_ref = graph.symbols.par_canonical_ref_for(item.import_ref);
        let local_binding = &chunk.canonical_names[&canonical_ref];
        let Specifier::Literal(export_alias) = item.export_alias.as_ref().unwrap() else {
          panic!("should not be star import from other chunks")
        };
        render_import_specifier(export_alias, local_binding)
      })
      .collect::<Vec<_>>();

    let import_path = chunk.import_path_for(importee_chunk);

    if import_items.is_empty() {
      // TODO: filename relative to importee
      render_plain_import(&import_path, &mut s);
    } else {
      import_items.sort();
      render_import_stmt(&import_items, &import_path, &mut s);
    }
  });

  // render external imports
  let imports_from_external_modules = &chunk.imports_from_external_modules;

  if imports_from_external_modules.is_empty() {
    return s;
  }

  imports_from_external_modules.iter().for_each(|(importee_id, named_imports)| {
    let importee = &graph.module_table.modules[*importee_id]
      .as_external()
      .expect("Should be external module here");

    let external_module_side_effects = &importee.side_effects;
    let mut is_importee_imported = false;
    let mut import_items = named_imports
      .iter()
      .filter_map(|item| {
        let canonical_ref = graph.symbols.par_canonical_ref_for(item.imported_as);
        if !graph.used_symbol_refs.contains(&canonical_ref) {
          return None;
        };
        let alias = &chunk.canonical_names[&canonical_ref];
        match &item.imported {
          Specifier::Star => {
            is_importee_imported = true;
            let importee_name = &importee.name;
            match options.format {
              OutputFormat::Esm => {
                s.push_str(&format!("import * as {alias} from \"{importee_name}\";\n",));
              }
              OutputFormat::Cjs => {
                let to_esm_fn_name = &chunk.canonical_names
                  [&graph.symbols.par_canonical_ref_for(graph.runtime.resolve_symbol("__toESM"))];
                s.push_str(&format!(
                  "const {alias} = {to_esm_fn_name}(require(\"{importee_name}\"));\n",
                ));
              }
              OutputFormat::App => {}
            }

            None
          }
          Specifier::Literal(imported) => Some(render_import_specifier(imported, alias)),
        }
      })
      .collect::<Vec<_>>();
    import_items.sort();
    if !import_items.is_empty() {
      match options.format {
        OutputFormat::Esm => {
          s.push_str(&format!(
            "import {{ {} }} from \"{importee_module_specifier}\";\n",
            import_items.join(", "),
            importee_module_specifier = &importee.name
          ));
        }
        OutputFormat::Cjs => {
          let to_esm_fn_name = &chunk.canonical_names
            [&graph.symbols.par_canonical_ref_for(graph.runtime.resolve_symbol("__toESM"))];
          s.push_str(&format!(
            "const {{ {} }} = {to_esm_fn_name}(require(\"{importee_module_specifier}\"));\n",
            import_items.join(", "),
            importee_module_specifier = &importee.name
          ));
        }
        OutputFormat::App => {
          unreachable!("App format doesn't need to generate imports")
        }
      }
    } else if !is_importee_imported {
      // Ensure the side effect
      if external_module_side_effects.has_side_effects() {
        render_plain_import(&importee.name, &mut s);
      }
    }
  });
  s
}
