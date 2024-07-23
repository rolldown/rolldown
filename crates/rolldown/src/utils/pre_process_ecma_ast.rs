use std::path::Path;

use oxc::minifier::RemoveDeadCode;
use oxc::semantic::{ScopeTree, SymbolTable};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use rolldown_ecmascript::EcmaAst;

use crate::types::oxc_parse_type::OxcParseType;

use super::tweak_ast_for_scanning::tweak_ast_for_scanning;

// #[allow(clippy::match_same_arms)]: `OxcParseType::Tsx` will have special logic to deal with ts compared to `OxcParseType::Jsx`
#[allow(clippy::match_same_arms)]
pub fn pre_process_ecma_ast(
  mut ast: EcmaAst,
  parse_type: &OxcParseType,
  path: &Path,
  source_type: SourceType,
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
    // symbols = ret.symbols;
    // scopes = ret.scopes;
  }

  ast.program.with_mut(|fields| {
    RemoveDeadCode::new(fields.allocator).build(fields.program);
  });

  tweak_ast_for_scanning(&mut ast);

  // We have to re-create the symbol table and scope tree after the transformation so far to make sure they are up-to-date.
  let (symbols, scopes) = ast.make_symbol_table_and_scope_tree();

  Ok((ast, symbols, scopes))
}
