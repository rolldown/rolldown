/// Hoist `@charset` and `@import` rules to the top of a CSS string.
///
/// Per the CSS specification:
/// - `@charset` must be the very first rule
/// - `@import` rules must precede all other rules (except `@charset` and `@layer`)
///
/// When code-split CSS chunks are concatenated, these at-rules may end up in
/// the wrong position. This function moves them to their correct locations.
pub fn hoist_at_rules(css: &str) -> String {
  let mut charset_rules: Vec<&str> = Vec::new();
  let mut import_rules: Vec<&str> = Vec::new();
  let mut other_lines: Vec<&str> = Vec::new();

  for line in css.lines() {
    let trimmed = line.trim();
    if trimmed.starts_with("@charset ") {
      charset_rules.push(line);
    } else if trimmed.starts_with("@import ") {
      import_rules.push(line);
    } else {
      other_lines.push(line);
    }
  }

  // If no hoisting needed, return as-is (avoid allocation)
  if charset_rules.is_empty() && import_rules.is_empty() {
    return css.to_owned();
  }

  // Deduplicate @charset (only the first one is valid)
  charset_rules.truncate(1);

  let mut result = String::with_capacity(css.len());

  for line in &charset_rules {
    result.push_str(line);
    result.push('\n');
  }
  for line in &import_rules {
    result.push_str(line);
    result.push('\n');
  }
  for (i, line) in other_lines.iter().enumerate() {
    result.push_str(line);
    if i < other_lines.len() - 1 {
      result.push('\n');
    }
  }

  // Preserve trailing newline if original had one
  if css.ends_with('\n') && !result.ends_with('\n') {
    result.push('\n');
  }

  result
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_no_at_rules() {
    let input = ".foo { color: red; }\n.bar { color: blue; }\n";
    assert_eq!(hoist_at_rules(input), input);
  }

  #[test]
  fn test_hoist_import() {
    let input = ".foo { color: red; }\n@import url(\"base.css\");\n.bar { color: blue; }\n";
    let result = hoist_at_rules(input);
    assert!(result.starts_with("@import"));
  }

  #[test]
  fn test_hoist_charset_and_import() {
    let input = ".foo { color: red; }\n@charset \"UTF-8\";\n@import url(\"base.css\");\n";
    let result = hoist_at_rules(input);
    assert!(result.starts_with("@charset"));
    let import_pos = result.find("@import").unwrap();
    let charset_pos = result.find("@charset").unwrap();
    assert!(charset_pos < import_pos);
  }

  #[test]
  fn test_deduplicate_charset() {
    let input = "@charset \"UTF-8\";\n@charset \"ASCII\";\n.foo { color: red; }\n";
    let result = hoist_at_rules(input);
    assert_eq!(result.matches("@charset").count(), 1);
  }
}
