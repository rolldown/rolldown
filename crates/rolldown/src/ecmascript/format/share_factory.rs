//! Rendering for `experimentalInlineCommonChunks`: a record's factory, rendered once and printed
//! by every carrier, and the registration/bridge statements a carrier adds around it.
//! See internal-docs/inline-common-chunks/implementation.md.

use json_escape_simd::escape;
use rolldown_common::{ModuleId, RenderedModule};
use rolldown_error::BuildDiagnostic;
use rolldown_sourcemap::SourceJoiner;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  concat_string,
  ecmascript::is_validate_identifier_name,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;

use crate::{
  ecmascript::ecma_generator::{RenderedModuleSource, RenderedModuleSources},
  types::generator::GenerateContext,
  utils::{chunk::render_chunk_exports::get_export_items, render_ecma_module::render_ecma_module},
};

/// One record's factory body. The carrier prints `__share("<id>", ` + `prelude` + the module
/// sources + `epilogue` + `);`.
pub struct RenderedRecord {
  /// `(exports) => {` followed by the bridge declarations for the records this record reads.
  pub prelude: String,
  pub module_sources: RenderedModuleSources,
  /// The getter table handed to `__share_export` and the factory's closing brace.
  pub epilogue: String,
  /// What the carriers report in `RenderedChunk.modules` for the copied modules.
  pub rendered_modules: FxHashMap<ModuleId, RenderedModule>,
}

fn runtime_helper_name<'a>(ctx: &'a GenerateContext<'_>, name: &str) -> &'a str {
  ctx.link_output.symbol_db.canonical_name_for_or_original(
    ctx.link_output.runtime.resolve_symbol(name),
    &ctx.chunk.canonical_names,
  )
}

fn object_key(name: &str) -> String {
  if is_validate_identifier_name(name) { name.to_string() } else { escape(name) }
}

/// Renders the record `ctx.chunk` once. `ctx.module_id_to_codegen_ret` is consumed.
pub fn render_record(ctx: &mut GenerateContext<'_>) -> (RenderedRecord, Vec<BuildDiagnostic>) {
  let record = ctx.inline_state.record(ctx.chunk_idx).expect("chunk should be a record");
  let module_id_to_codegen_ret = std::mem::take(&mut ctx.module_id_to_codegen_ret);
  let rendered_pairs: Vec<(RenderedModuleSource, Vec<BuildDiagnostic>)> = ctx
    .chunk
    .modules
    .par_iter()
    .copied()
    .zip(module_id_to_codegen_ret)
    .filter_map(|(id, codegen_ret)| {
      ctx.link_output.module_table[id]
        .as_normal()
        .map(|m| (m, codegen_ret.expect("should have codegen_ret")))
    })
    .map(|(m, codegen_ret)| {
      let render = render_ecma_module(m, ctx.options, codegen_ret);
      (
        RenderedModuleSource::new(m.idx, m.id.clone(), m.exec_order, render.sources),
        render.warnings,
      )
    })
    .collect();
  let mut warnings = Vec::new();
  let module_sources: RenderedModuleSources = rendered_pairs
    .into_iter()
    .map(|(source, module_warnings)| {
      warnings.extend(module_warnings);
      source
    })
    .collect();
  let rendered_modules = module_sources
    .iter()
    .map(|rendered_module_source| {
      let RenderedModuleSource { module_idx, module_id, exec_order, sources } =
        rendered_module_source;
      let rendered_exports = ctx.link_output.metas[*module_idx]
        .resolved_exports
        .iter()
        .filter(|(_, export)| ctx.link_output.retained_export_symbols.contains(&export.symbol_ref))
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
      (module_id.clone(), RenderedModule::new(sources.clone(), rendered_exports, *exec_order))
    })
    .collect();

  let mut prelude = concat_string!("(", record.exports_param, ") => {");
  let read = ctx.inline_state.readers_of(ctx.chunk_idx);
  if !read.is_empty() {
    let require_name = runtime_helper_name(ctx, "__share_require");
    for other in read {
      let bridge =
        ctx.inline_state.bridge_name(ctx.chunk_idx, *other).expect("reader has a bridge name");
      let id = &ctx.inline_state.record(*other).expect("reader targets a record").id;
      prelude.push_str(&concat_string!(
        "\n\tvar ",
        bridge,
        " = ",
        require_name,
        "(",
        escape(id),
        ");"
      ));
    }
  }

  let mut epilogue = String::new();
  let export_items = get_export_items(ctx.chunk);
  if !export_items.is_empty() {
    epilogue.push('\t');
    epilogue.push_str(runtime_helper_name(ctx, "__share_export"));
    epilogue.push('(');
    epilogue.push_str(&record.exports_param);
    epilogue.push_str(", {");
    for (index, (exported_name, symbol_ref)) in export_items.iter().enumerate() {
      if index > 0 {
        epilogue.push(',');
      }
      let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(*symbol_ref);
      let value = ctx.finalized_string_pattern_for_symbol_ref(
        canonical_ref,
        ctx.chunk_idx,
        &ctx.chunk.canonical_names,
      );
      epilogue.push_str(&concat_string!("\n\t\t", object_key(exported_name), ": () => ", value));
    }
    epilogue.push_str("\n\t});\n");
  }
  epilogue.push('}');

  (RenderedRecord { prelude, module_sources, epilogue, rendered_modules }, warnings)
}

/// Prints, for a carrier, every carried record's registration and then the bridges for the
/// records the carrier reads. Registration precedes any `__share_require` in the same file.
pub fn render_inline_records<'code>(
  ctx: &GenerateContext<'code>,
  source_joiner: &mut SourceJoiner<'code>,
) {
  let carried = ctx.inline_state.carried_by(ctx.chunk_idx);
  if carried.is_empty() {
    return;
  }
  let share_name = runtime_helper_name(ctx, "__share");
  for record_idx in carried {
    let record = ctx.inline_state.record(*record_idx).expect("carried chunk is a record");
    let rendered = &ctx.inline_renders[record_idx];
    source_joiner.append_source(concat_string!(
      share_name,
      "(",
      escape(&record.id),
      ", ",
      rendered.prelude
    ));
    for module_source in &rendered.module_sources {
      if let Some(sources) = &module_source.sources {
        for source in sources.as_ref() {
          source_joiner.append_source(source);
        }
      }
    }
    source_joiner.append_source(concat_string!(rendered.epilogue, ");"));
  }
  let read = ctx.inline_state.readers_of(ctx.chunk_idx);
  if !read.is_empty() {
    let require_name = runtime_helper_name(ctx, "__share_require");
    let mut bridges = String::new();
    for record_idx in read {
      let record = ctx.inline_state.record(*record_idx).expect("read chunk is a record");
      let bridge =
        ctx.inline_state.bridge_name(ctx.chunk_idx, *record_idx).expect("reader has a bridge name");
      if !bridges.is_empty() {
        bridges.push('\n');
      }
      bridges.push_str(&concat_string!(
        "var ",
        bridge,
        " = ",
        require_name,
        "(",
        escape(&record.id),
        ");"
      ));
    }
    source_joiner.append_source(bridges);
  }
}
