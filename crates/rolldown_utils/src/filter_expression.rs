use crate::{
  clean_url::clean_url,
  js_regex::HybridRegex,
  pattern_filter::{StringOrRegex, StringOrRegexMatchKind},
};

#[derive(Debug)]
pub enum FilterExpr {
  Or(Vec<FilterExpr>),
  And(Vec<FilterExpr>),
  Not(Box<FilterExpr>),
  Code(StringOrRegex),
  Id(StringOrRegex),
  CleanUrl(Box<FilterExpr>),
  ModuleType(String),
}

#[derive(Debug)]
pub enum FilterExprKind {
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
    FilterExpr::Or(args) => {
      args.iter().any(|arg| filter_expr_interpreter(arg, id, code, module_type, cwd))
    }
    FilterExpr::And(args) => {
      args.iter().all(|arg| filter_expr_interpreter(arg, id, code, module_type, cwd))
    }
    FilterExpr::Not(inner) => !filter_expr_interpreter(inner, id, code, module_type, cwd),
    FilterExpr::Code(pattern) => {
      pattern.test(code.expect("code should not be none"), &StringOrRegexMatchKind::Code)
    }
    FilterExpr::Id(id_pattern) => {
      id_pattern.test(id.expect("id should not be none"), &StringOrRegexMatchKind::Id(cwd))
    }
    FilterExpr::ModuleType(module_type_filter) => {
      module_type.as_ref().is_some_and(|module_type| module_type == module_type_filter)
    }
    FilterExpr::CleanUrl(expr) => {
      filter_expr_interpreter(expr, id.map(clean_url), code, module_type, cwd)
    }
  }
}

pub fn filter_exprs_interpreter(
  exprs: &[FilterExprKind],
  id: Option<&str>,
  code: Option<&str>,
  // TODO: Use ModuleType instead
  module_type: Option<&str>,
  cwd: &str,
) -> bool {
  let mut include_count = 0;
  for kind in exprs {
    match kind {
      FilterExprKind::Include(filter_expr) => {
        include_count += 1;
        if filter_expr_interpreter(filter_expr, id, code, module_type, cwd) {
          return true;
        }
      }
      FilterExprKind::Exclude(filter_expr) => {
        if filter_expr_interpreter(filter_expr, id, code, module_type, cwd) {
          return false;
        }
      }
    }
  }
  include_count == 0
}

#[derive(Debug)]
pub enum Token {
  Id,
  Code,
  ModuleType,
  /// Arg count
  And(u32),
  /// Arg count
  Or(u32),
  Not,
  Include,
  Exclude,
  CleanUrl,
  String(String),
  Regex(HybridRegex),
}

impl From<StringOrRegex> for Token {
  fn from(value: StringOrRegex) -> Self {
    match value {
      StringOrRegex::String(v) => Self::String(v),
      StringOrRegex::Regex(regex) => Self::Regex(regex),
    }
  }
}

// TODO: better error handling
pub fn parse(mut tokens: Vec<Token>) -> FilterExprKind {
  fn rec(tokens: &mut Vec<Token>) -> Option<FilterExpr> {
    let token = tokens.pop()?;
    match token {
      Token::Id => {
        let string_or_regex = match tokens.pop()? {
          Token::String(str) => StringOrRegex::String(str),
          Token::Regex(regexp) => StringOrRegex::Regex(regexp),
          _ => {
            unreachable!("Id token should be followed by a string or regex")
          }
        };
        Some(FilterExpr::Id(string_or_regex))
      }
      Token::Code => {
        let string_or_regex = match tokens.pop()? {
          Token::String(str) => StringOrRegex::String(str),
          Token::Regex(regexp) => StringOrRegex::Regex(regexp),
          _ => {
            unreachable!("Code token should be followed by a string or regex")
          }
        };
        Some(FilterExpr::Code(string_or_regex))
      }
      Token::ModuleType => {
        let Token::String(string) = tokens.pop()? else {
          unreachable!("ModuleType token should be followed by a string");
        };
        Some(FilterExpr::ModuleType(string))
      }
      Token::And(arg_count) => {
        let mut args = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
          let inner = rec(tokens)?;
          args.push(inner);
        }
        Some(FilterExpr::And(args))
      }
      Token::Or(arg_count) => {
        let mut args = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
          let inner = rec(tokens)?;
          args.push(inner);
        }
        Some(FilterExpr::Or(args))
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
      Token::CleanUrl => {
        let arg = rec(tokens)?;
        Some(FilterExpr::CleanUrl(Box::new(arg)))
      }
      Token::String(_) => {
        unreachable!("String token should not appear standalone");
      }
      Token::Regex(_) => {
        unreachable!("Regex token should not appear standalone");
      }
    }
  }
  match tokens.pop() {
    Some(Token::Include) => {
      let inner = rec(&mut tokens).expect("Failed to parse expression");
      FilterExprKind::Include(inner)
    }
    Some(Token::Exclude) => {
      let inner = rec(&mut tokens).expect("Failed to parse expression");
      FilterExprKind::Exclude(inner)
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
    let expr = FilterExpr::And(vec![
      FilterExpr::Id(StringOrRegex::Regex("node_modules".into())),
      FilterExpr::Not(Box::new(FilterExpr::Code(StringOrRegex::Regex("import\\s*".into())))),
    ]);
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
      Token::And(2u32),
      Token::Id,
      Token::Regex("node_modules".into()),
      Token::Not,
      Token::Code,
      Token::Regex("import\\s*".into()),
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
