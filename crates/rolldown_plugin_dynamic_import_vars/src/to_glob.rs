// Ported from https://github.com/rollup/plugins/blob/944e7d3ec4375371a2e70a55ac07cab4c61dc8b6/packages/dynamic-import-vars/src/dynamic-import-to-glob.js

use crate::should_ignore::should_ignore;
use oxc::{
  ast::ast::{Argument, BinaryExpression, CallExpression, Expression, TemplateLiteral},
  codegen::CodeGenerator,
  syntax::operator::BinaryOperator,
};
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

// Disallow ./*.ext
static OWN_DIRECTORY_STAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"^\./\*\.\w+$";
  Regex::new(pattern).expect("failed to compile regex")
});

static EXAMPLE_CODE: &str = "For example: import(`./foo/${bar}.js`).";

fn expr_to_str(expr: &Expression) -> String {
  let mut codegen = CodeGenerator::default();
  codegen.print_expression(expr);
  codegen.into_source_text()
}

pub(crate) fn to_glob_pattern(expr: &Expression) -> anyhow::Result<Option<String>> {
  let glob = expr_to_glob(expr)?;

  if should_ignore(&glob) {
    return Ok(None);
  }

  let glob = glob.replace("**", "*");

  if glob.starts_with('*') {
    let expr = expr_to_str(expr);
    return Err(
      anyhow::format_err!("invalid import \"{expr}\". It cannot be statically analyzed. Variable dynamic imports must start with ./ and be limited to a specific directory. {EXAMPLE_CODE}"));
  }

  if glob.starts_with('/') {
    let expr = expr_to_str(expr);
    return Err(
      anyhow::format_err!("invalid import \"{expr}\". Variable absolute imports are not supported, imports must start with ./ in the static part of the import. {EXAMPLE_CODE}"));
  }

  if !glob.starts_with("./") && !glob.starts_with("../") {
    let expr = expr_to_str(expr);
    return Err(
      anyhow::format_err!("invalid import \"{expr}\". Variable bare imports are not supported, imports must start with ./ in the static part of the import. {EXAMPLE_CODE}"));
  }

  if OWN_DIRECTORY_STAR_REGEX.is_match(&glob) {
    let expr = expr_to_str(expr);
    return Err(
      anyhow::format_err!("invalid import \"{expr}\". Variable imports cannot import their own directory, place imports in a separate directory or make the import filename more specific. {EXAMPLE_CODE}"));
  }

  if Path::new(&glob).extension().is_none() {
    let expr = expr_to_str(expr);
    return Err(
      anyhow::format_err!("invalid import \"{expr}\". A file extension must be included in the static part of the import. {EXAMPLE_CODE}"),
    );
  }

  Ok(Some(glob))
}

fn expr_to_glob(expr: &Expression) -> anyhow::Result<String> {
  Ok(match expr {
    Expression::TemplateLiteral(node) => template_literal_to_glob(node)?,
    Expression::CallExpression(node) => call_expr_to_glob(node)?,
    Expression::BinaryExpression(node) => binary_expr_to_glob(node)?,
    Expression::StringLiteral(node) => sanitize_string(&node.value)?,
    _ => String::from("*"),
  })
}

fn arg_to_glob(arg: &Argument) -> anyhow::Result<String> {
  Ok(match arg {
    Argument::SpreadElement(_) => String::from("*"),
    node => expr_to_glob(node.to_expression())?,
  })
}

fn template_literal_to_glob(node: &TemplateLiteral) -> anyhow::Result<String> {
  let mut glob = String::new();

  for (index, quasi) in node.quasis.iter().enumerate() {
    glob += &sanitize_string(&quasi.value.raw)?;
    if let Some(expr) = node.expressions.get(index) {
      glob += &expr_to_glob(expr)?;
    }
  }

  Ok(glob)
}

fn call_expr_to_glob(node: &CallExpression) -> anyhow::Result<String> {
  if let Expression::StaticMemberExpression(member_expr) = &node.callee {
    if member_expr.property.name == "concat" {
      let mut arg_glob = String::new();
      for arg in &node.arguments {
        arg_glob += &arg_to_glob(arg)?;
      }
      return Ok(expr_to_glob(&member_expr.object)? + &arg_glob);
    }
  }

  Ok(String::from("*"))
}

fn binary_expr_to_glob(node: &BinaryExpression) -> anyhow::Result<String> {
  if !matches!(node.operator, BinaryOperator::Addition) {
    return Err(anyhow::format_err!("{:?} operator is not supported.", node.operator.as_str()));
  }

  Ok(expr_to_glob(&node.left)? + &expr_to_glob(&node.right)?)
}

fn sanitize_string(s: &str) -> anyhow::Result<String> {
  if s.is_empty() {
    return Ok(s.to_string());
  }
  if s.contains('*') {
    return Err(anyhow::format_err!("A dynamic import cannot contain * characters."));
  }
  Ok(glob::Pattern::escape(s))
}

#[cfg(test)]
mod tests {
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

  #[test]
  fn template_literal_with_variable_filename() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./foo/${bar}.js`");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn external() {
    let parser = ExprParser::new();
    let ast = parser.parse("`https://some.cdn.com/package/${version}/index.js`");
    let glob = to_glob_pattern(&ast).unwrap();
    assert!(glob.is_none());
  }

  #[test]
  fn external_leaves_bare_module_specifiers_starting_with_https_in_tact() {
    let parser = ExprParser::new();
    let ast = parser.parse("'http_utils'");
    let glob = to_glob_pattern(&ast).unwrap();
    assert!(glob.is_none());
  }

  #[test]
  fn data_uri() {
    let parser = ExprParser::new();
    let ast = parser.parse("`data:${bar}`");
    let glob = to_glob_pattern(&ast).unwrap();
    assert!(glob.is_none());
  }

  #[test]
  fn template_literal_with_dot_prefixed_suffix() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${bar}.entry.js`");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./*.entry.js");
  }

  #[test]
  fn template_literal_with_variable_directory() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./foo/${bar}/x.js`");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./foo/*/x.js");
  }

  #[test]
  fn template_literal_with_multiple_variables() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${foo}/${bar}.js`");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./*/*.js");
  }

  #[test]
  fn dynamic_expression_with_variable_filename() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./foo/'.concat(bar,'.js')");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn dynamic_expression_with_variable_directory() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./foo/'.concat(bar, '/x.js')");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./foo/*/x.js");
  }

  #[test]
  fn dynamic_expression_with_multiple_variables() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./'.concat(foo, '/').concat(bar,'.js')");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./*/*.js");
  }

  #[test]
  fn string_concatenation() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./foo/' + bar + '.js'");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn string_concatenation_and_template_literals_combined() {
    let parser = ExprParser::new();
    let ast = parser.parse("'./' + `foo/${bar}` + '.js'");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn string_literal_in_a_template_literal_expression() {
    let parser = ExprParser::new();
    let ast = parser.parse("`${'./foo/'}${bar}.js`");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./foo/*.js");
  }

  #[test]
  fn multiple_variables_are_collapsed_into_a_single_star() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./foo/${bar}${baz}/${x}${y}.js`");
    let glob = to_glob_pattern(&ast).unwrap();
    assert_eq!(glob.unwrap(), "./foo/*/*.js");
  }

  #[test]
  fn throws_when_dynamic_import_contains_a_star() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./*${foo}.js`");
    let err = to_glob_pattern(&ast).unwrap_err();
    assert_eq!(err.to_string(), "A dynamic import cannot contain * characters.");
  }

  #[test]
  fn throws_when_dynamic_import_contains_a_non_add_operator() {
    let parser = ExprParser::new();
    let ast = parser.parse("'foo' - 'bar.js'");
    let err = to_glob_pattern(&ast).unwrap_err().to_string();
    assert_eq!(err, "\"-\" operator is not supported.");
  }

  #[test]
  fn throws_when_dynamic_import_is_a_single_variable() {
    let parser = ExprParser::new();
    let ast = parser.parse("foo");
    let err = to_glob_pattern(&ast).unwrap_err().to_string();
    assert_eq!(err, "invalid import \"foo\". It cannot be statically analyzed. Variable dynamic imports must start with ./ and be limited to a specific directory. For example: import(`./foo/${bar}.js`).");
  }

  #[test]
  fn throws_when_dynamic_import_starts_with_a_variable() {
    let parser = ExprParser::new();
    let ast = parser.parse("`${folder}/foo.js`");
    let err = to_glob_pattern(&ast).unwrap_err().to_string();
    assert_eq!(err,
      "invalid import \"`${folder}/foo.js`\". It cannot be statically analyzed. Variable dynamic imports must start with ./ and be limited to a specific directory. For example: import(`./foo/${bar}.js`)."
    );
  }

  #[test]
  fn throws_when_dynamic_import_starts_with_a_slash() {
    let parser = ExprParser::new();
    let ast = parser.parse("`/foo/${bar}.js`");
    let err = to_glob_pattern(&ast).unwrap_err().to_string();
    assert_eq!(err,
      "invalid import \"`/foo/${bar}.js`\". Variable absolute imports are not supported, imports must start with ./ in the static part of the import. For example: import(`./foo/${bar}.js`)."
    );
  }

  #[test]
  fn throws_when_dynamic_import_does_not_start_with_dot_slash() {
    let parser = ExprParser::new();
    let ast = parser.parse("`foo/${bar}.js`");
    let err = to_glob_pattern(&ast).unwrap_err().to_string();
    assert_eq!(err,
      "invalid import \"`foo/${bar}.js`\". Variable bare imports are not supported, imports must start with ./ in the static part of the import. For example: import(`./foo/${bar}.js`)."
    );
  }

  #[test]
  fn throws_when_dynamic_import_imports_its_own_directory() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${foo}.js`");
    let err = to_glob_pattern(&ast).unwrap_err().to_string();
    assert_eq!(err,
      "invalid import \"`./${foo}.js`\". Variable imports cannot import their own directory, place imports in a separate directory or make the import filename more specific. For example: import(`./foo/${bar}.js`)."
    );
  }

  #[test]
  fn throws_when_dynamic_import_imports_does_not_contain_a_file_extension() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./foo/${bar}`");
    let err = to_glob_pattern(&ast).unwrap_err().to_string();
    assert_eq!(err,
      "invalid import \"`./foo/${bar}`\". A file extension must be included in the static part of the import. For example: import(`./foo/${bar}.js`)."
    );
  }

  #[test]
  fn escapes_round_brackets() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${foo}/(foo).js`");
    let glob = to_glob_pattern(&ast).unwrap();
    // The escaped glob in JS is "./*/\\(foo\\).js"
    assert_eq!(glob.unwrap(), "./*/(foo).js");
  }

  #[test]
  fn escapes_square_brackets() {
    let parser = ExprParser::new();
    let ast = parser.parse("`./${foo}/[foo].js`");
    let glob = to_glob_pattern(&ast).unwrap();
    // The escaped glob in JS is "./*/\\[foo\\].js"
    assert_eq!(glob.unwrap(), "./*/[[]foo[]].js");
  }
}
