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
  let input = alloc.alloc_str(input);
  Parser::new(alloc, input, SourceType::default())
    .parse_expression()
    .unwrap_or_else(|e| panic!("Failed to parse {input:?} into expression. Got {e:#?}"))
}

pub fn quote_stmts<'alloc>(
  alloc: &'alloc Allocator,
  input: &str,
) -> allocator::Vec<'alloc, ast::Statement<'alloc>> {
  let input = alloc.alloc_str(input);
  let p = Parser::new(alloc, input, SourceType::default()).parse();
  assert!(
    !p.panicked && p.errors.is_empty(),
    "Failed to parse {:?} into statements. Got {:#?}",
    input,
    p.errors
  );
  p.program.body
}

pub fn quote_stmt<'alloc>(alloc: &'alloc Allocator, input: &str) -> ast::Statement<'alloc> {
  let mut stmts = quote_stmts(alloc, input);
  assert_eq!(stmts.len(), 1, "Expected exactly one statement, got {}", stmts.len());
  stmts.pop().unwrap()
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
