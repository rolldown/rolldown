mod allocator_helpers;
mod ast_snippet;
mod compiler;
mod ecma_ast;
mod ext;

pub use crate::{
  allocator_helpers::{from_in::FromIn, into_in::IntoIn, take_in::TakeIn},
  ast_snippet::AstSnippet,
  compiler::EcmaCompiler,
  ecma_ast::{program_cell::WithMutFields, EcmaAst},
  ext::{
    allocator_ext::AllocatorExt, span_ext::SpanExt, BindingIdentifierExt, BindingPatternExt,
    ExpressionExt, StatementExt,
  },
};
