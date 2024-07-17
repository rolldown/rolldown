use arcstr::ArcStr;
use rolldown_common::{Chunk, Specifier};

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

#[derive(Debug, Ord, PartialEq, Eq, PartialOrd)]
pub struct RenderImportSpecifier<'a> {
  pub imported: &'a str,
  pub alias: Option<&'a str>,
}

pub enum RenderImportDeclarationSpecifier<'a> {
  ImportSpecifier(Vec<RenderImportSpecifier<'a>>),
  ImportStarSpecifier(&'a str),
}

pub struct RenderImportStmt<'a> {
  pub path: ArcStr,
  pub is_external: bool, // for cjs __toESM(require('foo'))
  pub specifiers: RenderImportDeclarationSpecifier<'a>,
}

pub fn collect_render_chunk_imports<'a>(
  chunk: &'a Chunk,
  graph: &LinkStageOutput,
  chunk_graph: &ChunkGraph,
) -> Vec<RenderImportStmt<'a>> {
  let mut render_import_stmts = vec![];

  // render imports from other chunks
  chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &chunk_graph.chunks[*exporter_id];
    let mut specifiers = items
      .iter()
      .map(|item| {
        let canonical_ref = graph.symbols.par_canonical_ref_for(item.import_ref);
        let local_binding = &chunk.canonical_names[&canonical_ref];
        let Specifier::Literal(export_alias) = item.export_alias.as_ref().unwrap() else {
          panic!("should not be star import from other chunks")
        };
        RenderImportSpecifier {
          imported: export_alias,
          alias: if export_alias == local_binding { None } else { Some(local_binding) },
        }
      })
      .collect::<Vec<_>>();
    specifiers.sort_unstable();

    render_import_stmts.push(RenderImportStmt {
      // TODO: filename relative to importee
      path: chunk.import_path_for(importee_chunk).into(),
      is_external: false,
      specifiers: RenderImportDeclarationSpecifier::ImportSpecifier(specifiers),
    });
  });

  // render external imports
  chunk.imports_from_external_modules.iter().for_each(|(importee_id, named_imports)| {
    let importee = &graph.module_table.modules[*importee_id]
      .as_external()
      .expect("Should be external module here");

    let mut has_importee_imported = false;

    let mut specifiers = named_imports
      .iter()
      .filter_map(|item| {
        let canonical_ref = graph.symbols.par_canonical_ref_for(item.imported_as);
        if !graph.used_symbol_refs.contains(&canonical_ref) {
          return None;
        };
        let alias = &chunk.canonical_names[&canonical_ref];
        match &item.imported {
          Specifier::Star => {
            has_importee_imported = true;
            render_import_stmts.push(RenderImportStmt {
              path: importee.name.as_str().into(),
              is_external: true,
              specifiers: RenderImportDeclarationSpecifier::ImportStarSpecifier(alias),
            });
            None
          }
          Specifier::Literal(imported) => Some(RenderImportSpecifier {
            imported,
            alias: if alias == imported { None } else { Some(alias) },
          }),
        }
      })
      .collect::<Vec<_>>();
    specifiers.sort_unstable();

    if !specifiers.is_empty()
      || (importee.side_effects.has_side_effects() && !has_importee_imported)
    {
      render_import_stmts.push(RenderImportStmt {
        path: importee.name.as_str().into(),
        is_external: true,
        specifiers: RenderImportDeclarationSpecifier::ImportSpecifier(specifiers),
      });
    }
  });

  render_import_stmts
}
