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

#[cfg(test)]
mod test {
  use crate::{
    filter_expression::{FilterExpr, filter_expr_interpreter},
    pattern_filter::StringOrRegex,
  };

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
}
