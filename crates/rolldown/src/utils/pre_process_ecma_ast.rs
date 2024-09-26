use std::path::Path;

use oxc::ast::VisitMut;
use oxc::minifier::{
  CompressOptions, Compressor, InjectGlobalVariables, ReplaceGlobalDefines,
  ReplaceGlobalDefinesConfig,
};
use oxc::semantic::{ScopeTree, SemanticBuilder, Stats, SymbolTable};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};

use rolldown_common::NormalizedBundlerOptions;
use rolldown_ecmascript::{EcmaAst, WithMutFields};

use crate::types::oxc_parse_type::OxcParseType;

use super::ecma_visitors::EnsureSpanUniqueness;
use super::tweak_ast_for_scanning::tweak_ast_for_scanning;

#[derive(Default)]
pub struct PreProcessEcmaAst {
  /// Only recreate semantic data if ast is changed.
  ast_changed: bool,

  /// Semantic statistics.
  stats: Stats,
}

impl PreProcessEcmaAst {
  // #[allow(clippy::match_same_arms)]: `OxcParseType::Tsx` will have special logic to deal with ts compared to `OxcParseType::Jsx`
  #[allow(clippy::match_same_arms)]
  pub fn build(
    &mut self,
    mut ast: EcmaAst,
    parse_type: &OxcParseType,
    path: &Path,
    source_type: SourceType,
    replace_global_define_config: Option<&ReplaceGlobalDefinesConfig>,
    bundle_options: &NormalizedBundlerOptions,
  ) -> anyhow::Result<(EcmaAst, SymbolTable, ScopeTree)> {
    // Build initial semantic data and check for semantic errors.
    let semantic_ret = ast.program.with_mut(|WithMutFields { program, source, .. }| {
      SemanticBuilder::new(source).build(program)
    });

    // TODO:
    // if !semantic_ret.errors.is_empty() {
    // return Err(anyhow::anyhow!("Semantic Error: {:#?}", semantic_ret.errors));
    // }

    self.stats = semantic_ret.semantic.stats();
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
      self.ast_changed = true;
    }

    ast.program.with_mut(|WithMutFields { allocator, program, source }| -> anyhow::Result<()> {
      // Use built-in define plugin.
      if let Some(replace_global_define_config) = replace_global_define_config {
        let ret = ReplaceGlobalDefines::new(allocator, replace_global_define_config.clone())
          .build(symbols, scopes, program);
        symbols = ret.symbols;
        scopes = ret.scopes;
        self.ast_changed = true;
      }

      if !bundle_options.inject.is_empty() {
        let ret = InjectGlobalVariables::new(
          allocator,
          bundle_options.oxc_inject_global_variables_config.clone(),
        )
        .build(symbols, scopes, program);
        symbols = ret.symbols;
        scopes = ret.scopes;
        self.ast_changed = true;
      }

      if bundle_options.treeshake.enabled() {
        // Perform dead code elimination.
        // NOTE: `CompressOptions::dead_code_elimination` will remove `ParenthesizedExpression`s from the AST.
        let compressor = Compressor::new(allocator, CompressOptions::dead_code_elimination());
        if self.ast_changed {
          let semantic_ret = SemanticBuilder::new(source).with_stats(self.stats).build(program);
          (symbols, scopes) = semantic_ret.semantic.into_symbol_table_and_scope_tree();
        }
        compressor.build_with_symbols_and_scopes(symbols, scopes, program);
      }

      Ok(())
    })?;

    tweak_ast_for_scanning(&mut ast);

    ast.program.with_mut(|fields| {
      EnsureSpanUniqueness::new().visit_program(fields.program);
    });

    // NOTE: Recreate semantic data because AST is changed in the transformations above.
    (symbols, scopes) = ast.program.with_dependent(|owner, dep| {
      SemanticBuilder::new(&owner.source)
        // Required by `module.scope.get_child_ids` in `crates/rolldown/src/utils/renamer.rs`.
        .with_scope_tree_child_ids(true)
        // Preallocate memory for the underlying data structures.
        .with_stats(self.stats)
        .build(&dep.program)
        .semantic
        .into_symbol_table_and_scope_tree()
    });

    Ok((ast, symbols, scopes))
  }
}
