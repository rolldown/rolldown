use mime::Mime;
use std::{path::Path, str::FromStr};

// TODO implement it in a better way
pub fn is_texture(data: &[u8]) -> bool {
  data.iter().all(|&byte| {
    byte.is_ascii_graphic() || byte.is_ascii_whitespace() || byte == b'\n' || byte == b'\r'
  })
}

pub fn guess_mime(path: &Path, data: &[u8]) -> anyhow::Result<Mime> {
  if let Some(guessed) = mime_guess::from_path(path).first() {
    return Ok(guessed);
  }

  if let Some(inferred) = infer::get(data) {
    return Ok(Mime::from_str(inferred.mime_type())?);
  }

  if is_texture(data) || data.is_empty() {
    return Ok(mime::TEXT_PLAIN);
  }

  // Fallback to application/octet-stream
  Ok(mime::APPLICATION_OCTET_STREAM)
}
