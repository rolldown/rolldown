use crate::percent_encoding::encode_as_percent_escaped;
use mime::Mime;
use regex::{Captures, Regex};
use std::str::FromStr;
use std::sync::LazyLock;

fn parse_dataurl(dataurl: &str) -> Option<Captures> {
  static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^data:([^/]+\\/[^;]+)(;charset=[^;]+)?(;base64)?,([\\s\\S]*)$").unwrap()
  });
  RE.captures(dataurl)
}

/// Returns shorter of either a base64-encoded or percent-escaped data URL
// adapted from https://github.com/evanw/esbuild/blob/67cbf87a4909d87a902ca8c3b69ab5330defab0a/internal/helpers/dataurl.go by @ikkz
pub fn encode_as_shortest_dataurl(mime: &Mime, buf: &[u8]) -> String {
  let base64 = crate::base64::to_standard_base64(buf);
  let base64_url = format!("base64,{base64}");

  let body = match encode_as_percent_escaped(buf) {
    Some(percent_url) if percent_url.len() < base64_url.len() => {
      format!("charset=utf-8,{percent_url}")
    }
    _ => base64_url,
  };

  format!("data:{mime};{body}")
}

// Port from https://github.com/vitejs/vite/blob/main/packages/vite/src/node/plugins/dataUri.ts.
pub fn deserialize_dataurl(s: &str) -> anyhow::Result<(Mime, Vec<u8>)> {
  if !s.starts_with("data:") {
    return Err(anyhow::anyhow!("Invalid dataurl"));
  }
  let captures = parse_dataurl(s).ok_or_else(|| anyhow::anyhow!("Invalid dataurl"))?;
  let mime = captures.get(1).unwrap().as_str();
  let is_base64 = captures.get(3).is_some();
  let data = captures.get(4).unwrap().as_str();
  let mime = Mime::from_str(mime).unwrap_or(Mime::from_str("text/plain").unwrap());
  let data = if is_base64 {
    crate::base64::from_standard_base64(data.as_bytes())?
  } else {
    let text = urlencoding::decode(data).unwrap();
    text.as_bytes().to_vec()
  };
  Ok((mime, data))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_plain_text() {
    let dataurl = "data:text/plain;charset=utf-8,hello%20world";
    let (mime, data) = deserialize_dataurl(dataurl).unwrap();
    assert_eq!(mime, Mime::from_str("text/plain").unwrap());
    assert_eq!(data, b"hello world");
  }

  #[test]
  fn test_json_0() {
    let dataurl = "data:application/json,\"%31%32%33\"";
    let (mime, data) = deserialize_dataurl(dataurl).unwrap();
    assert_eq!(mime, Mime::from_str("application/json").unwrap());
    assert_eq!(data, b"\"123\"");
  }

  #[test]
  fn test_json_1() {
    let dataurl = "data:application/json;base64,eyJ3b3JrcyI6dHJ1ZX0=";
    let (mime, data) = deserialize_dataurl(dataurl).unwrap();
    assert_eq!(mime, Mime::from_str("application/json").unwrap());
    assert_eq!(data, b"{\"works\":true}");
  }

  #[test]
  fn test_json_2() {
    let dataurl = "data:application/json;charset=UTF-8,%31%32%33";
    let (mime, data) = deserialize_dataurl(dataurl).unwrap();
    assert_eq!(mime, Mime::from_str("application/json").unwrap());
    assert_eq!(data, b"123");
  }

  #[test]
  fn test_json_3() {
    let dataurl = "data:application/json;charset=UTF-8;base64,eyJ3b3JrcyI6dHJ1ZX0=";
    let (mime, data) = deserialize_dataurl(dataurl).unwrap();
    assert_eq!(mime, Mime::from_str("application/json").unwrap());
    assert_eq!(data, b"{\"works\":true}");
  }
}
