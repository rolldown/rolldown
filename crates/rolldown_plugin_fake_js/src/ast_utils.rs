use std::sync::LazyLock;

use oxc::ast::ast::{CommentKind, Program};
use regex::Regex;

use crate::codegen::extract_span_text;

static REFERENCE_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"/\s*<reference\s+(?:path|types)=").unwrap());

pub fn is_reference_directive(comment: &str) -> bool {
  REFERENCE_RE.is_match(comment)
}

pub fn collect_reference_directives_from_program(program: &Program, source: &str) -> Vec<String> {
  let mut directives = Vec::new();

  for comment in &program.comments {
    let comment_text = extract_span_text(source, comment.span);

    if comment.kind == CommentKind::Line && is_reference_directive(&comment_text) {
      directives.push(comment_text);
    }
  }

  directives
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_reference_directive() {
    assert!(is_reference_directive("/// <reference path=\"foo\" />"));
    assert!(is_reference_directive("/// <reference types=\"bar\" />"));
    assert!(!is_reference_directive("// regular comment"));
  }
}
