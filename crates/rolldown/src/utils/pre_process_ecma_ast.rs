use std::path::Path;

use oxc::ast::VisitMut;
use oxc::minifier::{
  CompressOptions, Compressor, InjectGlobalVariables, ReplaceGlobalDefines,
  ReplaceGlobalDefinesConfig,
};
use oxc::semantic::{ScopeTree, SemanticBuilder, SymbolTable};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};

use rolldown_common::NormalizedBundlerOptions;
use rolldown_ecmascript::{EcmaAst, WithMutFields};

use crate::types::oxc_parse_type::OxcParseType;

use super::ecma_visitors::EnsureSpanUniqueness;
use super::tweak_ast_for_scanning::tweak_ast_for_scanning;

// #[allow(clippy::match_same_arms)]: `OxcParseType::Tsx` will have special logic to deal with ts compared to `OxcParseType::Jsx`
#[allow(clippy::match_same_arms)]
pub fn pre_process_ecma_ast(
  mut ast: EcmaAst,
  parse_type: &OxcParseType,
  path: &Path,
  source_type: SourceType,
  replace_global_define_config: Option<&ReplaceGlobalDefinesConfig>,
  bundle_options: &NormalizedBundlerOptions,
) -> anyhow::Result<(EcmaAst, SymbolTable, ScopeTree)> {
  // Build initial semantic data and check for semantic errors.
  let semantic_ret = ast.program.with_mut(|WithMutFields { program, source, .. }| {
    SemanticBuilder::new(source, source_type).build(program)
  });

  // TODO:
  // if !semantic_ret.errors.is_empty() {
  // return Err(anyhow::anyhow!("Semantic Error: {:#?}", semantic_ret.errors));
  // }

  let (mut symbols, mut scopes) = semantic_ret.semantic.into_symbol_table_and_scope_tree();

  // Transform TypeScript and jsx.
  if !matches!(parse_type, OxcParseType::Js) {
    let trivias = ast.trivias.clone();
    let ret = ast.program.with_mut(move |fields| {
      let mut transformer_options = TransformOptions::default();
      match parse_type {
        OxcParseType::Js => unreachable!("Should not reach here"),
        OxcParseType::Jsx | OxcParseType::Tsx => {
          transformer_options.react.jsx_plugin = true;
        }
        OxcParseType::Ts => {}
      }

      Transformer::new(
        fields.allocator,
        path,
        source_type,
        fields.source,
        trivias,
        transformer_options,
      )
      .build_with_symbols_and_scopes(symbols, scopes, fields.program)
    });

    if !ret.errors.is_empty() {
      return Err(anyhow::anyhow!("Transform failed, got {:#?}", ret.errors));
    }

    symbols = ret.symbols;
    scopes = ret.scopes;
  }

  ast.program.with_mut(|WithMutFields { allocator, program, .. }| -> anyhow::Result<()> {
    // Use built-in define plugin.
    if let Some(replace_global_define_config) = replace_global_define_config {
      ReplaceGlobalDefines::new(allocator, replace_global_define_config.clone()).build(program);
    }

    if !bundle_options.inject.is_empty() {
      InjectGlobalVariables::new(
        allocator,
        bundle_options.oxc_inject_global_variables_config.clone(),
      )
      .build(&mut symbols, &mut scopes, program);
    }

    // Perform dead code elimination.
    let options = CompressOptions::dead_code_elimination();
    Compressor::new(allocator, options).build(program);

    Ok(())
  })?;

  tweak_ast_for_scanning(&mut ast);

  ast.program.with_mut(|fields| {
    EnsureSpanUniqueness::new().visit_program(fields.program);
  });

  // We have to re-create the symbol table and scope tree after the transformation so far to make sure they are up-to-date.
  let (symbols, scopes) = ast.make_symbol_table_and_scope_tree();

  Ok((ast, symbols, scopes))
}
