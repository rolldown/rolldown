use oxc::allocator::{self, Allocator};
use oxc::parser::Parser;
use oxc::span::SourceType;

pub fn quote_expr<'alloc>(
  alloc: &'alloc Allocator,
  input: &str,
) -> oxc::ast::ast::Expression<'alloc> {
  let alloc_input = allocator::String::from_str_in(input, alloc).into_bump_str();
  Parser::new(alloc, alloc_input, SourceType::default())
    .parse_expression()
    .unwrap_or_else(|e| panic!("Failed to parse {:?} into expression. Got {:#?}", alloc_input, e))
}
