use std::borrow::Cow;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode, utf8_percent_encode};
use rolldown_utils::url::clean_url;

const ENCODE_URI_SET: &AsciiSet = &CONTROLS
  .add(b'%')
  .add(b' ')
  .remove(b'!')
  .remove(b'#')
  .remove(b'$')
  .remove(b'&')
  .remove(b'\'')
  .remove(b'(')
  .remove(b')')
  .remove(b'*')
  .remove(b'+')
  .remove(b',')
  .remove(b'-')
  .remove(b'.')
  .remove(b'/')
  .remove(b':')
  .remove(b';')
  .remove(b'=')
  .remove(b'?')
  .remove(b'@')
  .remove(b'_')
  .remove(b'~');

pub fn encode_uri_path(uri: String) -> String {
  if uri.starts_with("data:") {
    uri
  } else {
    let path = clean_url(&uri);
    let mut encoded_uri = utf8_percent_encode(path, ENCODE_URI_SET).to_string();
    if path.len() != uri.len() {
      encoded_uri.push_str(&uri[path.len()..]);
    }
    encoded_uri
  }
}

pub fn decode_uri(uri: &str) -> Cow<'_, str> {
  percent_decode(uri.as_bytes()).decode_utf8_lossy()
}
