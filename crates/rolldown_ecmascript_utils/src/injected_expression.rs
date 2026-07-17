use oxc::allocator::{Allocator, TakeIn};
use oxc::ast::ast::{Expression, Statement};
use oxc::ast_visit::VisitMut;
use oxc::parser::{ParseOptions, Parser};
use oxc::span::{SPAN, SourceType, Span};

/// Zeroes every `Span` it reaches. Every generated `walk_*` calls `visit_span`,
/// so implementing this one method covers the whole subtree.
struct SpanZeroer;

impl VisitMut<'_> for SpanZeroer {
  fn visit_span(&mut self, it: &mut Span) {
    *it = SPAN;
  }
}

fn parse_in<'ast>(allocator: &'ast Allocator, code: &str) -> Result<Expression<'ast>, String> {
  // Parenthesized so `{ a: 1 }` parses as an object literal rather than a block
  // statement. Allocated into the arena so the AST's `&'ast str` slices outlive this
  // function.
  //
  // `parse()` rather than `parse_expression()`: the latter stops at the first
  // expression and silently discards whatever follows, so `1; 2` yields `1` and
  // `foo() } evil` yields `foo()`. Parsing a whole program lets trailing garbage
  // surface as a diagnostic or an extra statement, and fail the build.
  // The `)` sits on its own line so a trailing `//` comment in `code` can't swallow
  // it (`42 // note` must still parse).
  let source = allocator.alloc_str(&format!("({code}\n)"));
  // `preserve_parens: false` drops the wrapper we just added, so `(1, 2)` yields a
  // `SequenceExpression` rather than a `ParenthesizedExpression` around one. Codegen
  // then re-parenthesizes by precedence wherever the expression is spliced in.
  let ret = Parser::new(allocator, source, SourceType::mjs())
    .with_options(ParseOptions { preserve_parens: false, ..ParseOptions::default() })
    .parse();
  if !ret.diagnostics.is_empty() {
    return Err(ret.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"));
  }

  let mut program = ret.program;
  let [Statement::ExpressionStatement(stmt)] = program.body.as_mut_slice() else {
    return Err("expected a single expression".to_string());
  };
  Ok(stmt.expression.take_in(&allocator))
}

/// Parses `code` as a single JS expression into `allocator`, with all spans zeroed.
///
/// # Errors
/// Returns the parser diagnostics if `code` is not exactly one expression.
pub fn parse_injected_expression<'ast>(
  allocator: &'ast Allocator,
  code: &str,
) -> Result<Expression<'ast>, String> {
  let mut expr = parse_in(allocator, code)?;
  SpanZeroer.visit_expression(&mut expr);
  Ok(expr)
}

#[cfg(test)]
mod tests {
  use super::parse_injected_expression;
  use oxc::allocator::Allocator;
  use oxc::ast::ast::Expression;
  use oxc::ast_visit::{Visit, walk};
  use oxc::span::Span;

  #[derive(Default)]
  struct MaxSpanEnd(u32);
  impl Visit<'_> for MaxSpanEnd {
    fn visit_span(&mut self, it: &Span) {
      self.0 = self.0.max(it.end);
      walk::walk_span(self, it);
    }
  }

  #[test]
  fn parses_a_string_literal() {
    let allocator = Allocator::default();
    let expr = parse_injected_expression(&allocator, "'resolved'").expect("should parse");
    let Expression::StringLiteral(lit) = expr else { panic!("expected a string literal") };
    assert_eq!(lit.value.as_str(), "resolved");
  }

  #[test]
  fn parses_a_comma_expression_as_one_expression() {
    // Kept whole rather than split, which is what makes the precedence divergence safe.
    let allocator = Allocator::default();
    let expr = parse_injected_expression(&allocator, "1, 2").expect("should parse");
    assert!(matches!(expr, Expression::SequenceExpression(_)));
  }

  #[test]
  fn parses_code_with_a_trailing_line_comment() {
    let allocator = Allocator::default();
    let expr =
      parse_injected_expression(&allocator, "42 // trailing comment").expect("should parse");
    assert!(matches!(expr, Expression::NumericLiteral(_)));
  }

  #[test]
  fn parses_an_object_literal_rather_than_a_block() {
    let allocator = Allocator::default();
    let expr = parse_injected_expression(&allocator, "{ a: 1 }").expect("should parse");
    assert!(matches!(expr, Expression::ObjectExpression(_)));
  }

  #[test]
  fn zeroes_every_span() {
    let allocator = Allocator::default();
    let expr = parse_injected_expression(&allocator, "new URL('a.txt', import.meta.url).href")
      .expect("should parse");
    let mut max = MaxSpanEnd::default();
    max.visit_expression(&expr);
    assert_eq!(max.0, 0, "all spans must be zeroed so sourcemaps stay honest");
  }

  fn rejects(code: &str) -> bool {
    let allocator = Allocator::default();
    parse_injected_expression(&allocator, code).is_err()
  }

  #[test]
  fn rejects_a_syntax_error() {
    assert!(rejects("'unterminated"));
  }

  #[test]
  fn rejects_a_statement() {
    assert!(rejects("const a = 1;"));
  }

  #[test]
  fn rejects_a_comment_with_no_expression() {
    assert!(rejects("// only a comment"));
  }

  #[test]
  fn rejects_trailing_code_rather_than_silently_dropping_it() {
    // `Parser::parse_expression` would return `1` and `foo()` here, discarding the
    // rest without a word. Parsing a whole program refuses instead.
    assert!(rejects("1; 2"));
    assert!(rejects("foo() } evil"));
  }
}
