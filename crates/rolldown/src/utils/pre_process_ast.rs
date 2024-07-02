use std::path::Path;

use oxc::minifier::{RemoveDeadCode, ReplaceGlobalDefines};
use oxc::semantic::{ScopeTree, SymbolTable};
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use rolldown_common::NormalizedBundlerOptions;
use rolldown_oxc_utils::OxcAst;

use crate::types::oxc_parse_type::OxcParseType;

use super::fold_const_value::BasicInlineBinaryValue;
use super::tweak_ast_for_scanning::tweak_ast_for_scanning;

// #[allow(clippy::match_same_arms)]: `OxcParseType::Tsx` will have special logic to deal with ts compared to `OxcParseType::Jsx`
#[allow(clippy::match_same_arms)]
pub fn pre_process_ast(
  mut ast: OxcAst,
  parse_type: &OxcParseType,
  path: &Path,
  source_type: SourceType,
  options: &NormalizedBundlerOptions,
) -> anyhow::Result<(OxcAst, SymbolTable, ScopeTree)> {
  ast.program.with_mut(|fields| {
    ReplaceGlobalDefines::new(fields.allocator, options.defines.clone()).build(fields.program);
  });

  let (mut symbols, mut scopes) = ast.make_symbol_table_and_scope_tree();

  if !matches!(parse_type, OxcParseType::Js) {
    let trivias = ast.trivias.clone();
    let ret = ast.program.with_mut(move |fields| {
      let mut transformer_options = TransformOptions::default();
      match parse_type {
        OxcParseType::Js => unreachable!("Should not reach here"),
        OxcParseType::Jsx => {
          transformer_options.react.jsx_plugin = true;
        }
        OxcParseType::Ts => {}
        OxcParseType::Tsx => {
          transformer_options.react.jsx_plugin = true;
        }
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

  ast.program.with_mut(|fields| {
    // Inline binary value . e.g. `process.env.NODE_ENV === "production"` -> `true` or `false`
    BasicInlineBinaryValue::new(fields.allocator).build(fields.program);
    RemoveDeadCode::new(fields.allocator).build(fields.program);
  });

  tweak_ast_for_scanning(&mut ast);

  Ok((ast, symbols, scopes))
}
