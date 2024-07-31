use std::sync::LazyLock;

use regex::Regex;

pub fn is_data_url(s: &str) -> bool {
  s.trim_start().starts_with("data:")
}

static DATA_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new("^data:([^/]+\\/[^;]+)(;charset=[^;]+)?(;base64)?,([\\s\\S]*)$").unwrap()
});

pub struct ParsedDataUrl<'a> {
  pub mime: &'a str,
  pub is_base64: bool,
  pub data: &'a str,
}

pub fn parse_data_url(dataurl: &str) -> Option<ParsedDataUrl> {
  let captures = DATA_URL_RE.captures(dataurl)?;
  let mime = captures.get(1).map(|m| m.as_str())?;
  let is_base64 = captures.get(3).is_some();
  let data = captures.get(4).map(|m| m.as_str())?;
  Some(ParsedDataUrl { mime, is_base64, data })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_plain_text() {
    let dataurl = "data:text/plain;charset=utf-8,hello%20world";
    let ParsedDataUrl { mime, is_base64, data } = parse_data_url(dataurl).unwrap();
    assert_eq!(mime, "text/plain");
    assert!(!is_base64);
    assert_eq!(data, "hello%20world");
  }

  #[test]
  fn test_json_0() {
    let dataurl = "data:application/json,\"%31%32%33\"";
    let ParsedDataUrl { mime, is_base64, data } = parse_data_url(dataurl).unwrap();
    assert_eq!(mime, "application/json");
    assert!(!is_base64);
    assert_eq!(data, "\"%31%32%33\"");
  }

  #[test]
  fn test_json_1() {
    let dataurl = "data:application/json;base64,eyJ3b3JrcyI6dHJ1ZX0=";
    let ParsedDataUrl { mime, is_base64, data } = parse_data_url(dataurl).unwrap();
    assert_eq!(mime, "application/json");
    assert!(is_base64);
    assert_eq!(data, "eyJ3b3JrcyI6dHJ1ZX0=");
  }

  #[test]
  fn test_json_2() {
    let dataurl = "data:application/json;charset=UTF-8,%31%32%33";
    let ParsedDataUrl { mime, is_base64, data } = parse_data_url(dataurl).unwrap();
    assert_eq!(mime, "application/json");
    assert!(!is_base64);
    assert_eq!(data, "%31%32%33");
  }

  #[test]
  fn test_json_3() {
    let dataurl = "data:application/json;charset=UTF-8;base64,eyJ3b3JrcyI6dHJ1ZX0=";
    let ParsedDataUrl { mime, is_base64, data } = parse_data_url(dataurl).unwrap();
    assert_eq!(mime, "application/json");
    assert!(is_base64);
    assert_eq!(data, "eyJ3b3JrcyI6dHJ1ZX0=");
  }
}
