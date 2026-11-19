use crate::utils::chunk::namespace_marker::render_namespace_markers;
use crate::utils::chunk::render_chunk_exports::{
  get_chunk_export_names_with_ctx, render_wrapped_entry_chunk,
};
use crate::{
  chunk_graph::LazyHostInits,
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::render_chunk_exports,
  utils::external_import_interop::{ChunkAssignments, chunk_external_interop_modes},
};
use itertools::Either;
use rolldown_common::{AddonRenderContext, ExportsKind, OutputExports, StaticImport};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{
  concat_string,
  ecmascript::{escape_non_ascii, property_access_str_with_ascii, to_ecma_string_literal},
};
use rustc_hash::FxHashMap;

use super::utils::{render_chunk_directives, render_modules_with_peek_runtime_module_at_first};

#[expect(clippy::needless_pass_by_value)]
pub fn render_cjs<'code>(
  ctx: &GenerateContext<'_>,
  addon_render_context: AddonRenderContext<'code>,
  module_sources: &'code RenderedModuleSources,
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
      if let Some(marker) = render_namespace_markers(
        ctx.options.es_module,
        has_default_export,
        &ctx.options.generated_code,
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
  let ascii_only = ctx.options.charset.is_ascii();

  // `__toESM` can come from a chunk that this list requires after the external module, so the
  // conversions of the external modules go after all the `require()` calls.
  let mut conversions = String::new();
  let direct_external_imports = ctx
    .chunk
    .direct_imports_from_external_modules
    .iter()
    .map(|(importee_idx, named_imports)| (*importee_idx, named_imports))
    .collect::<FxHashMap<_, _>>();
  // The chunk imports and the external imports in the order of `static_imports`, then the external
  // modules that the chunk takes symbols from without importing them directly. An entry-level
  // `export *` from an external module that the chunk does not import otherwise is not written
  // here.
  ctx
    .chunk
    .static_imports
    .iter()
    .filter_map(|static_import| match *static_import {
      StaticImport::Chunk(exporter_id) => Some(Either::Left(exporter_id)),
      StaticImport::External(importee_idx) => direct_external_imports
        .get(&importee_idx)
        .map(|named_imports| Either::Right((importee_idx, Some(*named_imports)))),
    })
    .chain(
      ctx
        .chunk
        .import_symbol_from_external_modules
        .iter()
        .map(|importee_idx| Either::Right((*importee_idx, None))),
    )
    .for_each(|import| match import {
      Either::Left(exporter_id) => {
        let importee_chunk = &ctx.chunk_graph.chunk_table[exporter_id];
        let import_path =
          to_ecma_string_literal(&ctx.chunk.import_path_for(importee_chunk), ascii_only);
        let require_path_str = concat_string!("require(", import_path, ");\n");
        let items = ctx
          .chunk
          .imports_from_other_chunks
          .get(&exporter_id)
          .expect("every chunk of `static_imports` is in `imports_from_other_chunks`");
        if items.is_empty() {
          let forwards_cjs_entry =
            ctx.chunk.entry_module(&ctx.link_output.module_table).is_some_and(|entry| {
              matches!(entry.exports_kind, ExportsKind::CommonJs)
                && ctx.link_output.metas[entry.idx].wrap_kind().is_none()
                && ctx.chunk_graph.module_to_chunk[entry.idx] == Some(exporter_id)
                && importee_chunk.entry_module_idx() == Some(entry.idx)
            });
          if forwards_cjs_entry {
            s.push_str("module.exports = ");
          }
          s.push_str(&require_path_str);
        } else {
          let binding = &ctx.chunk.require_binding_names_for_other_chunks[&exporter_id];
          s.push_str("const ");
          s.push_str(binding);
          s.push_str(" = ");
          s.push_str(&require_path_str);
        }
        // The chunk initializes deferred modules right after this `require()`, before the later
        // imports run (`plan_lazy_host_inits`). The host is this chunk or an earlier import, so
        // its binding exists here.
        // See internal-docs/import-defer/implementation.md ("Init after the import of a host").
        for init in ctx.chunk_graph.lazy_host_inits.inits_after(ctx.chunk_idx, exporter_id) {
          if let Some(name) = LazyHostInits::init_export_name(
            ctx.chunk_graph,
            ctx.link_output,
            init.host,
            init.module,
          ) {
            let binding = &ctx.chunk.require_binding_names_for_other_chunks[&init.host];
            s.push_str(&property_access_str_with_ascii(binding, &name, ascii_only));
            s.push_str("();\n");
          }
        }
      }
      Either::Right((importee_idx, named_imports)) => {
        let importee = ctx.link_output.module_table[importee_idx]
          .as_external()
          .expect("Should be external module here");

        let file_name = importee.get_file_name(ctx.resolved_paths);
        let import_path = to_ecma_string_literal(
          &importee.import_path_from_file_name(ctx.chunk, file_name.clone()),
          ascii_only,
        );
        let require_path_str = concat_string!("require(", import_path, ")");

        if ctx.link_output.used_external_symbols.contains(&importee.namespace_ref) {
          let external_module_symbol_name = ctx
            .link_output
            .symbol_db
            .canonical_name_for_or_original(importee.namespace_ref, &ctx.chunk.canonical_names);
          let external_module_symbol_name =
            escape_non_ascii(external_module_symbol_name, ascii_only);
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
            // A module of a deferred closure loads the external lazily with `__toESMCached`, in
            // this chunk or in another chunk (`has_lazy_namespace_load`). The binding of each chunk
            // uses the same helper and key, so all loads give one namespace object. The key is the
            // file name of the external, which is the same in all chunks.
            let cache_key = ctx.link_output.metas[importee_idx]
              .has_lazy_namespace_load
              .then(|| to_ecma_string_literal(&file_name, ascii_only));
            let to_esm_fn = ctx.finalized_string_pattern_for_symbol_ref(
              ctx.link_output.runtime.resolve_symbol(if cache_key.is_some() {
                "__toESMCached"
              } else {
                "__toESM"
              }),
              ctx.chunk_idx,
              &ctx.chunk.canonical_names,
            );
            // `__toESMCached("ext", ` or `__toESM(`.
            let to_esm_call_start = match &cache_key {
              Some(cache_key) => concat_string!(to_esm_fn, "(", cache_key, ", "),
              None => concat_string!(to_esm_fn, "("),
            };
            let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(importee.namespace_ref);
            if let Some(node_mode_name) = ctx.chunk.node_mode_external_ns_names.get(&canonical_ref)
            {
              // Mixed-mode: emit two __toESM bindings (node-mode and non-node-mode)
              s.push_str(&concat_string!(
                "let ",
                external_module_symbol_name.as_ref(),
                " = ",
                require_path_str,
                ";\n"
              ));
              conversions.push_str(&concat_string!(
                "let ",
                escape_non_ascii(node_mode_name, ascii_only),
                " = ",
                to_esm_call_start,
                external_module_symbol_name.as_ref(),
                ", 1);\n",
                external_module_symbol_name.as_ref(),
                " = ",
                to_esm_call_start,
                external_module_symbol_name.as_ref(),
                ");\n"
              ));
            } else {
              // Single-mode. Both flags can still be set: deconflicting suppresses the node binding
              // when no ESM-format module in the chunk would read it (`chunk_has_node_esm_reader`).
              // Those readers take this plain binding, so non-Node is the mode that matches them.
              let node_mode_arg =
                if interop_modes.node_esm && !interop_modes.non_node_esm { ", 1" } else { "" };
              s.push_str(&concat_string!(
                "let ",
                external_module_symbol_name.as_ref(),
                " = ",
                require_path_str,
                ";\n"
              ));
              conversions.push_str(&concat_string!(
                external_module_symbol_name.as_ref(),
                " = ",
                to_esm_call_start,
                external_module_symbol_name.as_ref(),
                node_mode_arg,
                ");\n"
              ));
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
      }
    });

  s.push_str(&conversions);
  s
}
