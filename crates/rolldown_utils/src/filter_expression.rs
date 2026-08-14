use std::{borrow::Cow, sync::OnceLock};

use fast_glob::glob_match;
use rustc_hash::FxHashMap;

use crate::{
  js_regex::HybridRegex,
  pattern_filter::{
    StringOrRegex, StringOrRegexMatchKind, get_matcher_string, glob_matcher_depends_on_cwd,
    normalize_path,
  },
  url::{clean_url, get_query},
};

#[derive(Debug)]
pub enum FilterExpr {
  Or(Vec<FilterExpr>),
  And(Vec<FilterExpr>),
  Not(Box<FilterExpr>),
  Code(StringOrRegex),
  Id(IdMatcher),
  ImporterId(IdMatcher),
  CleanUrl(Box<FilterExpr>),
  ModuleType(String),
  Query(String, QueryValue),
}

/// Path matcher for `Id`/`ImporterId` leaves. Wraps a [`StringOrRegex`] and, for
/// glob (`String`) patterns, memoizes the resolved matcher so `get_matcher_string`'s
/// allocation doesn't run on every module tested.
///
/// The cache is cwd-*correct*, not cwd-*assuming*. A `**`-prefixed or absolute glob
/// resolves independently of `cwd`, so it is cached once and reused for any `cwd`. A
/// relative glob's resolution joins `cwd`, so it is cached together with the `cwd` it
/// was resolved for and re-resolved from scratch whenever a later call presents a
/// different `cwd`. Matching is therefore behaviorally identical to resolving on every
/// call — no `cwd` is ever baked in — while the repeated allocation is still avoided in
/// the common case where `cwd` is constant for a build.
#[derive(Debug)]
pub struct IdMatcher {
  pattern: StringOrRegex,
  glob_cache: OnceLock<ResolvedGlob>,
}

#[derive(Debug)]
enum ResolvedGlob {
  /// A `**`-prefixed or absolute glob: resolution ignores `cwd`, valid for any `cwd`.
  CwdIndependent(Box<str>),
  /// A relative glob: resolution joined `cwd`, so it is only valid for that `cwd`.
  ScopedToCwd { cwd: Box<str>, matcher: Box<str> },
}

impl ResolvedGlob {
  fn resolve(glob: &str, cwd: &str) -> Self {
    let matcher = Box::from(get_matcher_string(glob, cwd).as_ref());
    if glob_matcher_depends_on_cwd(glob) {
      Self::ScopedToCwd { cwd: Box::from(cwd), matcher }
    } else {
      Self::CwdIndependent(matcher)
    }
  }
}

impl IdMatcher {
  fn test(&self, value: &str, cwd: &str) -> bool {
    match &self.pattern {
      StringOrRegex::Regex(re) => re.matches(value),
      StringOrRegex::String(glob) => {
        match self.glob_cache.get_or_init(|| ResolvedGlob::resolve(glob, cwd)) {
          ResolvedGlob::CwdIndependent(matcher) => glob_match(matcher.as_bytes(), value.as_bytes()),
          ResolvedGlob::ScopedToCwd { cwd: resolved_cwd, matcher }
            if resolved_cwd.as_bytes() == cwd.as_bytes() =>
          {
            glob_match(matcher.as_bytes(), value.as_bytes())
          }
          // Relative glob first resolved under a different `cwd`: resolve fresh so we
          // never match against a stale, wrong-`cwd` matcher.
          ResolvedGlob::ScopedToCwd { .. } => {
            glob_match(get_matcher_string(glob, cwd).as_bytes(), value.as_bytes())
          }
        }
      }
    }
  }
}

impl From<StringOrRegex> for IdMatcher {
  fn from(pattern: StringOrRegex) -> Self {
    Self { pattern, glob_cache: OnceLock::new() }
  }
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

/// Every leaf is total over the inputs a hook can supply. Hooks pass `None` for the
/// inputs they don't have — `renderChunk` has no `id`, `resolveId`/`load` have no
/// `code` — and each hook's filter is typed as an arbitrary `TopLevelFilterExpression[]`,
/// so a leaf reading an input its hook never supplies is reachable. Such a leaf reports
/// "no match" instead of panicking.
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
      // When code is None (e.g. the `resolveId`/`load` hooks), return false (no match)
      code.is_some_and(|code| pattern.test(code, &StringOrRegexMatchKind::Code))
    }
    FilterExpr::Id(id_pattern) => {
      // When id is None (e.g. the `renderChunk` hook), return false (no match)
      id.is_some_and(|id| id_pattern.test(id, cwd))
    }
    FilterExpr::ImporterId(id_pattern) => {
      // When importer_id is None (e.g., entry files), return false (no match)
      match importer_id {
        Some(importer) => id_pattern.test(importer, cwd),
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
        // When id is None (e.g. the `renderChunk` hook) there is no query at all —
        // which is an *empty* query, not a missing one. Reporting "no match" for the
        // whole leaf instead would make `query(k, false)` ("the id does not carry
        // `?k`") disagree with the equivalent `not(query(k, true))`, and would leave
        // `query(k, true)` and `query(k, false)` both false at once — a state no real
        // id can produce, since they are exact complements everywhere else.
        let query_string = id.map(get_query).unwrap_or("");
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
      Token::Id => Ok(FilterExpr::Id(pop_string_or_regex(tokens, "Id")?.into())),
      Token::ImporterId => {
        Ok(FilterExpr::ImporterId(pop_string_or_regex(tokens, "ImporterId")?.into()))
      }
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
    Some(Token::Include) => Ok(FilterExprKind::Include(rec(&mut tokens)?)),
    Some(Token::Exclude) => Ok(FilterExprKind::Exclude(rec(&mut tokens)?)),
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

  use super::{IdMatcher, filter_exprs_interpreter, parse};

  #[test]
  fn test_filter_expr_interpreter() {
    // https://github.com/vitejs/rolldown-vite/blob/fef84b75dbb35a6ec27debdc0dced1d0f1250eb8/packages/vite/src/node/plugins/importAnalysisBuild.ts?plain=1#L242-L244
    let expr = FilterExpr::And(vec![
      FilterExpr::Id(StringOrRegex::Regex("node_modules".into()).into()),
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
      let expr = FilterExpr::Id(StringOrRegex::Regex("src/".into()).into());

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
  fn missing_id_is_a_non_match_not_a_panic() {
    // `renderChunk` evaluates filters with `id: None`, and `bindingifyRenderChunkFilter`
    // accepts an arbitrary `TopLevelFilterExpression[]` (only `importerId` is rejected),
    // so an `id`/`query` leaf is reachable there and must report "no match".
    let id_only = FilterExpr::Id(StringOrRegex::Regex("target".into()).into());
    assert!(!filter_expr_interpreter(
      &id_only,
      None,
      Some("console.log('test')"),
      None,
      None,
      ".",
      &mut InterpreterCtx::default()
    ));

    let query_only = FilterExpr::Query("raw".to_string(), QueryValue::Boolean(true));
    assert!(!filter_expr_interpreter(
      &query_only,
      None,
      Some("console.log('test')"),
      None,
      None,
      ".",
      &mut InterpreterCtx::default()
    ));
  }

  #[test]
  fn a_missing_id_reads_as_an_empty_query_not_a_dead_leaf() {
    // `query(k, false)` asks "the id does not carry `?k`". With no id at all there is
    // no query, so it holds — and must keep agreeing with the equivalent
    // `not(query(k, true))`, and stay the exact complement of `query(k, true)`.
    let eval = |expr: &FilterExpr| {
      filter_expr_interpreter(
        expr,
        None,
        Some("console.log('test')"),
        None,
        None,
        ".",
        &mut InterpreterCtx::default(),
      )
    };
    let present = FilterExpr::Query("raw".to_string(), QueryValue::Boolean(true));
    let absent = FilterExpr::Query("raw".to_string(), QueryValue::Boolean(false));

    assert!(!eval(&present), "no id carries `?raw`");
    assert!(eval(&absent), "`query(k, false)` holds when there is no query");
    assert_eq!(
      eval(&absent),
      eval(&FilterExpr::Not(Box::new(FilterExpr::Query(
        "raw".to_string(),
        QueryValue::Boolean(true)
      )))),
      "`query(k, false)` must agree with `not(query(k, true))`"
    );

    // The string/regex variants have no key to compare against, so they still miss.
    assert!(!eval(&FilterExpr::Query("raw".to_string(), QueryValue::String("1".to_string()))));
  }

  #[test]
  fn missing_code_is_a_non_match_not_a_panic() {
    // Mirror case: `resolveId`/`load` evaluate filters with `code: None`.
    let code_only = FilterExpr::Code(StringOrRegex::Regex("import".into()));
    assert!(!filter_expr_interpreter(
      &code_only,
      Some("/target.js"),
      None,
      None,
      None,
      ".",
      &mut InterpreterCtx::default()
    ));
  }

  #[test]
  fn id_matcher_relative_glob_is_cwd_correct() {
    // A relative glob resolves against cwd. The same matcher instance must stay
    // correct when evaluated under different cwds — no cwd may be baked into the
    // cache (the earlier caching bug kept matching the first cwd seen forever).
    let matcher = IdMatcher::from(StringOrRegex::String("src/*.ts".to_string()));
    // Under cwd `/a`, only ids under `/a/src/` match.
    assert!(matcher.test("/a/src/foo.ts", "/a"));
    assert!(!matcher.test("/b/src/foo.ts", "/a"));
    // Same instance under cwd `/b`: it must re-resolve; now only `/b/src/` matches.
    assert!(matcher.test("/b/src/foo.ts", "/b"));
    assert!(!matcher.test("/a/src/foo.ts", "/b"));
  }

  #[test]
  fn id_matcher_cwd_independent_glob_is_reused_across_cwds() {
    // A `**`-prefixed glob resolves without consulting cwd, so it is cached once and
    // handed back under *every* cwd — the only arm that reuses a single entry that
    // way. That reuse is safe only while `get_matcher_string` and
    // `glob_matcher_depends_on_cwd` agree on which globs read cwd; should they ever
    // drift apart, this arm would keep matching against whichever directory happened
    // to be current when the cache was first filled, with nothing to catch it.
    let matcher = IdMatcher::from(StringOrRegex::String("**/*.ts".to_string()));
    // Fill the cache under `/a`.
    assert!(matcher.test("/a/src/foo.ts", "/a"));
    // The same instance under a different cwd must answer identically: matching is
    // decided by the glob alone, never by the cwd the entry was resolved for.
    assert!(matcher.test("/b/src/foo.ts", "/b"));
    assert!(matcher.test("/b/src/foo.ts", "/a"));
    assert!(matcher.test("/a/src/foo.ts", "/b"));
    // A non-match stays a non-match under either cwd.
    assert!(!matcher.test("/a/src/foo.js", "/a"));
    assert!(!matcher.test("/a/src/foo.js", "/b"));
  }
}
