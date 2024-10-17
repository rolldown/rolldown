// Port from https://github.com/7086cmd/mime_more/blob/main/src/light_guess.rs
use mime::Mime;
use phf::{phf_map, Map};
use std::str::FromStr;

use crate::mime::MimeExt;
pub static MIME_TYPES: Map<&'static str, (&'static str, bool)> = phf_map! {
    // Text
    "css" => ("text/css", true),
    "htm" => ("text/html", true),
    "html" => ("text/html", true),
    "js" => ("text/javascript", true),
    "json" => ("application/json", true),
    "markdown" => ("text/markdown", true),
    "md" => ("text/markdown", true),
    "mjs" => ("text/javascript", true),

    // Images
    "avif" => ("image/avif", false),
    "gif" => ("image/gif", false),
    "jpg" => ("image/jpeg", false),
    "jpeg" => ("image/jpeg", false),
    "png" => ("image/png", false),
    "svg" => ("image/svg+xml",false),
    "webp" => ("image/webp", false),

    // Fonts
    "eot" => ("application/vnd.ms-fontobject", false),
    "otf" => ("font/otf", false),
    "sfnt" => ("font/sfnt", false),
    "ttf" => ("font/ttf", false),
    "woff" => ("font/woff", false),
    "woff2" => ("font/woff2", false),
    // Others
    "pdf" => ("application/pdf", false),
    "wasm" => ("application/wasm", false),
    "webmanifest" => ("application/manifest+json", false),
};

/// Adapted from:
/// - https://github.com/rolldown/rolldown/pull/1406/files#diff-4b612e077c82ae0e05e50eb0d419e02c05a04b83c6ac5440c0d0c9d0c38af942
/// - https://github.com/evanw/esbuild/blob/fc37c2fa9de2ad77476a6d4a8f1516196b90187e/internal/helpers/mime.go#L5
///
/// Thanks to @ikkz and @evanw for the inspiration.
pub fn mime_type_by_extension(ext: &str) -> Option<(&'static str, bool)> {
  MIME_TYPES.get(ext).copied()
}

pub fn try_from_ext(ext: &str) -> anyhow::Result<MimeExt> {
  mime_type_by_extension(ext)
    .ok_or_else(|| anyhow::Error::msg(format!("No mime type found for extension: {ext}")))
    .and_then(|(mime, is_utf8_encoded)| {
      let mime = Mime::from_str(mime)?;
      Ok(MimeExt::from((mime, is_utf8_encoded)))
    })
}

pub fn try_from_path(path: &std::path::Path) -> anyhow::Result<MimeExt> {
  if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
    try_from_ext(ext)
  } else {
    anyhow::bail!("No extension found for path: {:?}", path);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normal_extensions() {
    assert_eq!(mime_type_by_extension("txt"), None);
    assert_eq!(mime_type_by_extension("css").unwrap().0, "text/css");
    assert_eq!(mime_type_by_extension("html").unwrap().0, "text/html");
    assert_eq!(mime_type_by_extension("json").unwrap().0, "application/json");
    assert_eq!(mime_type_by_extension("png").unwrap().0, "image/png");
    assert_eq!(mime_type_by_extension("svg").unwrap().0, "image/svg+xml");
    assert_eq!(mime_type_by_extension("woff2").unwrap().0, "font/woff2");
    assert_eq!(mime_type_by_extension("pdf").unwrap().0, "application/pdf");
    assert_eq!(mime_type_by_extension("wasm").unwrap().0, "application/wasm");
    assert_eq!(mime_type_by_extension("webmanifest").unwrap().0, "application/manifest+json");
  }

  #[test]
  fn unknown_extensions() {
    assert!(mime_type_by_extension("unknown").is_none());
  }

  #[test]
  fn try_from_exts() {
    assert!(matches!(try_from_ext("png").unwrap().mime.subtype(), mime::PNG));
    assert!(matches!(try_from_ext("svg").unwrap().mime.subtype(), mime::SVG));
    assert!(matches!(try_from_ext("woff2").unwrap().mime.type_(), mime::FONT));
  }
}
