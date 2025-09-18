use crate::light_guess::{self, RawMimeExt};
use mime::Mime;
use std::{fmt::Display, path::Path, str::FromStr};

#[inline]
fn is_valid_utf8(data: &[u8]) -> bool {
  simdutf8::basic::from_utf8(data).is_ok()
}

#[derive(Debug)]
pub struct MimeExt {
  pub mime: Mime,
  pub is_utf8_encoded: bool,
}

impl Display for MimeExt {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.mime)?;
    if self.is_utf8_encoded {
      write!(f, ";charset=utf-8")?;
    }
    Ok(())
  }
}

impl From<(Mime, bool)> for MimeExt {
  fn from(value: (Mime, bool)) -> Self {
    Self { mime: value.0, is_utf8_encoded: value.1 }
  }
}

impl TryFrom<RawMimeExt> for MimeExt {
  fn try_from(raw_mime_ext: RawMimeExt) -> Result<Self, Self::Error> {
    let mime = Mime::from_str(raw_mime_ext.mime_str)?;
    Ok(MimeExt { mime, is_utf8_encoded: raw_mime_ext.is_utf8_encoded })
  }

  type Error = anyhow::Error;
}

// second param is whether the data is utf8 encoded
pub fn guess_mime(path: &Path, data: &[u8]) -> anyhow::Result<MimeExt> {
  if let Ok(guessed) = light_guess::try_from_path(path) {
    return Ok(guessed);
  }

  if let Some(inferred) = infer::get(data) {
    return Ok((Mime::from_str(inferred.mime_type())?, false).into());
  }

  if is_valid_utf8(data) || data.is_empty() {
    return Ok((mime::TEXT_PLAIN, true).into());
  }

  // Fallback to application/octet-stream
  Ok((mime::APPLICATION_OCTET_STREAM, false).into())
}
