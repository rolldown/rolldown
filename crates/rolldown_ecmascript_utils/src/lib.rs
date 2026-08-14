mod ast_factory;
mod extensions;
mod injected_expression;
mod scope;
mod source_utils;

pub use crate::{
  ast_factory::{
    BindingIdentifierFactoryExt, CallExpressionFactoryExt, ClassElementFactoryExt,
    EsmWrapperBodyKind, EsmWrapperCallKind, EsmWrapperDeclKind, EsmWrapperStmtOptions,
    ExpressionFactoryExt, IdentifierNameFactoryExt, MemberExpressionFactoryExt,
    ObjectPropertyKindFactoryExt, StatementFactoryExt,
  },
  extensions::ast_ext::{
    binding_pattern_ext::BindingPatternExt,
    binding_property_ext::BindingPropertyExt,
    call_expression_ext::CallExpressionExt,
    expression_ext::ExpressionExt,
    function::FunctionExt,
    jsx::{JsxExt, JsxMemberExpressionObjectExt},
    statement_ext::StatementExt,
  },
  injected_expression::parse_injected_expression,
  scope::is_top_level,
  source_utils::contains_script_closing_tag,
};
