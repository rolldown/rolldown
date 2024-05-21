use regex::Regex;

static VALID_IDENTIFIER_RE: once_cell::sync::Lazy<Regex> =
  once_cell::sync::Lazy::new(|| Regex::new(r"^[a-zA-Z_$][a-zA-Z0-9_$]*$").unwrap());

pub fn is_validate_identifier_name(name: &str) -> bool {
  VALID_IDENTIFIER_RE.is_match(name)
}
