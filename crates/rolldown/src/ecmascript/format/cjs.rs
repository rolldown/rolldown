use crate::utils::chunk::namespace_marker::render_namespace_markers;
use crate::utils::chunk::render_chunk_exports::{
  get_chunk_export_names_with_ctx, render_wrapped_entry_chunk,
};
use crate::{
  ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::render_chunk_exports,
  utils::external_import_interop::external_import_needs_interop,
};
use rolldown_common::{AddonRenderContext, OutputExports};
use rolldown_error::BuildDiagnostic;
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::concat_string;

use super::utils::{render_chunk_directives, render_modules_with_peek_runtime_module_at_first};

#[expect(clippy::needless_pass_by_value)]
pub fn render_cjs<'code>(
  ctx: &GenerateContext<'_>,
  addon_render_context: AddonRenderContext<'code>,
  module_sources: &'code RenderedModuleSources,
  _warnings: &mut Vec<BuildDiagnostic>,
) -> SourceJoiner<'code> {
  let mut source_joiner = SourceJoiner::default();
  let AddonRenderContext { hashbang, banner, intro, outro, footer, directives } =
    addon_render_context;

  if let Some(hashbang) = hashbang {
    source_joiner.append_source(hashbang);
  }
  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  if !directives.is_empty() {
    let rendered_chunk_directives = render_chunk_directives(directives.iter());
    if !rendered_chunk_directives.is_empty() {
      source_joiner.append_source(rendered_chunk_directives);
    }
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  // Use pre-computed output_exports from the chunk
  let export_mode = ctx.chunk.output_exports;

  // Handle namespace markers for entry chunks
  if let (Some(entry_module), OutputExports::Named) =
    (ctx.chunk.user_defined_entry_module(&ctx.link_output.module_table), export_mode)
  {
    let export_names = get_chunk_export_names_with_ctx(ctx);
    let has_default_export = export_names.iter().any(|name| name.as_str() == "default");

    // Only `named` export can we render the namespace markers.
    if entry_module.exports_kind.is_esm() {
      // Symbol.toStringTag should only be added to module facades (chunks that represent a specific module)
      if let Some(marker) = render_namespace_markers(
        ctx.options.es_module,
        has_default_export,
        &ctx.options.generated_code,
        ctx.chunk.is_entry_point(),
      ) {
        source_joiner.append_source(marker);
      }
    }
  }

  // Runtime module should be placed before the generated `requires` in CJS format.
  // Because, we might need to generate `__toESM(require(...))` that relies on the runtime module.
  render_modules_with_peek_runtime_module_at_first(
    ctx,
    &mut source_joiner,
    module_sources,
    render_cjs_chunk_imports(ctx),
  );

  if let Some(source) = render_wrapped_entry_chunk(ctx, Some(&export_mode)) {
    source_joiner.append_source(source);
  }

  if let Some(exports) = render_chunk_exports(ctx, Some(&export_mode)) {
    source_joiner.append_source(exports);
  }

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  source_joiner
}

// Make sure the imports generate stmts keep live bindings.
fn render_cjs_chunk_imports(ctx: &GenerateContext<'_>) -> String {
  let mut s = String::new();

  // render imports from other chunks
  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_id];
    let require_path_str =
      concat_string!("require('", ctx.chunk.import_path_for(importee_chunk), "');\n");
    if items.is_empty() {
      s.push_str(&require_path_str);
    } else {
      s.push_str("const ");
      s.push_str(&ctx.chunk.require_binding_names_for_other_chunks[exporter_id]);
      s.push_str(" = ");
      s.push_str(&require_path_str);
    }
  });
  // render external imports
  ctx
    .chunk
    .direct_imports_from_external_modules
    .iter()
    .map(|(importee_id, named_imports)| (importee_id, Some(named_imports)))
    .chain(
      ctx.chunk.import_symbol_from_external_modules.iter().map(|importee_id| (importee_id, None)),
    )
    .for_each(|(importee_idx, named_imports)| {
      let importee = ctx.link_output.module_table[*importee_idx]
        .as_external()
        .expect("Should be external module here");

      let require_path_str = concat_string!(
        "require(\"",
        &importee.get_import_path(ctx.chunk, ctx.options.paths.as_ref()),
        "\")"
      );

      if ctx.link_output.used_symbol_refs.contains(&importee.namespace_ref) {
        let external_module_symbol_name = ctx
          .link_output
          .symbol_db
          .canonical_name_for_or_original(importee.namespace_ref, &ctx.chunk.canonical_names);
        // Check if this import needs __toESM
        let needs_interop =
          named_imports.is_some_and(|imports| external_import_needs_interop(imports));
        if needs_interop {
          // generate code like:
          // let external_module_symbol_name = require("external-module");
          // external_module_symbol_name = __toESM(external_module_symbol_name);
          let require_external = concat_string!(
            "let ",
            external_module_symbol_name,
            " = ",
            require_path_str,
            ";\n",
            external_module_symbol_name,
            " = ",
            ctx.finalized_string_pattern_for_symbol_ref(
              ctx.link_output.runtime.resolve_symbol("__toESM"),
              ctx.chunk_idx,
              &ctx.chunk.canonical_names,
            ),
            "(",
            external_module_symbol_name,
            ");\n"
          );
          s.push_str(&require_external);
        } else {
          // generate code like:
          // let external_module_symbol_name = require("external-module");
          let require_external =
            concat_string!("let ", external_module_symbol_name, " = ", require_path_str, ";\n");
          s.push_str(&require_external);
        }
      } else if importee.side_effects.has_side_effects() {
        s.push_str(&require_path_str);
        s.push_str(";\n");
      }
    });

  s
}
