mod allocator_helpers;
mod ast_snippet;
mod compiler;
mod ext;
mod oxc_ast;

pub use crate::{
  allocator_helpers::{from_in::FromIn, into_in::IntoIn, take_in::TakeIn},
  ast_snippet::AstSnippet,
  compiler::OxcCompiler,
  ext::{BindingIdentifierExt, BindingPatternExt, ExpressionExt, StatementExt},
  oxc_ast::{OxcAst, WithFieldsMut},
};
