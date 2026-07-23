use std::borrow::Cow;

use rustc_hash::FxHashMap;

use crate::{
  js_regex::HybridRegex,
  pattern_filter::{StringOrRegex, StringOrRegexMatchKind, normalize_path},
  url::{clean_url, get_query},
};

#[derive(Debug)]
pub enum FilterExpr {
  Or(Vec<FilterExpr>),
  And(Vec<FilterExpr>),
  Not(Box<FilterExpr>),
  Code(StringOrRegex),
  Id(StringOrRegex),
  ImporterId(StringOrRegex),
  CleanUrl(Box<FilterExpr>),
  ModuleType(String),
  Query(String, QueryValue),
}

#[derive(Debug)]
pub enum QueryValue {
  String(String),
  Regex(HybridRegex),
  Boolean(bool),
}

#[derive(Debug)]
pub enum FilterExprKind {
  Include(FilterExpr),
  Exclude(FilterExpr),
}

/// Relative, static cost estimate for evaluating a [`FilterExpr`], used to order
/// `And`/`Or` operands cheapest-first (see [`optimize_filter_expr`]). Lower is
/// cheaper. These are ordinal ranks, not measured costs — only their relative
/// order matters. The dominant distinction is that `Code` tests scan the entire
/// module source, whereas `Id`/`ImporterId` only test the (short) path.
fn cost_rank(expr: &FilterExpr) -> u32 {
  match expr {
    FilterExpr::ModuleType(_) => 0,
    FilterExpr::Id(_) | FilterExpr::ImporterId(_) => 1,
    // The first `Query` eval scans/decodes the id's URL and collects its params into a
    // map, so rank it *above* the cheap `Id`/`ModuleType` checks — they should get to
    // short-circuit before that parse is forced.
    FilterExpr::Query(..) => 2,
    // Both `Code` variants scan the full source and neither is reliably cheaper — an
    // anchored regex can fail fast, while an absent string still scans everything — so
    // give them one tier and let the stable sort preserve their authored order.
    FilterExpr::Code(_) => 3,
    // A wrapper costs whatever its (single) inner expression costs.
    FilterExpr::Not(inner) | FilterExpr::CleanUrl(inner) => cost_rank(inner),
    // Rank a compound by its *costliest* reachable leaf, not its cheapest: an operand
    // whose cheap leaf fails still falls through to its expensive one, so `min` would let
    // e.g. `or(moduleType, code)` pose as free (rank 0) and jump ahead of a cheaper
    // sibling that would have rejected first — forcing the very full-source scan this
    // reordering exists to avoid.
    FilterExpr::And(args) | FilterExpr::Or(args) => args.iter().map(cost_rank).max().unwrap_or(0),
  }
}

/// Reorder `And`/`Or` operands cheapest-first (stably) so that short-circuit
/// evaluation skips expensive leaves — most importantly a `Code` regex over the
/// full module source — whenever a cheaper operand already decides the result.
///
/// This is a semantics-preserving transform: the boolean operators are pure over
/// side-effect-free predicates. The only interpreter state, the lazily-populated
/// [`InterpreterCtx::parsed_url_cache`], is keyed by the (fixed) id's query and so
/// is order-independent. Reordering only ever moves *cheaper* operands earlier, so
/// it can only make short-circuiting skip an expensive leaf more often, never less.
fn optimize_filter_expr(expr: &mut FilterExpr) {
  match expr {
    FilterExpr::And(args) | FilterExpr::Or(args) => {
      for arg in args.iter_mut() {
        optimize_filter_expr(arg);
      }
      args.sort_by_cached_key(cost_rank);
    }
    FilterExpr::Not(inner) | FilterExpr::CleanUrl(inner) => optimize_filter_expr(inner),
    FilterExpr::Id(_)
    | FilterExpr::ImporterId(_)
    | FilterExpr::Code(_)
    | FilterExpr::ModuleType(_)
    | FilterExpr::Query(..) => {}
  }
}

pub fn filter_expr_interpreter<'a>(
  expr: &FilterExpr,
  id: Option<&'a str>,
  code: Option<&str>,
  module_type: Option<&str>,
  importer_id: Option<&'a str>,
  cwd: &str,
  ctx: &mut InterpreterCtx<'a>,
) -> bool {
  match expr {
    FilterExpr::Or(args) => args
      .iter()
      .any(|arg| filter_expr_interpreter(arg, id, code, module_type, importer_id, cwd, ctx)),
    FilterExpr::And(args) => args
      .iter()
      .all(|arg| filter_expr_interpreter(arg, id, code, module_type, importer_id, cwd, ctx)),
    FilterExpr::Not(inner) => {
      !filter_expr_interpreter(inner, id, code, module_type, importer_id, cwd, ctx)
    }
    FilterExpr::Code(pattern) => {
      pattern.test(code.expect("`code` should not be none"), &StringOrRegexMatchKind::Code)
    }
    FilterExpr::Id(id_pattern) => {
      id_pattern.test(id.expect("`id` should not be none"), &StringOrRegexMatchKind::Id(cwd))
    }
    FilterExpr::ImporterId(id_pattern) => {
      // When importer_id is None (e.g., entry files), return false (no match)
      match importer_id {
        Some(importer) => id_pattern.test(importer, &StringOrRegexMatchKind::Id(cwd)),
        None => false,
      }
    }
    FilterExpr::ModuleType(module_type_filter) => {
      module_type.as_ref().is_some_and(|module_type| module_type == module_type_filter)
    }
    FilterExpr::CleanUrl(expr) => filter_expr_interpreter(
      expr,
      id.map(clean_url),
      code,
      module_type,
      importer_id.map(clean_url),
      cwd,
      ctx,
    ),
    FilterExpr::Query(key, value) => {
      if ctx.parsed_url_cache.is_none() {
        let query_string = get_query(id.expect("`id` should not be none"));
        let cache = form_urlencoded::parse(query_string.as_bytes())
          .into_iter()
          .map(|(k, v)| (k.to_string(), v))
          .collect::<_>();
        ctx.parsed_url_cache = Some(cache);
      }
      match value {
        QueryValue::String(v) => ctx
          .parsed_url_cache
          .as_ref()
          .and_then(|cache| cache.get(key).map(|qv| qv == v))
          .unwrap_or(false),
        QueryValue::Regex(hybrid_regex) => ctx
          .parsed_url_cache
          .as_ref()
          .and_then(|cache| cache.get(key).map(|qv| hybrid_regex.matches(qv)))
          .unwrap_or(false),
        QueryValue::Boolean(v) => {
          let has_key = ctx.parsed_url_cache.as_ref().is_some_and(|cache| cache.contains_key(key));
          *v == has_key
        }
      }
    }
  }
}

#[derive(Default, Debug)]
pub struct InterpreterCtx<'a> {
  parsed_url_cache: Option<FxHashMap<String, Cow<'a, str>>>,
}

pub fn filter_exprs_interpreter(
  exprs: &[FilterExprKind],
  id: Option<&str>,
  code: Option<&str>,
  // TODO: Use ModuleType instead
  module_type: Option<&str>,
  importer_id: Option<&str>,
  cwd: &str,
) -> bool {
  let mut include_count = 0;
  let mut ctx = InterpreterCtx::default();
  let id = id.map(|id| normalize_path(id));
  let id = id.as_deref();
  let importer_id = importer_id.map(|id| normalize_path(id));
  let importer_id = importer_id.as_deref();
  for kind in exprs {
    match kind {
      FilterExprKind::Include(filter_expr) => {
        include_count += 1;
        if filter_expr_interpreter(filter_expr, id, code, module_type, importer_id, cwd, &mut ctx) {
          return true;
        }
      }
      FilterExprKind::Exclude(filter_expr) => {
        if filter_expr_interpreter(filter_expr, id, code, module_type, importer_id, cwd, &mut ctx) {
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
  ImporterId,
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
  Query,
  String(String),
  Regex(HybridRegex),
  Boolean(bool),
}

impl From<StringOrRegex> for Token {
  fn from(value: StringOrRegex) -> Self {
    match value {
      StringOrRegex::String(v) => Self::String(v),
      StringOrRegex::Regex(regex) => Self::Regex(regex),
    }
  }
}

pub fn parse(mut tokens: Vec<Token>) -> anyhow::Result<FilterExprKind> {
  fn pop(tokens: &mut Vec<Token>) -> anyhow::Result<Token> {
    tokens.pop().ok_or_else(|| anyhow::anyhow!("unexpected end of filter expression tokens"))
  }

  fn pop_string_or_regex(tokens: &mut Vec<Token>, context: &str) -> anyhow::Result<StringOrRegex> {
    match pop(tokens)? {
      Token::String(str) => Ok(StringOrRegex::String(str)),
      Token::Regex(regexp) => Ok(StringOrRegex::Regex(regexp)),
      other => {
        anyhow::bail!("{context} token should be followed by a string or regex, but got {other:?}")
      }
    }
  }

  fn rec(tokens: &mut Vec<Token>) -> anyhow::Result<FilterExpr> {
    let token = pop(tokens)?;
    match token {
      Token::Id => Ok(FilterExpr::Id(pop_string_or_regex(tokens, "Id")?)),
      Token::ImporterId => Ok(FilterExpr::ImporterId(pop_string_or_regex(tokens, "ImporterId")?)),
      Token::Code => Ok(FilterExpr::Code(pop_string_or_regex(tokens, "Code")?)),
      Token::Query => {
        let key = match pop(tokens)? {
          Token::String(key) => key,
          other => anyhow::bail!("key of `Query` should be a string, but got {other:?}"),
        };
        let value = match pop(tokens)? {
          Token::String(v) => QueryValue::String(v),
          Token::Regex(v) => QueryValue::Regex(v),
          Token::Boolean(v) => QueryValue::Boolean(v),
          other => anyhow::bail!(
            "value of `Query` should be a string, regex, or boolean, but got {other:?}"
          ),
        };
        Ok(FilterExpr::Query(key, value))
      }
      Token::ModuleType => {
        let string = match pop(tokens)? {
          Token::String(s) => s,
          other => {
            anyhow::bail!("ModuleType token should be followed by a string, but got {other:?}")
          }
        };
        Ok(FilterExpr::ModuleType(string))
      }
      Token::And(arg_count) => {
        let mut args = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
          args.push(rec(tokens)?);
        }
        Ok(FilterExpr::And(args))
      }
      Token::Or(arg_count) => {
        let mut args = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
          args.push(rec(tokens)?);
        }
        Ok(FilterExpr::Or(args))
      }
      Token::Not => Ok(FilterExpr::Not(Box::new(rec(tokens)?))),
      Token::CleanUrl => Ok(FilterExpr::CleanUrl(Box::new(rec(tokens)?))),
      Token::Include => anyhow::bail!("Include token should not appear inside an expression"),
      Token::Exclude => anyhow::bail!("Exclude token should not appear inside an expression"),
      Token::String(_) => anyhow::bail!("String token should not appear standalone"),
      Token::Regex(_) => anyhow::bail!("Regex token should not appear standalone"),
      Token::Boolean(_) => anyhow::bail!("Boolean token should not appear standalone"),
    }
  }

  match tokens.pop() {
    Some(Token::Include) => {
      let mut expr = rec(&mut tokens)?;
      optimize_filter_expr(&mut expr);
      Ok(FilterExprKind::Include(expr))
    }
    Some(Token::Exclude) => {
      let mut expr = rec(&mut tokens)?;
      optimize_filter_expr(&mut expr);
      Ok(FilterExprKind::Exclude(expr))
    }
    Some(other) => {
      anyhow::bail!("filter expression should start with Include or Exclude, but got {other:?}")
    }
    None => anyhow::bail!("filter expression is empty"),
  }
}

#[cfg(test)]
mod test {
  use crate::{
    filter_expression::{FilterExpr, InterpreterCtx, QueryValue, Token, filter_expr_interpreter},
    pattern_filter::StringOrRegex,
  };

  use super::{filter_exprs_interpreter, optimize_filter_expr, parse};

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
      None,
      ".",
      &mut InterpreterCtx::default()
    ));

    assert!(filter_expr_interpreter(
      &expr,
      Some("/node_modules/bar.js"),
      Some("console.log('test')"),
      None,
      None,
      ".",
      &mut InterpreterCtx::default()
    ));

    assert!(!filter_expr_interpreter(
      &expr,
      Some("/node_modules/bar.js"),
      Some("import('foo')"),
      None,
      None,
      ".",
      &mut InterpreterCtx::default()
    ));

    #[cfg(windows)]
    {
      use super::FilterExprKind;
      let expr = FilterExpr::Id(StringOrRegex::Regex("src/".into()));

      assert!(filter_exprs_interpreter(
        &[FilterExprKind::Include(expr)],
        Some("C:\\path\\to\\src\\entry.js"),
        None,
        None,
        None,
        ".",
      ));
    }
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

    let expr = parse(tokens).unwrap();
    // the expr return `true`, but since it is a `Exclude`, finally it should be `false`
    assert!(!filter_exprs_interpreter(
      &[expr],
      Some("/node_modules/bar.js"),
      Some("console.log('test')"),
      None,
      None,
      ".",
    ));
  }

  #[test]
  fn optimize_keeps_query_after_id() {
    // The first `Query` eval parses the id's URL and builds a param map, so a cheap
    // `Id` check must run (and get to short-circuit) before that parse is forced —
    // even though `Query` has fewer nodes, it is not cheaper to evaluate.
    let mut expr = FilterExpr::And(vec![
      FilterExpr::Id(StringOrRegex::Regex("target".into())),
      FilterExpr::Query("raw".to_string(), QueryValue::Boolean(true)),
    ]);
    optimize_filter_expr(&mut expr);
    match &expr {
      FilterExpr::And(args) => {
        assert!(matches!(args[0], FilterExpr::Id(_)), "Id must precede Query, got {args:?}");
        assert!(matches!(args[1], FilterExpr::Query(..)));
      }
      other => panic!("expected And, got {other:?}"),
    }
  }

  #[test]
  fn optimize_ranks_compound_by_costliest_leaf() {
    // A nested `or` containing a `Code` regex must not be ranked cheap by its
    // `ModuleType` leaf: a non-matching module has to reject at the cheap `Id` check
    // first, not fall into the full-source `Code` scan inside the `or`.
    let mut expr = FilterExpr::And(vec![
      FilterExpr::Id(StringOrRegex::Regex("target".into())),
      FilterExpr::Or(vec![
        FilterExpr::ModuleType("css".to_string()),
        FilterExpr::Code(StringOrRegex::Regex("needle".into())),
      ]),
    ]);
    optimize_filter_expr(&mut expr);
    match &expr {
      FilterExpr::And(args) => {
        assert!(
          matches!(args[0], FilterExpr::Id(_)),
          "Id must precede the Code-bearing Or, got {args:?}"
        );
        assert!(matches!(args[1], FilterExpr::Or(_)));
      }
      other => panic!("expected And, got {other:?}"),
    }
  }

  #[test]
  fn optimize_reorders_operands_cheapest_first() {
    // and(code(/regex/), id(/regex/)) — the expensive `Code` leaf is authored first.
    let mut expr = FilterExpr::And(vec![
      FilterExpr::Code(StringOrRegex::Regex("import".into())),
      FilterExpr::Id(StringOrRegex::Regex("node_modules".into())),
    ]);
    optimize_filter_expr(&mut expr);
    // After optimization the cheap `Id` leaf must come first so it can short-circuit.
    match &expr {
      FilterExpr::And(args) => {
        assert!(
          matches!(args[0], FilterExpr::Id(_)),
          "expected Id first after reorder, got {args:?}"
        );
        assert!(matches!(args[1], FilterExpr::Code(_)));
      }
      other => panic!("expected And, got {other:?}"),
    }
  }

  #[test]
  fn optimize_preserves_semantics() {
    // Build the same And in both orders; after optimization they must agree with
    // each other and with the original evaluation for every input.
    let build = |code_first: bool| {
      let id = FilterExpr::Id(StringOrRegex::Regex("node_modules".into()));
      let code = FilterExpr::Not(Box::new(FilterExpr::Code(StringOrRegex::Regex("import".into()))));
      let args = if code_first { vec![code, id] } else { vec![id, code] };
      FilterExpr::And(args)
    };
    let inputs = [
      (Some("/node_modules/bar.js"), Some("console.log('x')")),
      (Some("/node_modules/bar.js"), Some("import('x')")),
      (Some("/src/foo.js"), Some("console.log('x')")),
      (Some("/src/foo.js"), Some("import('x')")),
    ];
    for (id, code) in inputs {
      let baseline = filter_expr_interpreter(
        &build(false),
        id,
        code,
        None,
        None,
        ".",
        &mut InterpreterCtx::default(),
      );
      for code_first in [false, true] {
        let mut expr = build(code_first);
        optimize_filter_expr(&mut expr);
        let got =
          filter_expr_interpreter(&expr, id, code, None, None, ".", &mut InterpreterCtx::default());
        assert_eq!(got, baseline, "mismatch for id={id:?} code={code:?} code_first={code_first}");
      }
    }
  }

  #[test]
  fn optimize_keeps_code_checks_in_authored_order() {
    // Both `Code` variants scan the full source; which is cheaper depends on the pattern
    // (an anchored regex can fail fast, an absent string scans everything), so their
    // authored order must be preserved rather than reordered by variant.
    let mut expr = FilterExpr::Or(vec![
      FilterExpr::Code(StringOrRegex::Regex("^import".into())),
      FilterExpr::Code(StringOrRegex::String("rare-marker".to_string())),
    ]);
    optimize_filter_expr(&mut expr);
    match &expr {
      FilterExpr::Or(args) => {
        assert!(
          matches!(args[0], FilterExpr::Code(StringOrRegex::Regex(_))),
          "authored Code order must be preserved, got {args:?}"
        );
        assert!(matches!(args[1], FilterExpr::Code(StringOrRegex::String(_))));
      }
      other => panic!("expected Or, got {other:?}"),
    }
  }
}
