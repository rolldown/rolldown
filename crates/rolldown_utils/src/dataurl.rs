use crate::percent_encoding::encode_as_percent_escaped;
use mime::Mime;

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
