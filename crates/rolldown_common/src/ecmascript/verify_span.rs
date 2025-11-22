use anyhow::{Result, anyhow};
use oxc::{
  ast::{AstKind, ast::Program},
  ast_visit::Visit,
  span::GetSpan,
};

pub struct SpanVerifier {
  invalid_span_nodes: Vec<String>,
}

impl SpanVerifier {
  pub fn verify(ast: &Program<'_>) -> Result<()> {
    let mut verifier = Self { invalid_span_nodes: Vec::new() };
    verifier.visit_program(ast);
    if verifier.invalid_span_nodes.is_empty() {
      Ok(())
    } else {
      Err(anyhow!("Invalid span nodes: {:?}", verifier.invalid_span_nodes))
    }
  }
}

impl<'a> Visit<'a> for SpanVerifier {
  fn enter_node(&mut self, kind: AstKind<'a>) {
    let span = kind.span();
    if span.start > span.end {
      self.invalid_span_nodes.push(format!("{kind:?}"));
    }
  }
}
