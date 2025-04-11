// Ported from https://github.com/KermanX/vite/blob/main/packages/vite/src/node/plugins/dynamicImportVars.ts#L67-L108

/// Finds the position of the first occurrence of a question mark pattern in a string.
/// Handles both escaped and unescaped question marks: \? or ?
///
/// Returns the position of the found pattern, or None if not found.
fn find_query_marker_maybe_escaped(s: &str) -> Option<usize> {
  // Use chars().enumerate() to efficiently check each character
  let mut char_iter = s.char_indices().peekable();

  while let Some((pos, ch)) = char_iter.next() {
    match ch {
      '?' => return Some(pos), // Unescaped question mark
      '\\' => {
        // Check if next character is '?'
        if let Some(&(_, '?')) = char_iter.peek() {
          return Some(pos); // Escaped question mark
        }
      }
      _ => {}
    }
  }

  None
}

/// Checks if a query string contains any of the special query parameters:
/// url, raw, worker, or sharedworker
///
/// These parameters must appear either:
/// - After a ? or & character
/// - And be followed by either & or the end of the string
fn has_special_query_param(query: &str) -> bool {
  // Special parameter names we're looking for
  static SPECIAL_PARAMS: [&str; 4] = ["raw", "sharedworker", "url", "worker"];

  // Early return if query is empty or has no parameters
  if query.is_empty() {
    return false;
  }

  // Remove leading ? if present
  let query = query.strip_prefix('?').unwrap_or(query);

  // Empty query after stripping prefix
  if query.is_empty() {
    return false;
  }

  // Use any() for short-circuit evaluation - return true as soon as we find a match
  query.split('&').any(|param| {
    // Get parameter name (before any = sign)
    let param_name = match param.find('=') {
      Some(pos) => &param[..pos],
      None => param,
    };

    // Check if it's a special parameter
    SPECIAL_PARAMS.contains(&param_name)
  })
}

#[derive(Debug, PartialEq)]
pub struct DynamicImportRequest {
  pub query: String,
  pub import: bool,
}

#[derive(Debug, PartialEq)]
pub struct DynamicImportPattern {
  pub glob_params: Option<DynamicImportRequest>,
  pub user_pattern: String,
  pub raw_pattern: String,
}

pub fn parse_pattern(pattern: &str) -> DynamicImportPattern {
  // Find user pattern end (where the query part begins, possibly with escaped ?)
  let user_pattern_end = find_query_marker_maybe_escaped(pattern).unwrap_or(pattern.len());
  let user_pattern = &pattern[..user_pattern_end];

  // Find raw pattern end (where the query part begins, with unescaped ?)
  let (raw_pattern, search) = match pattern.find('?') {
    Some(pos) => (&pattern[..pos], &pattern[pos..]),
    None => (pattern, ""),
  };

  DynamicImportPattern {
    glob_params: if search.is_empty() {
      None
    } else {
      Some(DynamicImportRequest {
        query: search.to_string(),
        import: has_special_query_param(search),
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
  fn with_special_query() {
    assert_eq!(
      super::parse_pattern("./mods/*.js?foo=bar&raw"),
      super::DynamicImportPattern {
        glob_params: Some(super::DynamicImportRequest {
          query: "?foo=bar&raw".to_string(),
          import: true,
        }),
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
    assert_eq!(
      super::parse_pattern("./mods/*.js?url"),
      super::DynamicImportPattern {
        glob_params: Some(super::DynamicImportRequest { query: "?url".to_string(), import: true }),
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
    assert_eq!(
      super::parse_pattern("./mods/*.js?worker&c=d"),
      super::DynamicImportPattern {
        glob_params: Some(super::DynamicImportRequest {
          query: "?worker&c=d".to_string(),
          import: true,
        }),
        user_pattern: "./mods/*.js".to_string(),
        raw_pattern: "./mods/*.js".to_string(),
      }
    );
  }
}
