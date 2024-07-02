use mime::Mime;
use std::{path::Path, str::FromStr};

pub fn guess_mime(path: &Path, data: &[u8]) -> anyhow::Result<Mime> {
  if let Some(guessed) = mime_guess::from_path(path).first() {
    return Ok(guessed);
  }

  if let Some(inferred) = infer::get(data) {
    return Ok(Mime::from_str(inferred.mime_type())?);
  }

  // Fallback to application/octet-stream
  Ok(mime::APPLICATION_OCTET_STREAM)
}
