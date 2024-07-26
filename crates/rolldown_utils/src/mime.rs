use crate::light_guess;
use mime::Mime;
use std::{path::Path, str::FromStr};

fn is_texture(data: &[u8]) -> bool {
  std::str::from_utf8(data).is_ok()
}

pub fn guess_mime(path: &Path, data: &[u8]) -> anyhow::Result<Mime> {
  if let Ok(guessed) = light_guess::try_from_path(path) {
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
