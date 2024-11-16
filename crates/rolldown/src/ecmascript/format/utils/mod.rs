use itertools::Itertools;
use rolldown_std_utils::OptionExt;
use rolldown_utils::concat_string;

use crate::{
  types::generator::GenerateContext,
  utils::chunk::collect_render_chunk_imports::{
    collect_render_chunk_imports, ExternalRenderImportStmt, RenderImportDeclarationSpecifier,
    RenderImportStmt,
  },
};

pub mod namespace;

pub fn render_factory_parameters(
  ctx: &mut GenerateContext<'_>,
  externals: &[ExternalRenderImportStmt],
  has_exports: bool,
) -> String {
  let mut parameters = if has_exports { vec!["exports"] } else { vec![] };
  externals.iter().for_each(|external| {
    let symbol_name = ctx.chunk.canonical_name_by_token.get(&external.binding_name_token).unpack();
    parameters.push(symbol_name.as_str());
  });
  parameters.join(", ")
}

pub fn render_chunk_external_imports(
  ctx: &GenerateContext<'_>,
) -> (String, Vec<ExternalRenderImportStmt>) {
  let render_import_stmts =
    collect_render_chunk_imports(ctx.chunk, ctx.link_output, ctx.chunk_graph);

  let mut import_code = String::new();
  let externals = render_import_stmts
    .into_iter()
    .filter_map(|stmt| {
      if let RenderImportStmt::ExternalRenderImportStmt(external_stmt) = stmt {
        let symbol_name =
          ctx.chunk.canonical_name_by_token.get(&external_stmt.binding_name_token).unpack();
        match &external_stmt.specifiers {
          RenderImportDeclarationSpecifier::ImportSpecifier(specifiers) => {
            if specifiers.is_empty() {
              Some(external_stmt)
            } else {
              let specifiers = specifiers
                .iter()
                .map(|specifier| {
                  if let Some(alias) = &specifier.alias {
                    concat_string!(specifier.imported, ": ", alias)
                  } else {
                    specifier.imported.to_string()
                  }
                })
                .collect::<Vec<_>>();
              import_code.push_str("const {");
              import_code.push_str(&specifiers.join(", "));
              import_code.push_str("} = ");
              import_code.push_str(symbol_name);
              import_code.push_str(";\n");
              Some(external_stmt)
            }
          }
          RenderImportDeclarationSpecifier::ImportStarSpecifier(alias) => {
            import_code.push_str(&concat_string!("const ", alias, " = ", symbol_name, ";\n"));
            Some(external_stmt)
          }
        }
      } else {
        None
      }
    })
    .collect_vec();

  (import_code, externals)
}
