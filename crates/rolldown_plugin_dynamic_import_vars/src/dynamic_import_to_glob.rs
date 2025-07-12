// Ported from https://github.com/rollup/plugins/blob/944e7d3/packages/dynamic-import-vars/src/dynamic-import-to-glob.js
use oxc::{
  ast::ast::{Argument, BinaryExpression, CallExpression, Expression, TemplateLiteral},
  syntax::operator::BinaryOperator,
};
use std::{borrow::Cow, path::Path};

const EXAMPLE_CODE: &str = "For example: import(`./foo/${bar}.js`).";
const IGNORED_PROTOCOLS: [&str; 3] = ["data:", "http:", "https:"];
const SPECIAL_PARAMS: [&str; 4] = ["raw", "sharedworker", "url", "worker"];

#[inline]
pub fn has_special_query_param(query: &str) -> bool {
  if query.len() < 2 {
    return false;
  }
  query[1..].split('&').any(|param| SPECIAL_PARAMS.contains(&param))
}

#[inline]
pub fn should_ignore(glob: &str) -> bool {
  IGNORED_PROTOCOLS.into_iter().any(|protocol| glob.starts_with(protocol))
}

pub fn to_valid_glob<'a>(glob: &'a str, source: &str) -> anyhow::Result<Cow<'a, str>> {
  if !glob.starts_with("./") && !glob.starts_with("../") {
    Err(anyhow::anyhow!(
      "Invalid import {source}. Variable bare imports are not supported, imports must start with ./ in the static part of the import. {EXAMPLE_CODE}"
    ))?;
  }

  if Path::new(glob).extension().is_none() {
    Err(anyhow::anyhow!(
      "Invalid import {source}. A file extension must be included in the static part of the import. {EXAMPLE_CODE}"
    ))?;
  }

  Ok(if glob.contains(['?', '[', ']', '{', '}']) {
    let mut escaped = String::with_capacity(glob.len());
    for c in glob.chars() {
      match c {
        '?' | '[' | ']' | '{' | '}' => {
          escaped.push('[');
          escaped.push(c);
          escaped.push(']');
        }
        c => {
          escaped.push(c);
        }
      }
    }
    Cow::Owned(escaped)
  } else {
    Cow::Borrowed(glob)
  })
}

pub fn template_literal_to_glob<'a>(node: &TemplateLiteral) -> anyhow::Result<Cow<'a, str>> {
  let mut glob = String::new();
  for (index, quasi) in node.quasis.iter().enumerate() {
    glob += &sanitize_string(&quasi.value.raw)?;
    if let Some(expr) = node.expressions.get(index) {
      glob += &expr_to_glob(expr)?;
    }
  }
  Ok(Cow::Owned(glob))
}

fn expr_to_glob<'a>(expr: &'a Expression) -> anyhow::Result<Cow<'a, str>> {
  match expr {
    Expression::TemplateLiteral(node) => template_literal_to_glob(node),
    Expression::CallExpression(node) => call_expr_to_glob(node),
    Expression::BinaryExpression(node) => binary_expr_to_glob(node),
    Expression::StringLiteral(node) => sanitize_string(&node.value),
    _ => Ok(Cow::Borrowed("*")),
  }
}

fn sanitize_string(s: &str) -> anyhow::Result<Cow<'_, str>> {
  if s.contains('*') {
    Err(anyhow::anyhow!("A dynamic import cannot contain * characters."))?;
  }
  Ok(Cow::Borrowed(s))
}

fn call_expr_to_glob<'a>(node: &'a CallExpression) -> anyhow::Result<Cow<'a, str>> {
  if let Expression::StaticMemberExpression(member_expr) = &node.callee {
    if member_expr.property.name == "concat" {
      if node.arguments.is_empty() {
        return expr_to_glob(&member_expr.object);
      }
      let mut glob = expr_to_glob(&member_expr.object)?.into_owned();
      for arg in &node.arguments {
        let part = match arg {
          Argument::SpreadElement(_) => "*",
          _ => &expr_to_glob(arg.to_expression())?,
        };
        glob += part;
      }
      return Ok(Cow::Owned(glob));
    }
  }
  Ok(Cow::Borrowed("*"))
}

fn binary_expr_to_glob<'a>(node: &'a BinaryExpression) -> anyhow::Result<Cow<'a, str>> {
  if node.operator != BinaryOperator::Addition {
    return Err(anyhow::anyhow!("{:?} operator is not supported.", node.operator.as_str()));
  }
  let left = expr_to_glob(&node.left)?;
  let right = expr_to_glob(&node.right)?;
  Ok(Cow::Owned(rolldown_utils::concat_string!(left, right)))
}

#[cfg(test)]
mod tests {
  use cow_utils::CowUtils;
  use oxc::{allocator::Allocator, parser::Parser, span::SourceType};

  use super::*;

  struct ExprParser {
    allocator: Allocator,
  }

  impl<'a> ExprParser {
    fn new() -> Self {
      let allocator = Allocator::default();
      ExprParser { allocator }
    }

    fn parse(&'a self, source_text: &'a str) -> Expression<'a> {
      let parser = Parser::new(&self.allocator, source_text, SourceType::default());
      parser.parse_expression().unwrap()
    }
  }

  fn to_glob_pattern<'a>(expr: &'a Expression, source: &'a str) -> anyhow::Result<Option<String>> {
    let glob = expr_to_glob(expr)?;
    let glob = glob.cow_replace("**", "*");
    if should_ignore(&glob) || !glob.contains('*') {
      return Ok(None);
    }
    Ok(Some(to_valid_glob(&glob, source)?.into_owned()))
  }

  #[test]
  fn template_literal_with_variable_filename() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./foo/${bar}.js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn external() {
    let parser = ExprParser::new();
    let ast = parser.parse("`https://some.cdn.com/package/${version}/index.js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert!(glob.is_none());
  }

  #[test]
  fn external_leaves_bare_module_specifiers_starting_with_https_in_tact() {
    let parser = ExprParser::new();
    let ast = parser.parse("'http_utils'");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert!(glob.is_none());
  }

  #[test]
  fn data_uri() {
    let parser = ExprParser::new();
    let ast = parser.parse("`data:${bar}`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert!(glob.is_none());
  }

  #[test]
  fn template_literal_with_dot_prefixed_suffix() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${bar}.entry.js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./*.entry.js");
  }

  #[test]
  fn template_literal_with_variable_directory() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./foo/${bar}/x.js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./foo/*/x.js");
  }

  #[test]
  fn template_literal_with_multiple_variables() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${foo}/${bar}.js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./*/*.js");
  }

  #[test]
  fn dynamic_expression_with_variable_filename() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./foo/'.concat(bar,'.js')");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn dynamic_expression_with_variable_directory() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./foo/'.concat(bar, '/x.js')");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./foo/*/x.js");
  }

  #[test]
  fn dynamic_expression_with_multiple_variables() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./'.concat(foo, '/').concat(bar,'.js')");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./*/*.js");
  }

  #[test]
  fn string_concatenation() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./foo/' + bar + '.js'");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn string_concatenation_and_template_literals_combined() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./' + `foo/${bar}` + '.js'");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn string_literal_in_a_template_literal_expression() {
    let parser = ExprParser::new();
    let ast = parser.parse("`${'./foo/'}${bar}.js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn multiple_variables_are_collapsed_into_a_single_star() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./foo/${bar}${baz}/${x}${y}.js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    assert_eq!(glob.unwrap(), "./foo/*/*.js");
  }

  #[test]
  fn throws_when_dynamic_import_contains_a_star() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./*${foo}.js`");
    let err = to_glob_pattern(&ast, "").unwrap_err();
    assert_eq!(err.to_string(), "A dynamic import cannot contain * characters.");
  }

  #[test]
  fn throws_when_dynamic_import_contains_a_non_add_operator() {
    let parser = ExprParser::new();
    let ast = parser.parse("'foo' - 'bar.js'");
    let err = to_glob_pattern(&ast, "").unwrap_err().to_string();
    assert_eq!(err, "\"-\" operator is not supported.");
  }

  #[test]
  fn throws_when_dynamic_import_imports_does_not_contain_a_file_extension() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./foo/${bar}`");
    let err = to_glob_pattern(&ast, "\"`./foo/${bar}`\"").unwrap_err().to_string();
    assert_eq!(
      err,
      "Invalid import \"`./foo/${bar}`\". A file extension must be included in the static part of the import. For example: import(`./foo/${bar}.js`)."
    );
  }

  #[test]
  fn escapes_round_brackets() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${foo}/(foo).js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    // The escaped glob in JS is "./*/\\(foo\\).js"
    assert_eq!(glob.unwrap(), "./*/(foo).js");
  }

  #[test]
  fn escapes_square_brackets() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${foo}/[foo].js`");
    let glob = to_glob_pattern(&ast, "").unwrap();
    // The escaped glob in JS is "./*/\\[foo\\].js"
    assert_eq!(glob.unwrap(), "./*/[[]foo[]].js");
  }
}
