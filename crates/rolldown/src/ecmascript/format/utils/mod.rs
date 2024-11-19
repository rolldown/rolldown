use itertools::Itertools;
use rolldown_common::ExternalModule;

use crate::types::generator::GenerateContext;

pub mod namespace;

pub fn render_factory_parameters(
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  has_exports: bool,
) -> String {
  let mut parameters = if has_exports { vec!["exports"] } else { vec![] };
  externals.iter().for_each(|external| {
    let symbol_name = &ctx.chunk.canonical_names[&external.namespace_ref];
    parameters.push(symbol_name.as_str());
  });
  parameters.join(", ")
}

pub fn render_chunk_external_imports<'a>(
  ctx: &'a GenerateContext<'_>,
) -> (String, Vec<&'a ExternalModule>) {
  let mut import_code = String::new();

  let externals = ctx
    .chunk
    .imports_from_external_modules
    .iter()
    .filter_map(|(importee_id, _)| {
      let importee = ctx.link_output.module_table.modules[*importee_id]
        .as_external()
        .expect("Should be external module here");

      let external_module_symbol_name = &ctx.chunk.canonical_names[&importee.namespace_ref];

      if ctx.link_output.used_symbol_refs.contains(&importee.namespace_ref) {
        let to_esm_fn_name = &ctx.chunk.canonical_names[&ctx
          .link_output
          .symbol_db
          .canonical_ref_for(ctx.link_output.runtime.resolve_symbol("__toESM"))];

        import_code.push_str(external_module_symbol_name);
        import_code.push_str(" = ");
        import_code.push_str(to_esm_fn_name);
        import_code.push('(');
        import_code.push_str(external_module_symbol_name);
        import_code.push_str(");\n");
        Some(importee)
      } else if importee.side_effects.has_side_effects() {
        Some(importee)
      } else {
        None
      }
    })
    .collect_vec();

  (import_code, externals)
}
