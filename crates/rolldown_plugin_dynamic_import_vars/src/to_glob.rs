use std::path::Path;

use once_cell::sync::Lazy;
use oxc::{
  ast::ast::{Argument, BinaryExpression, CallExpression, Expression, TemplateLiteral},
  syntax::operator::BinaryOperator,
};
use regex::Regex;

use crate::sanitize::sanitize_string;
use crate::should_ignore::should_ignore;

// Disallow ./*.ext
static OWN_DIRECTORY_STAR_REGEX: Lazy<Regex> = Lazy::new(|| {
  let pattern: &str = "^\\.\\/\\*\\.[\\w]+$";
  Regex::new(pattern).expect("failed to compile regex")
});

static EXAMPLE_CODE: Lazy<&str> = Lazy::new(|| "For example: import(`./foo/${bar}.js`).");

pub fn to_glob_pattern<'ast>(expr: &Expression<'ast>) -> Option<String> {
  let glob = expr_to_glob(expr);

  if should_ignore(&glob) {
    return None;
  }

  let glob = glob.replace("**", "*");

  if glob.starts_with('*') {
    // `invalid import "${sourceString}". It cannot be statically analyzed. Variable dynamic imports must start with ./ and be limited to a specific directory. ${example}`
  }

  if glob.ends_with('/') {
    // `invalid import "${sourceString}". Variable absolute imports are not supported, imports must start with ./ in the static part of the import. ${example}`
  }

  if !glob.starts_with("./") && !glob.starts_with("../") {
    // `invalid import "${sourceString}". Variable bare imports are not supported, imports must start with ./ in the static part of the import. ${example}`
  }

  if OWN_DIRECTORY_STAR_REGEX.is_match(&glob) {
    // `${
    //   `invalid import "${sourceString}". Variable imports cannot import their own directory, ` +
    //   'place imports in a separate directory or make the import filename more specific. '
    // }${example}`
  }

  if Path::new(&glob).extension().is_none() {
    // `invalid import "${sourceString}". A file extension must be included in the static part of the import. ${example}`
  }

  Some(glob)
}

fn expr_to_glob<'ast>(expr: &Expression<'ast>) -> String {
  match expr {
    Expression::TemplateLiteral(node) => template_literal_to_glob(node),
    Expression::CallExpression(node) => call_expr_to_glob(node),
    Expression::BinaryExpression(node) => binary_expr_to_glob(node),
    Expression::StringLiteral(node) => sanitize_string(&node.value),
    _ => String::from("*"),
  }
}

fn arg_to_glob<'ast>(arg: &Argument<'ast>) -> String {
  match arg {
    Argument::TemplateLiteral(node) => template_literal_to_glob(node),
    Argument::CallExpression(node) => call_expr_to_glob(node),
    Argument::BinaryExpression(node) => binary_expr_to_glob(node),
    Argument::StringLiteral(node) => sanitize_string(&node.value),
    _ => String::from("*"),
  }
}

fn template_literal_to_glob<'ast>(node: &TemplateLiteral<'ast>) -> String {
  let mut glob = String::new();

  for (index, quasi) in node.quasis.iter().enumerate() {
    glob += &sanitize_string(&quasi.value.raw);
    if let Some(expr) = node.expressions.get(index) {
      glob += &expr_to_glob(expr);
    }
  }

  return glob;
}

fn call_expr_to_glob<'ast>(node: &CallExpression<'ast>) -> String {
  if let Expression::StaticMemberExpression(member_expr) = &node.callee {
    if member_expr.property.name == "concat" {
      let arg_globs: Vec<String> = node.arguments.iter().map(arg_to_glob).collect();
      return expr_to_glob(&member_expr.object) + &arg_globs.join("");
    }
  }

  String::from("*")
}

fn binary_expr_to_glob<'ast>(node: &BinaryExpression<'ast>) -> String {
  if !matches!(node.operator, BinaryOperator::Addition) {
    // `${node.operator} operator is not supported.`
  }

  expr_to_glob(&node.left) + &expr_to_glob(&node.right)
}
