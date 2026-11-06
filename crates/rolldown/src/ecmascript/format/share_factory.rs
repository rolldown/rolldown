//! Rendering for `experimentalInlineCommonChunks`: the registrations and bridges a file prints
//! right after its imports. See internal-docs/inline-common-chunks/implementation.md.

use rolldown_common::ChunkIdx;
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{
  concat_string,
  ecmascript::{escape_non_ascii, to_ecma_string_literal, to_module_import_export_name_with_ascii},
};

use crate::{
  ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::get_export_items,
};

fn runtime_helper_name<'a>(ctx: &'a GenerateContext<'_>, name: &str) -> &'a str {
  ctx.link_output.symbol_db.canonical_name_for_or_original(
    ctx.link_output.runtime.resolve_symbol(name),
    &ctx.chunk.canonical_names,
  )
}

/// The getter table is an object literal, where `__proto__: value` sets the prototype; a computed
/// key defines the property, as `__exportAll` tables do.
fn object_key(name: &str, ascii_only: bool) -> String {
  if name == "__proto__" {
    "[\"__proto__\"]".to_string()
  } else {
    to_module_import_export_name_with_ascii(name, ascii_only)
  }
}

/// Prints, for a file that reads records, the factory of every record it carries (dependencies
/// first) and then its bridges, so a registration always precedes any `__share_require` the file
/// can see. `carried` holds the record modules as finalized and printed for this file.
pub fn render_inline_records<'code>(
  ctx: &GenerateContext<'code>,
  source_joiner: &mut SourceJoiner<'code>,
  carried: &'code [(ChunkIdx, RenderedModuleSources)],
) {
  let read = ctx.inline_state.readers_of(ctx.chunk_idx);
  if carried.is_empty() && read.is_empty() {
    return;
  }
  let names = ctx
    .inline_state
    .file_names(ctx.chunk_idx)
    .expect("a file that reads records is named before it is rendered");
  let ascii_only = ctx.options.charset.is_ascii();
  let share_name = escape_non_ascii(runtime_helper_name(ctx, "__share"), ascii_only);
  let require_name = escape_non_ascii(runtime_helper_name(ctx, "__share_require"), ascii_only);
  let export_name = escape_non_ascii(runtime_helper_name(ctx, "__share_export"), ascii_only);
  let exports_param = escape_non_ascii(&names.exports_param, ascii_only);
  let record_id = |record_idx: ChunkIdx| {
    &ctx.inline_state.record(record_idx).expect("chunk should be a record").id
  };

  // A parenthesized factory is a PIFE: the engine compiles it with the file instead of at its
  // first `__share_require`, like rolldown's own module wrappers under the same option.
  let (open, close) = if ctx.options.optimization.is_pife_for_module_wrappers_enabled() {
    (", ((", "}));")
  } else {
    (", (", "});")
  };
  for (record_idx, module_sources) in carried {
    // `__share("id", (exports) => {` and the bridges to the records this record reads.
    let mut head = concat_string!(
      share_name.as_ref(),
      "(",
      to_ecma_string_literal(record_id(*record_idx), ascii_only),
      open,
      exports_param.as_ref(),
      ") => {"
    );
    for other in ctx.inline_state.readers_of(*record_idx) {
      let bridge = names
        .factory_bridges
        .get(record_idx)
        .and_then(|bridges| bridges.get(other))
        .expect("a factory has a bridge for every record it reads");
      head.push_str(&concat_string!(
        "\n\tvar ",
        escape_non_ascii(bridge, ascii_only),
        " = ",
        require_name.as_ref(),
        "(",
        to_ecma_string_literal(record_id(*other), ascii_only),
        ");"
      ));
    }
    // Dev mode: the record's graph rows ship from inside its factory, which runs once per page;
    // a carrier's own prelude lists only its chunk's modules.
    if ctx.options.is_dev_mode_enabled()
      && let Some(rows) = crate::hmr::module_graph_delta::render_register_graph_source(
        &ctx.link_output.module_table,
        ctx.chunk_graph.chunk_table[*record_idx].modules.iter().copied(),
        &ctx.link_output.user_defined_entry_modules,
        None,
      )
    {
      head.push_str(&concat_string!("\n\t", rows));
    }
    source_joiner.append_source(head);
    for module_source in module_sources {
      if let Some(sources) = &module_source.sources {
        for source in sources.as_ref() {
          source_joiner.append_source(source);
        }
      }
    }
    // The getter table publishing the record's exports, then the factory's closing brace.
    let mut tail = String::new();
    let export_items = get_export_items(&ctx.chunk_graph.chunk_table[*record_idx]);
    if !export_items.is_empty() {
      tail.push_str(&concat_string!(
        "\t",
        export_name.as_ref(),
        "(",
        exports_param.as_ref(),
        ", {"
      ));
      for (index, (exported_name, symbol_ref)) in export_items.iter().enumerate() {
        if index > 0 {
          tail.push(',');
        }
        let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(*symbol_ref);
        let value = ctx.finalized_string_pattern_for_symbol_ref(
          canonical_ref,
          *record_idx,
          &ctx.chunk.canonical_names,
        );
        tail.push_str(&concat_string!(
          "\n\t\t",
          object_key(exported_name, ascii_only),
          ": () => ",
          escape_non_ascii(&value, ascii_only)
        ));
      }
      tail.push_str("\n\t});\n");
    }
    tail.push_str(close);
    source_joiner.append_source(tail);
  }

  if !read.is_empty() {
    let mut bridges = String::new();
    for record_idx in read {
      let bridge =
        names.bridges.get(record_idx).expect("a file has a bridge for every record it reads");
      if !bridges.is_empty() {
        bridges.push('\n');
      }
      bridges.push_str(&concat_string!(
        "var ",
        escape_non_ascii(bridge, ascii_only),
        " = ",
        require_name.as_ref(),
        "(",
        to_ecma_string_literal(record_id(*record_idx), ascii_only),
        ");"
      ));
    }
    source_joiner.append_source(bridges);
  }
}
