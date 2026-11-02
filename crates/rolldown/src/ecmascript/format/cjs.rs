use crate::utils::chunk::namespace_marker::render_namespace_markers;
use crate::utils::chunk::render_chunk_exports::{
  get_chunk_export_names_with_ctx, render_wrapped_entry_chunk,
};
use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::render_chunk_exports,
  utils::external_import_interop::{ChunkAssignments, chunk_external_interop_modes},
};
use rolldown_common::{AddonRenderContext, OutputExports};
use rolldown_error::BuildDiagnostic;
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{
  concat_string,
  ecmascript::{escape_non_ascii, to_ecma_string_literal},
};

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

  let ast_dce = ctx.chunk_ast_dce.as_ref();
  if ast_dce.is_none() {
    if let Some(marker) = render_cjs_namespace_markers(ctx) {
      source_joiner.append_source(marker);
    }
  }

  if let Some(output) = ast_dce {
    if let Some(imports) = &output.imports {
      if let Some(map) = &output.imports_map {
        source_joiner
          .append_source(rolldown_sourcemap::SourceMapSource::new(imports.clone(), map.clone()));
      } else {
        source_joiner.append_source(imports.clone());
      }
    }
    for source in module_sources {
      if let Some(sources) = &source.sources {
        for source in sources.iter() {
          source_joiner.append_source(source);
        }
      }
    }
    if let Some(exports) = &output.exports {
      if let Some(map) = &output.exports_map {
        source_joiner
          .append_source(rolldown_sourcemap::SourceMapSource::new(exports.clone(), map.clone()));
      } else {
        source_joiner.append_source(exports.clone());
      }
    }
  } else {
    // Runtime helpers must precede generated requires that use them.
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
  }

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }
  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }
  source_joiner
}

pub fn render_cjs_namespace_markers(ctx: &GenerateContext<'_>) -> Option<&'static str> {
  if let (Some(entry_module), OutputExports::Named) =
    (ctx.chunk.user_defined_entry_module(&ctx.link_output.module_table), ctx.chunk.output_exports)
  {
    let export_names = get_chunk_export_names_with_ctx(ctx);
    let has_default_export = export_names.iter().any(|name| name.as_str() == "default");

    // Only `named` export can we render the namespace markers.
    if entry_module.exports_kind.is_esm() {
      // Symbol.toStringTag should only be added to module facades (chunks that represent a specific module)
      return render_namespace_markers(
        ctx.options.es_module,
        has_default_export,
        &ctx.options.generated_code,
        ctx.chunk.is_entry_point(),
      );
    }
  }
  None
}

// Make sure the imports generate stmts keep live bindings.
pub fn render_cjs_chunk_imports(ctx: &GenerateContext<'_>) -> String {
  let mut s = String::new();
  let ascii_only = ctx.options.charset.is_ascii();

  // render imports from other chunks
  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_id];
    let import_path =
      to_ecma_string_literal(&ctx.chunk.import_path_for(importee_chunk), ascii_only);
    let require_path_str = concat_string!("require(", import_path, ");\n");
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

      let import_path = to_ecma_string_literal(
        &importee.get_import_path(ctx.chunk, ctx.resolved_paths),
        ascii_only,
      );
      let require_path_str = concat_string!("require(", import_path, ")");

      if ctx.link_output.used_external_symbols.contains(&importee.namespace_ref) {
        let external_module_symbol_name = ctx
          .link_output
          .symbol_db
          .canonical_name_for_or_original(importee.namespace_ref, &ctx.chunk.canonical_names);
        let external_module_symbol_name = escape_non_ascii(external_module_symbol_name, ascii_only);
        // `named_imports` only covers imports written by modules that live in this chunk, so the
        // mode set also folds in what the inclusion pass recorded for observers landing here.
        // Deconflicting derives the mixed-mode binding names from this same call.
        let interop_modes = chunk_external_interop_modes(
          ctx.link_output,
          ChunkAssignments::from_graph(ctx.chunk_graph),
          ctx.chunk_idx,
          importee.namespace_ref,
          named_imports.map(Vec::as_slice),
        );
        if let Some(interop_modes) = interop_modes {
          let to_esm_fn = ctx.finalized_string_pattern_for_symbol_ref(
            ctx.link_output.runtime.resolve_symbol("__toESM"),
            ctx.chunk_idx,
            &ctx.chunk.canonical_names,
          );
          let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(importee.namespace_ref);
          if let Some(node_mode_name) = ctx.chunk.node_mode_external_ns_names.get(&canonical_ref) {
            // Mixed-mode: emit two __toESM bindings (node-mode and non-node-mode)
            let require_external = concat_string!(
              "let ",
              external_module_symbol_name.as_ref(),
              " = ",
              require_path_str,
              ";\nlet ",
              escape_non_ascii(node_mode_name, ascii_only),
              " = ",
              to_esm_fn,
              "(",
              external_module_symbol_name.as_ref(),
              ", 1);\n",
              external_module_symbol_name.as_ref(),
              " = ",
              to_esm_fn,
              "(",
              external_module_symbol_name.as_ref(),
              ");\n"
            );
            s.push_str(&require_external);
          } else {
            // Single-mode. Both flags can still be set: deconflicting suppresses the node binding
            // when no ESM-format module in the chunk would read it (`chunk_has_node_esm_reader`).
            // Those readers take this plain binding, so non-Node is the mode that matches them.
            let node_mode_arg =
              if interop_modes.node_esm && !interop_modes.non_node_esm { ", 1" } else { "" };
            let require_external = concat_string!(
              "let ",
              external_module_symbol_name.as_ref(),
              " = ",
              require_path_str,
              ";\n",
              external_module_symbol_name.as_ref(),
              " = ",
              to_esm_fn,
              "(",
              external_module_symbol_name.as_ref(),
              node_mode_arg,
              ");\n"
            );
            s.push_str(&require_external);
          }
        } else {
          // generate code like:
          // let external_module_symbol_name = require("external-module");
          let require_external = concat_string!(
            "let ",
            external_module_symbol_name.as_ref(),
            " = ",
            require_path_str,
            ";\n"
          );
          s.push_str(&require_external);
        }
      } else if importee.side_effects.has_side_effects() {
        s.push_str(&require_path_str);
        s.push_str(";\n");
      }
    });

  s
}
