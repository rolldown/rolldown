mod ast_snippet;
mod extensions;
mod scope;
mod source_utils;

pub use crate::{
  ast_snippet::AstSnippet,
  extensions::ast_ext::{
    binding_identifier_ext::BindingIdentifierExt, binding_pattern_ext::BindingPatternExt,
    call_expression_ext::CallExpressionExt, expression_ext::ExpressionExt, function::FunctionExt,
    jsx::JsxExt, statement_ext::StatementExt,
  },
  scope::is_top_level,
  source_utils::contains_script_closing_tag,
};
