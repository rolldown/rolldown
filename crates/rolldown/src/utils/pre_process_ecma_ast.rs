use std::path::Path;

use oxc::ast::VisitMut;
use oxc::minifier::{
  CompressOptions, Compressor, ReplaceGlobalDefines, ReplaceGlobalDefinesConfig,
};
use oxc::semantic::{ScopeTree, SymbolTable};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use rolldown_ecmascript::EcmaAst;

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
) -> anyhow::Result<(EcmaAst, SymbolTable, ScopeTree)> {
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
      .build(fields.program)
    });

    if !ret.errors.is_empty() {
      return Err(anyhow::anyhow!("Transform failed, got {:#?}", ret.errors));
    }
  }

  ast.program.with_mut(|fields| -> anyhow::Result<()> {
    if let Some(replace_global_define_config) = replace_global_define_config {
      ReplaceGlobalDefines::new(fields.allocator, replace_global_define_config.clone())
        .build(fields.program);
    }
    let options = CompressOptions::dead_code_elimination();
    Compressor::new(fields.allocator, options).build(fields.program);

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
