// Ported from https://github.com/KermanX/vite/blob/main/packages/vite/src/node/plugins/dynamicImportVars.ts#L67-L108

use regex::Regex;
use std::sync::LazyLock;

const REQUEST_QUERY_MAYBE_ESCAPED_SPLIT_RE: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"\\?\?";
  Regex::new(pattern).expect("failed to compile regex")
});
const REQUEST_QUERY_SPLIT_RE: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"\?";
  Regex::new(pattern).expect("failed to compile regex")
});
const URL_RE: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"(\?|&)url(?:&|$)";
  Regex::new(pattern).expect("failed to compile regex")
});
const RAW_RE: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"(\?|&)raw(?:&|$)";
  Regex::new(pattern).expect("failed to compile regex")
});
const WORKER_OR_SHARED_WORKER_RE: LazyLock<Regex> = LazyLock::new(|| {
  let pattern = r"(?:\?|&)(worker|sharedworker)(?:&|$)";
  Regex::new(pattern).expect("failed to compile regex")
});

#[derive(Debug, PartialEq)]
pub(crate) struct DynamicImportRequest {
  pub query: String,
  pub import: bool,
}

#[derive(Debug, PartialEq)]
pub(crate) struct DynamicImportPattern {
  pub glob_params: Option<DynamicImportRequest>,
  pub user_pattern: String,
  pub raw_pattern: String,
}

pub(crate) fn parse_pattern(pattern: &str) -> DynamicImportPattern {
  let user_pattern_end =
    REQUEST_QUERY_MAYBE_ESCAPED_SPLIT_RE.find(pattern).map_or(pattern.len(), |m| m.start());
  let user_pattern = &pattern[..user_pattern_end];
  let raw_pattern_end = REQUEST_QUERY_SPLIT_RE.find(pattern);
  let (raw_pattern, search) = match raw_pattern_end {
    Some(m) => (&pattern[..m.start()], &pattern[m.start()..]),
    None => (pattern, ""),
  };
  DynamicImportPattern {
    glob_params: if search.is_empty() {
      None
    } else {
      Some(DynamicImportRequest {
        query: search.to_string(),
        import: URL_RE.is_match(search)
          || RAW_RE.is_match(search)
          || WORKER_OR_SHARED_WORKER_RE.is_match(search),
      })
    },
    user_pattern: user_pattern.to_string(),
    raw_pattern: raw_pattern.to_string(),
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn basic() {
    assert_eq!(
      super::parse_pattern("./mods/*.js"),
      super::DynamicImportPattern {
        glob_params: None,
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
  }

  #[test]
  fn with_query() {
    assert_eq!(
      super::parse_pattern("./mods/*.js?foo=bar"),
      super::DynamicImportPattern {
        glob_params: Some(super::DynamicImportRequest {
          query: "?foo=bar".to_string(),
          import: false,
        }),
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
  }

  #[test]
  fn with_raw_query() {
    assert_eq!(
      super::parse_pattern("./mods/*.js?raw"),
      super::DynamicImportPattern {
        glob_params: Some(super::DynamicImportRequest { query: "?raw".to_string(), import: true }),
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
  }

  #[test]
  fn with_url_query() {
    assert_eq!(
      super::parse_pattern("./mods/*.js?url"),
      super::DynamicImportPattern {
        glob_params: Some(super::DynamicImportRequest { query: "?url".to_string(), import: true }),
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
  }

  #[test]
  fn with_worker_query() {
    assert_eq!(
      super::parse_pattern("./mods/*.js?worker"),
      super::DynamicImportPattern {
        glob_params: Some(super::DynamicImportRequest {
          query: "?worker".to_string(),
          import: true,
        }),
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
    assert_eq!(
      super::parse_pattern("./mods/*.js?sharedworker"),
      super::DynamicImportPattern {
        glob_params: Some(super::DynamicImportRequest {
          query: "?sharedworker".to_string(),
          import: true,
        }),
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
  }
}
