use oxc::ast::ast::{Comment, CommentKind, Program};
use regex::Regex;
use std::sync::LazyLock;

static REFERENCE_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"/\s*<reference\s+(?:path|types)=").unwrap());

pub fn is_reference_directive(comment: &str) -> bool {
  REFERENCE_RE.is_match(comment)
}

pub fn collect_reference_directives_from_program(program: &Program, source: &str) -> Vec<String> {
  let mut directives = Vec::new();

  for comment in &program.comments {
    let comment_text = extract_comment_text(comment, source);

    if comment.kind == CommentKind::Line && is_reference_directive(&comment_text) {
      directives.push(comment_text);
    }
  }

  directives
}

fn extract_comment_text(comment: &Comment, source: &str) -> String {
  let start = comment.span.start as usize;
  let end = comment.span.end as usize;

  if start < source.len() && end <= source.len() && start < end {
    source[start..end].to_string()
  } else {
    String::new()
  }
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
