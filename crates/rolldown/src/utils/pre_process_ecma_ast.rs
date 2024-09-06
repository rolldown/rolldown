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
  // Only recreate semantic data if ast is changed.
  let mut ast_changed = false;

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
    ast_changed = true;
  }

  ast.program.with_mut(|WithMutFields { allocator, program, .. }| -> anyhow::Result<()> {
    // Use built-in define plugin.
    if let Some(replace_global_define_config) = replace_global_define_config {
      ReplaceGlobalDefines::new(allocator, replace_global_define_config.clone()).build(program);
      ast_changed = true;
    }

    if !bundle_options.inject.is_empty() {
      InjectGlobalVariables::new(
        allocator,
        bundle_options.oxc_inject_global_variables_config.clone(),
      )
      .build(&mut symbols, &mut scopes, program);
      ast_changed = true;
    }

    // Perform dead code elimination.
    // NOTE: `CompressOptions::dead_code_elimination` will remove `ParenthesizedExpression`s from the AST.
    let compressor = Compressor::new(allocator, CompressOptions::dead_code_elimination());
    if ast_changed {
      // This method recreates symbols and scopes.
      compressor.build(program);
    } else {
      compressor.build_with_symbols_and_scopes(symbols, scopes, program);
    }

    Ok(())
  })?;

  tweak_ast_for_scanning(&mut ast);

  ast.program.with_mut(|fields| {
    EnsureSpanUniqueness::new().visit_program(fields.program);
  });

  // NOTE: Recreate semantic data because AST is changed in the transformations above.
  let (symbols, scopes) = ast.make_symbol_table_and_scope_tree();

  Ok((ast, symbols, scopes))
}
