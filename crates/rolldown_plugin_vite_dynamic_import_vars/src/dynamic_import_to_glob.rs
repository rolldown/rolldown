// Ported from https://github.com/rollup/plugins/blob/944e7d3/packages/dynamic-import-vars/src/dynamic-import-to-glob.js
use oxc::{
  ast::ast::{Argument, BinaryExpression, CallExpression, Expression, TemplateLiteral},
  syntax::operator::BinaryOperator,
};
use rolldown_utils::ecmascript::to_usv_str;
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
    Expression::StringLiteral(node) => match to_usv_str(node.value) {
      Cow::Borrowed(value) => sanitize_string(value),
      Cow::Owned(value) => sanitize_string(&value).map(|glob| Cow::Owned(glob.into_owned())),
    },
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

  fn to_glob_pattern<'a>(expr: &'a Expression, source: &'a str) -> anyhow::Result<Option<String>> {
    let glob = expr_to_glob(expr)?;
    let glob = glob.cow_replace("**", "*");
    if should_ignore(&glob) || !glob.contains('*') {
      return Ok(None);
    }
    Ok(Some(to_valid_glob(&glob, source)?.into_owned()))
  }

  fn run(code: &str, source: &str) -> anyhow::Result<Option<String>> {
    let allocator = Allocator::default();
    let expr = Parser::new(&allocator, code, SourceType::default()).parse_expression().unwrap();
    to_glob_pattern(&expr, source)
  }

  #[test]
  fn converts_dynamic_import_expressions_to_globs() {
    // (case, expression, expected glob; `None` means the import is left alone)
    // Rows are not interchangeable: no longer escaping `[`/`]`, or no longer ignoring `data:`,
    // fails only `escapes_square_brackets` / `data_uri` respectively (mutation-checked).
    let cases: &[(&str, &str, Option<&str>)] = &[
      ("template_literal_with_variable_filename", "`./foo/${bar}.js`", Some("./foo/*.js")),
      ("external", "`https://some.cdn.com/package/${version}/index.js`", None),
      ("external_leaves_bare_module_specifiers_starting_with_https_in_tact", "'http_utils'", None),
      ("data_uri", "`data:${bar}`", None),
      ("template_literal_with_dot_prefixed_suffix", "`./${bar}.entry.js`", Some("./*.entry.js")),
      ("template_literal_with_variable_directory", "`./foo/${bar}/x.js`", Some("./foo/*/x.js")),
      ("template_literal_with_multiple_variables", "`./${foo}/${bar}.js`", Some("./*/*.js")),
      (
        "dynamic_expression_with_variable_filename",
        "'./foo/'.concat(bar,'.js')",
        Some("./foo/*.js"),
      ),
      (
        "dynamic_expression_with_variable_directory",
        "'./foo/'.concat(bar, '/x.js')",
        Some("./foo/*/x.js"),
      ),
      (
        "dynamic_expression_with_multiple_variables",
        "'./'.concat(foo, '/').concat(bar,'.js')",
        Some("./*/*.js"),
      ),
      ("string_concatenation", "'./foo/' + bar + '.js'", Some("./foo/*.js")),
      (
        "string_concatenation_and_template_literals_combined",
        "'./' + `foo/${bar}` + '.js'",
        Some("./foo/*.js"),
      ),
      (
        "string_literal_in_a_template_literal_expression",
        "`${'./foo/'}${bar}.js`",
        Some("./foo/*.js"),
      ),
      (
        "multiple_variables_are_collapsed_into_a_single_star",
        "`./foo/${bar}${baz}/${x}${y}.js`",
        Some("./foo/*/*.js"),
      ),
      // The escaped glob in JS is "./*/\\(foo\\).js"
      ("escapes_round_brackets", "`./${foo}/(foo).js`", Some("./*/(foo).js")),
      // The escaped glob in JS is "./*/\\[foo\\].js"
      ("escapes_square_brackets", "`./${foo}/[foo].js`", Some("./*/[[]foo[]].js")),
      // A lone surrogate becomes U+FFFD, as it does when the runtime resolves the import.
      (
        "string_literal_with_a_lone_surrogate",
        r"'./foo\uD800/' + bar + '.js'",
        Some("./foo\u{FFFD}/*.js"),
      ),
    ];
    for &(case, code, expected) in cases {
      let glob = run(code, "").unwrap_or_else(|err| panic!("{case}: unexpected error: {err}"));
      assert_eq!(glob.as_deref(), expected, "{case}");
    }
  }

  #[test]
  fn rejects_unsupported_dynamic_import_expressions() {
    // (case, expression, source passed to `to_valid_glob`, expected error)
    // Each row is the only guard for its error: accepting every binary operator, or dropping the
    // file-extension check, fails only that row (mutation-checked).
    let cases: &[(&str, &str, &str, &str)] = &[
      (
        "throws_when_dynamic_import_contains_a_star",
        "`./*${foo}.js`",
        "",
        "A dynamic import cannot contain * characters.",
      ),
      (
        "throws_when_dynamic_import_contains_a_non_add_operator",
        "'foo' - 'bar.js'",
        "",
        "\"-\" operator is not supported.",
      ),
      (
        "throws_when_dynamic_import_imports_does_not_contain_a_file_extension",
        "`./foo/${bar}`",
        "\"`./foo/${bar}`\"",
        "Invalid import \"`./foo/${bar}`\". A file extension must be included in the static part of the import. For example: import(`./foo/${bar}.js`).",
      ),
    ];
    for &(case, code, source, expected) in cases {
      let err = run(code, source).expect_err(case);
      assert_eq!(err.to_string(), expected, "{case}");
    }
  }
}
