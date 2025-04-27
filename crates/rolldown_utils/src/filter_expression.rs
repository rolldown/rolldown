use crate::pattern_filter::{StringOrRegex, StringOrRegexMatchKind};

pub enum FilterExpr {
  Or(Box<FilterExpr>, Box<FilterExpr>),
  And(Box<FilterExpr>, Box<FilterExpr>),
  Not(Box<FilterExpr>),
  Code(StringOrRegex),
  Id(StringOrRegex),
  ModuleType(String),
}

pub enum FilterKind {
  Include(FilterExpr),
  Exclude(FilterExpr),
}

pub fn filter_expr_interpreter(
  expr: &FilterExpr,
  id: Option<&str>,
  code: Option<&str>,
  module_type: Option<&str>,
  cwd: &str,
) -> bool {
  match expr {
    FilterExpr::Or(left, right) => {
      filter_expr_interpreter(left, id, code, module_type, cwd)
        || filter_expr_interpreter(right, id, code, module_type, cwd)
    }
    FilterExpr::And(left, right) => {
      filter_expr_interpreter(left, id, code, module_type, cwd)
        && filter_expr_interpreter(right, id, code, module_type, cwd)
    }
    FilterExpr::Not(inner) => !filter_expr_interpreter(inner, id, code, module_type, cwd),
    FilterExpr::Code(pattern) => {
      pattern.test(code.expect("code should not be none"), &StringOrRegexMatchKind::Code)
    }
    FilterExpr::Id(id_pattern) => {
      id_pattern.test(id.expect("id should not be none"), &StringOrRegexMatchKind::Id(cwd))
    }
    FilterExpr::ModuleType(module_type_filter) => {
      if let Some(module_type) = module_type {
        module_type == module_type_filter
      } else {
        false
      }
    }
  }
}

pub fn filter_exprs_interpreter(
  exprs: &[FilterKind],
  id: Option<&str>,
  code: Option<&str>,
  module_type: Option<&str>,
  cwd: &str,
) -> bool {
  for kind in exprs {
    match kind {
      FilterKind::Include(filter_expr) => {
        if filter_expr_interpreter(filter_expr, id, code, module_type, cwd) {
          return true;
        }
      }
      FilterKind::Exclude(filter_expr) => {
        if filter_expr_interpreter(filter_expr, id, code, module_type, cwd) {
          return false;
        }
      }
    }
  }
  false
}

pub enum Token {
  Id(StringOrRegex),
  Code(StringOrRegex),
  ModuleType(String),
  And,
  Or,
  Not,
  Include,
  Exclude,
}

pub fn parse(mut tokens: Vec<Token>) -> FilterKind {
  fn rec(tokens: &mut Vec<Token>) -> Option<FilterExpr> {
    let token = tokens.pop()?;
    match token {
      Token::Id(string_or_regex) => Some(FilterExpr::Id(string_or_regex)),
      Token::Code(string_or_regex) => Some(FilterExpr::Code(string_or_regex)),
      Token::ModuleType(string) => Some(FilterExpr::ModuleType(string)),
      Token::And => {
        let left = rec(tokens)?;
        let right = rec(tokens)?;
        Some(FilterExpr::And(Box::new(left), Box::new(right)))
      }
      Token::Or => {
        let left = rec(tokens)?;
        let right = rec(tokens)?;
        Some(FilterExpr::Or(Box::new(left), Box::new(right)))
      }
      Token::Not => {
        let inner = rec(tokens)?;
        Some(FilterExpr::Not(Box::new(inner)))
      }
      Token::Include => {
        unreachable!("Include token should not be in the expression");
      }
      Token::Exclude => {
        unreachable!("Exclude token should not be in the expression");
      }
    }
  }
  match tokens.pop() {
    Some(Token::Include) => {
      let inner = rec(&mut tokens).expect("Failed to parse expression");
      FilterKind::Include(inner)
    }
    Some(Token::Exclude) => {
      let inner = rec(&mut tokens).expect("Failed to parse expression");
      FilterKind::Exclude(inner)
    }

    _ => unreachable!("Expression should start with Include or Exclude"),
  }
}

#[cfg(test)]
mod test {
  use crate::{
    filter_expression::{FilterExpr, Token, filter_expr_interpreter},
    pattern_filter::StringOrRegex,
  };

  use super::{filter_exprs_interpreter, parse};

  #[test]
  fn test_filter_expr_interpreter() {
    // https://github.com/vitejs/rolldown-vite/blob/fef84b75dbb35a6ec27debdc0dced1d0f1250eb8/packages/vite/src/node/plugins/importAnalysisBuild.ts?plain=1#L242-L244
    let expr = FilterExpr::And(
      Box::new(FilterExpr::Id(StringOrRegex::Regex("node_modules".into()))),
      Box::new(FilterExpr::Not(Box::new(FilterExpr::Code(StringOrRegex::Regex(
        "import\\s*".into(),
      ))))),
    );
    assert!(!filter_expr_interpreter(
      &expr,
      Some("/foo/bar.js"),
      Some("console.log('test')"),
      None,
      "."
    ));

    assert!(filter_expr_interpreter(
      &expr,
      Some("/node_modules/bar.js"),
      Some("console.log('test')"),
      None,
      "."
    ));

    assert!(!filter_expr_interpreter(
      &expr,
      Some("/node_modules/bar.js"),
      Some("import('foo')"),
      None,
      "."
    ));
  }

  #[test]
  fn parse_test() {
    // exclude(and(id(/node_modules/), not(code(/import\\s*/))))
    let mut tokens = vec![
      Token::Exclude,
      Token::And,
      Token::Id(StringOrRegex::Regex("node_modules".into())),
      Token::Not,
      Token::Code(StringOrRegex::Regex("import\\s*".into())),
    ];
    tokens.reverse();

    let expr = parse(tokens);
    // the expr return `true`, but since it is a `Exclude`, finally it should be `false`
    assert!(!filter_exprs_interpreter(
      &[expr],
      Some("/node_modules/bar.js"),
      Some("console.log('test')"),
      None,
      ".",
    ));
  }
}
