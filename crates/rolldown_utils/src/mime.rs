use crate::light_guess::{self, RawMimeExt};
use mime::Mime;
use std::{fmt::Display, path::Path, str::FromStr};

const SNIFF_LEN: usize = 512;

/// Mirrors the text signature in Go's `http.DetectContentType`.
///
/// MIME sniffing only examines the first 512 bytes and treats bytes in these
/// control ranges as a binary signal. Other bytes, including invalid UTF-8,
/// are considered text-like.
#[inline]
fn is_text_like(data: &[u8]) -> bool {
  data
    .iter()
    .take(SNIFF_LEN)
    .all(|byte| !matches!(*byte, 0x00..=0x08 | 0x0B | 0x0E..=0x1A | 0x1C..=0x1F))
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
  guess_mime_impl(path, data, true)
}

/// Same as `guess_mime` but skips the text fallback check
pub fn guess_mime_skip_utf8_check(path: &Path, data: &[u8]) -> anyhow::Result<Mime> {
  guess_mime_impl(path, data, false).map(|v| v.mime)
}

fn guess_mime_impl(path: &Path, data: &[u8], check_text_fallback: bool) -> anyhow::Result<MimeExt> {
  if let Ok(guessed) = light_guess::try_from_path(path) {
    return Ok(guessed);
  }

  if let Some(inferred) = infer::get(data) {
    return Ok((Mime::from_str(inferred.mime_type())?, false).into());
  }

  if check_text_fallback && is_text_like(data) {
    return Ok((mime::TEXT_PLAIN, true).into());
  }

  // Fallback to application/octet-stream
  Ok((mime::APPLICATION_OCTET_STREAM, false).into())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn text_like_fallback_accepts_non_utf8_bytes() {
    let guessed = guess_mime(Path::new("unknown.bin"), &[0xFF]).unwrap();
    assert_eq!(guessed.mime, mime::TEXT_PLAIN);
    assert!(guessed.is_utf8_encoded);
  }

  #[test]
  fn binary_fallback_rejects_control_bytes() {
    let guessed = guess_mime(Path::new("unknown.bin"), &[0x00]).unwrap();
    assert_eq!(guessed.mime, mime::APPLICATION_OCTET_STREAM);
    assert!(!guessed.is_utf8_encoded);
  }

  #[test]
  fn content_signature_is_checked_before_text_fallback() {
    let guessed =
      guess_mime(Path::new("unknown.txt"), &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
        .unwrap();
    assert!(matches!(guessed.mime.subtype(), mime::PNG));
    assert!(!guessed.is_utf8_encoded);
  }
}
