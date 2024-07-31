use rolldown_common::ExportsKind;

use crate::types::generator::GenerateContext;

pub fn determine_use_strict(ctx: &GenerateContext) -> bool {
  ctx.chunk.modules.iter().filter_map(|id| ctx.link_output.module_table.modules[*id].as_ecma()).all(
    |ecma_module| {
      let is_esm = matches!(&ecma_module.exports_kind, ExportsKind::Esm);
      is_esm || ctx.link_output.ast_table[ecma_module.ecma_ast_idx()].0.contains_use_strict
    },
  )
}
