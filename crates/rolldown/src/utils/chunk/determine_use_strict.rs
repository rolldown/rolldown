use crate::types::generator::GenerateContext;

pub fn determine_use_strict(ctx: &GenerateContext) -> bool {
  ctx.renderable_ecma_modules().all(|ecma_module| {
    ecma_module.exports_kind.is_esm()
      || ctx.link_output.ast_table[ecma_module.ecma_ast_idx()].0.contains_use_strict
  })
}
