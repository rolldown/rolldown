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

fn export_getter_property<'a>(
  name: &str,
  getter: oxc::ast::ast::Expression<'a>,
  span: oxc::span::Span,
  builder: &oxc::ast::builder::AstBuilder<'a>,
) -> oxc::ast::ast::ObjectPropertyKind<'a> {
  use oxc::ast::ast::{ObjectPropertyKind, PropertyKey, PropertyKind, Str};
  ObjectPropertyKind::new_object_property(
    span,
    PropertyKind::Init,
    PropertyKey::new_string_literal(span, Str::from_str_in(name, builder), None, builder),
    getter,
    false,
    false,
    name == "__proto__",
    builder,
  )
}

/// Construct a carrier's registrations and bridges around already-finalized, rebased bodies.
/// The caller retains every donor arena. Generated nodes use synthetic spans; only donor nodes
/// contribute mappings. See internal-docs/inline-common-chunks/implementation.md.
pub fn inline_records_ast<'a>(
  ctx: &GenerateContext<'_>,
  allocator: &'a oxc::allocator::Allocator,
  generated_span: oxc::span::Span,
  carried: impl IntoIterator<Item = (ChunkIdx, oxc::allocator::Vec<'a, oxc::ast::ast::Statement<'a>>)>,
) -> oxc::allocator::Vec<'a, oxc::ast::ast::Statement<'a>> {
  use oxc::{
    allocator::Vec as ArenaVec,
    ast::{
      ast::{
        Argument, ArrowFunctionBody, BindingPattern, Expression, FormalParameter,
        FormalParameterKind, FormalParameters, FunctionBody, Statement, Str,
      },
      builder::AstBuilder,
    },
  };
  use rolldown_ecmascript_utils::StatementFactoryExt as _;

  struct GeneratedSpan(oxc::span::Span);
  impl oxc::ast_visit::VisitMut<'_> for GeneratedSpan {
    fn visit_span(&mut self, span: &mut oxc::span::Span) {
      *span = self.0;
    }
  }

  let builder = AstBuilder::new(allocator);
  let mut statements = ArenaVec::new_in(&builder);
  let Some(names) = ctx.inline_state.file_names(ctx.chunk_idx) else {
    return statements;
  };
  let identifier = |name: &str| {
    Expression::new_identifier(generated_span, Str::from_str_in(name, &builder), &builder)
  };
  let string = |value: &str| {
    Expression::new_string_literal(
      generated_span,
      Str::from_str_in(value, &builder),
      None,
      &builder,
    )
  };
  let record_id = |record| ctx.inline_state.record(record).expect("inline record").id.as_str();
  let bridge = |name: &str, record| {
    let mut statement = Statement::new_var_decl(
      name,
      Expression::new_call_expression(
        generated_span,
        identifier(runtime_helper_name(ctx, "__share_require")),
        None,
        [Argument::from(string(record_id(record)))],
        false,
        &builder,
      ),
      &builder,
    );
    oxc::ast_visit::VisitMut::visit_statement(&mut GeneratedSpan(generated_span), &mut statement);
    statement
  };

  for (record, body) in carried {
    let mut factory_body = ArenaVec::new_in(&builder);
    for other in ctx.inline_state.readers_of(record) {
      let name = &names.factory_bridges[&record][other];
      factory_body.push(bridge(name, *other));
    }
    if ctx.options.is_dev_mode_enabled() {
      factory_body.extend(crate::hmr::module_graph_delta::register_graph_ast(
        &ctx.link_output.module_table,
        ctx.chunk_graph.chunk_table[record].modules.iter().copied(),
        &ctx.link_output.user_defined_entry_modules,
        allocator,
        generated_span,
      ));
    }
    factory_body.extend(body);
    let export_items = get_export_items(&ctx.chunk_graph.chunk_table[record]);
    if !export_items.is_empty() {
      let properties = ArenaVec::from_iter_in(
        export_items.iter().map(|(name, symbol)| {
          let mut value = ctx.finalized_expr_for_symbol_ref(*symbol, record, &builder);
          // The expression contains generated names only. Use the generated region of V rather
          // than offset zero, which belongs to the first real donor.
          oxc::ast_visit::VisitMut::visit_expression(
            &mut GeneratedSpan(generated_span),
            &mut value,
          );
          let getter = Expression::new_arrow_function_expression(
            generated_span,
            false,
            None,
            FormalParameters::boxed(
              generated_span,
              FormalParameterKind::ArrowFormalParameters,
              [],
              None,
              &builder,
            ),
            None,
            ArrowFunctionBody::from(value),
            &builder,
          );
          export_getter_property(name, getter, generated_span, &builder)
        }),
        &builder,
      );
      let table = Expression::new_object_expression(generated_span, properties, &builder);
      factory_body.push(Statement::new_expression_statement(
        generated_span,
        Expression::new_call_expression(
          generated_span,
          identifier(runtime_helper_name(ctx, "__share_export")),
          None,
          [Argument::from(identifier(&names.exports_param)), Argument::from(table)],
          false,
          &builder,
        ),
        &builder,
      ));
    }
    let params = FormalParameters::boxed(
      generated_span,
      FormalParameterKind::ArrowFormalParameters,
      [FormalParameter::new(
        generated_span,
        [],
        BindingPattern::new_binding_identifier(
          generated_span,
          Str::from_str_in(&names.exports_param, &builder),
          &builder,
        ),
        None,
        None,
        false,
        None,
        false,
        false,
        &builder,
      )],
      None,
      &builder,
    );
    let factory = Expression::new_arrow_function_expression(
      generated_span,
      false,
      None,
      params,
      None,
      ArrowFunctionBody::FunctionBody(FunctionBody::boxed(
        generated_span,
        [],
        factory_body,
        &builder,
      )),
      &builder,
    );
    statements.push(Statement::new_expression_statement(
      generated_span,
      Expression::new_call_expression(
        generated_span,
        identifier(runtime_helper_name(ctx, "__share")),
        None,
        [Argument::from(string(record_id(record))), Argument::from(factory)],
        false,
        &builder,
      ),
      &builder,
    ));
  }
  for record in ctx.inline_state.readers_of(ctx.chunk_idx) {
    statements.push(bridge(&names.bridges[record], *record));
  }
  statements
}

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

  for (record_idx, module_sources) in carried {
    // `__share("id", (exports) => {` and the bridges to the records this record reads.
    let mut head = concat_string!(
      share_name.as_ref(),
      "(",
      to_ecma_string_literal(record_id(*record_idx), ascii_only),
      ", (",
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
    tail.push_str("});");
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

#[cfg(test)]
mod tests {
  #[test]
  fn proto_export_getter_is_an_own_property() {
    use oxc::{
      allocator::Allocator,
      ast::{
        ast::{Expression, Program, Statement},
        builder::AstBuilder,
      },
      codegen::Codegen,
      span::{SPAN, SourceType},
    };
    let allocator = Allocator::default();
    let builder = AstBuilder::new(&allocator);
    let getter =
      rolldown_ecmascript_utils::parse_injected_expression(&allocator, "() => 37").unwrap();
    let object = Expression::new_object_expression(
      SPAN,
      [super::export_getter_property("__proto__", getter, SPAN, &builder)],
      &builder,
    );
    let program = Program::new(
      SPAN,
      SourceType::mjs(),
      "",
      [],
      None,
      [],
      [Statement::new_expression_statement(SPAN, object, &builder)],
      &builder,
    );
    let code = Codegen::new().build(&program).code;
    let execution = std::process::Command::new("node").args(["--input-type=module", "--eval", &format!("import vm from 'node:vm'; import assert from 'node:assert/strict'; const table = vm.runInNewContext({}); assert.equal(Object.hasOwn(table, '__proto__'), true); assert.equal(table.__proto__(), 37);", serde_json::to_string(&code).unwrap())]).output().unwrap();
    assert!(execution.status.success(), "{code}\n{}", String::from_utf8_lossy(&execution.stderr));
  }
}
