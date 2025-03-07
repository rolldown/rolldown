/// This module provides a way to create AST nodes from a string input. __These methods should only be used in prototyping__.
use oxc::parser::Parser;
use oxc::span::SourceType;
use oxc::{
  allocator::{self, Allocator},
  ast::ast,
};

pub fn quote_expr<'alloc>(
  alloc: &'alloc Allocator,
  input: &str,
) -> oxc::ast::ast::Expression<'alloc> {
  let alloc_input = allocator::String::from_str_in(input, alloc).into_bump_str();
  Parser::new(alloc, alloc_input, SourceType::default())
    .parse_expression()
    .unwrap_or_else(|e| panic!("Failed to parse {alloc_input:?} into expression. Got {e:#?}"))
}

pub fn quote_stmts<'alloc>(
  alloc: &'alloc Allocator,
  input: &str,
) -> allocator::Vec<'alloc, ast::Statement<'alloc>> {
  let alloc_input = allocator::String::from_str_in(input, alloc).into_bump_str();
  let p = Parser::new(alloc, alloc_input, SourceType::default()).parse();
  assert!(
    !p.panicked && p.errors.is_empty(),
    "Failed to parse {:?} into statements. Got {:#?}",
    alloc_input,
    p.errors
  );
  p.program.body
}

#[test]
fn test_quote_expr() {
  let alloc = Allocator::new();
  let _expr = quote_expr(&alloc, "1 + 2");
}

#[test]
fn test_quote_stmts() {
  let alloc = Allocator::new();
  let _stmts = quote_stmts(&alloc, "1 + 2;");
}
