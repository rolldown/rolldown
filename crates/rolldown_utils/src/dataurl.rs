use nom::{
  IResult, Parser,
  bytes::complete::{tag, take_till, take_while},
  character::complete::char,
  combinator::{map, opt, recognize},
  error::Error,
  sequence::preceded,
};

use crate::{concat_string, mime::MimeExt, percent_encoding::encode_as_percent_escaped};

pub fn is_data_url(s: &str) -> bool {
  s.trim_start().starts_with("data:")
}

pub struct ParsedDataUrl<'a> {
  pub mime: &'a str,
  pub is_base64: bool,
  pub data: &'a str,
}

#[inline]
// Parse the data URL with a more flexible approach
fn parse_data_url_nom(input: &str) -> IResult<&str, ParsedDataUrl<'_>> {
  // Start with "data:" prefix
  let (input, _) = tag("data:")(input)?;

  // Parse the MIME type
  let (input, mime) = recognize(take_till(|c| c == ';' || c == ',')).parse(input)?;

  // Handle the case where there's no charset or base64 marker
  if let Ok((remaining, data)) =
    preceded(char::<_, Error<_>>(','), take_while(|_| true)).parse(input)
  {
    return Ok((
      remaining,
      ParsedDataUrl { mime: mime.trim(), is_base64: false, data: data.trim() },
    ));
  }

  // Parse optional charset
  let (input, _) =
    opt(preceded(tag(";charset="), take_till(|c| c == ';' || c == ','))).parse(input)?;

  // Check for base64 encoding
  let (input, is_base64) = map(opt(tag(";base64")), |opt_tag| opt_tag.is_some()).parse(input)?;

  // Parse the data part after the comma
  let (remaining, data) = preceded(char(','), take_while(|_| true)).parse(input)?;

  Ok((remaining, ParsedDataUrl { mime: mime.trim(), is_base64, data: data.trim() }))
}

pub fn parse_data_url(dataurl: &str) -> Option<ParsedDataUrl<'_>> {
  match parse_data_url_nom(dataurl) {
    Ok((_, parsed)) => Some(parsed),
    Err(_) => None,
  }
}

/// Returns shorter of either a base64-encoded or percent-escaped data URL
// adapted from https://github.com/evanw/esbuild/blob/67cbf87a4909d87a902ca8c3b69ab5330defab0a/internal/helpers/dataurl.go by @ikkz
pub fn encode_as_shortest_dataurl(mime_ext: &MimeExt, buf: &[u8]) -> String {
  let base64 = crate::base64::to_standard_base64(buf);
  let mime_ext_string = mime_ext.to_string();
  let base64_url = concat_string!("data:", mime_ext_string, ";base64,", base64);

  match encode_as_percent_escaped(buf)
    .map(|encoded| concat_string!("data:", mime_ext_string, ",", encoded))
  {
    Some(percent_url) if percent_url.len() < base64_url.len() => percent_url,
    _ => base64_url,
  }
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
