use arcstr::ArcStr;
use rolldown_common::{Chunk, OutputFormat, Specifier, SymbolRef};

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

#[derive(Debug, Ord, PartialEq, Eq, PartialOrd)]
pub struct RenderImportSpecifier {
  pub imported: ArcStr,
  pub alias: Option<ArcStr>,
}

#[derive(Debug)]
pub enum RenderImportDeclarationSpecifier {
  ImportSpecifier(Vec<RenderImportSpecifier>),
  ImportStarSpecifier(),
}

#[derive(Debug)]
pub struct ExternalRenderImportStmt {
  pub path: ArcStr,
  pub binding_name_token: SymbolRef, // for cjs __toESM(require('foo')) and iife get deconflict name
  pub specifiers: RenderImportDeclarationSpecifier,
}

#[derive(Debug)]
pub enum RenderImportStmt {
  NormalRenderImportStmt(),
  ExternalRenderImportStmt(ExternalRenderImportStmt),
}

pub fn collect_render_chunk_imports(
  chunk: &Chunk,
  graph: &LinkStageOutput,
  _chunk_graph: &ChunkGraph,
  format: &OutputFormat,
) -> Vec<RenderImportStmt> {
  let mut render_import_stmts = vec![];

  // render imports from other chunks
  chunk.imports_from_other_chunks.iter().for_each(|(_, items)| {
    let mut specifiers = items
      .iter()
      .map(|item| {
        let canonical_ref = graph.symbol_db.canonical_ref_for(item.import_ref);
        let local_binding = &chunk.canonical_names[&canonical_ref];
        let Specifier::Literal(export_alias) = item.export_alias.as_ref().unwrap() else {
          panic!("should not be star import from other chunks")
        };
        RenderImportSpecifier {
          imported: export_alias.as_str().into(),
          alias: if export_alias == local_binding {
            None
          } else {
            Some(local_binding.as_str().into())
          },
        }
      })
      .collect::<Vec<_>>();
    specifiers.sort_unstable();

    render_import_stmts.push(RenderImportStmt::NormalRenderImportStmt());
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
        let target = if matches!(format, OutputFormat::Esm) {
          item.imported_as
        } else {
          importee.namespace_ref
        };
        let canonical_ref = graph.symbol_db.canonical_ref_for(target);
        if !graph.used_symbol_refs.contains(&canonical_ref) {
          return None;
        };
        let alias = &chunk.canonical_names[&canonical_ref];
        match &item.imported {
          Specifier::Star => {
            has_importee_imported = true;
            render_import_stmts.push(RenderImportStmt::ExternalRenderImportStmt(
              ExternalRenderImportStmt {
                path: importee.name.clone(),
                binding_name_token: importee.namespace_ref,
                specifiers: RenderImportDeclarationSpecifier::ImportStarSpecifier(),
              },
            ));
            None
          }
          Specifier::Literal(imported) => Some(RenderImportSpecifier {
            imported: imported.as_str().into(),
            alias: if alias == imported { None } else { Some(alias.as_str().into()) },
          }),
        }
      })
      .collect::<Vec<_>>();
    specifiers.sort_unstable();

    if !specifiers.is_empty()
      || (importee.side_effects.has_side_effects() && !has_importee_imported)
    {
      render_import_stmts.push(RenderImportStmt::ExternalRenderImportStmt(
        ExternalRenderImportStmt {
          path: importee.name.clone(),
          binding_name_token: importee.namespace_ref,
          specifiers: RenderImportDeclarationSpecifier::ImportSpecifier(specifiers),
        },
      ));
    }
  });

  render_import_stmts
}
