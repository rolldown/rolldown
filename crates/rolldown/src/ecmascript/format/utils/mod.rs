use itertools::Itertools;

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
    let symbol_name = &ctx.chunk.canonical_names[&external.binding_name_token];
    parameters.push(symbol_name.as_str());
  });
  parameters.join(", ")
}

pub fn render_chunk_external_imports(
  ctx: &GenerateContext<'_>,
) -> (String, Vec<ExternalRenderImportStmt>) {
  let render_import_stmts =
    collect_render_chunk_imports(ctx.chunk, ctx.link_output, ctx.chunk_graph, ctx.options.format);

  let mut import_code = String::new();
  let externals = render_import_stmts
    .into_iter()
    .filter_map(|stmt| {
      if let RenderImportStmt::ExternalRenderImportStmt(external_stmt) = stmt {
        let symbol_name = &ctx.chunk.canonical_names[&external_stmt.binding_name_token];

        let need_to_esm_wrapper = match &external_stmt.specifiers {
          RenderImportDeclarationSpecifier::ImportSpecifier(specifiers) => !specifiers.is_empty(),
          RenderImportDeclarationSpecifier::ImportStarSpecifier() => true,
        };
        if need_to_esm_wrapper {
          let to_esm_fn_name = &ctx.chunk.canonical_names[&ctx
            .link_output
            .symbol_db
            .canonical_ref_for(ctx.link_output.runtime.resolve_symbol("__toESM"))];
          import_code.push_str(&format!("{symbol_name} = {to_esm_fn_name}({symbol_name});\n"));
        }

        Some(external_stmt)
      } else {
        None
      }
    })
    .collect_vec();

  (import_code, externals)
}
