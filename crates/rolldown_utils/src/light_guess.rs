// Port from https://github.com/7086cmd/mime_more/blob/main/src/light_guess.rs
use phf::{phf_map, Map};

use crate::mime::MimeExt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawMimeExt {
  pub mime_str: &'static str,
  pub is_utf8_encoded: bool,
}

impl RawMimeExt {
  const fn new(mime_str: &'static str, is_utf8_encoded: bool) -> RawMimeExt {
    RawMimeExt { mime_str, is_utf8_encoded }
  }
}

pub static MIME_TYPES: Map<&'static str, RawMimeExt> = phf_map! {
    // Text
    "css" => RawMimeExt::new("text/css", true),
    "htm" => RawMimeExt::new("text/html", true),
    "html" => RawMimeExt::new("text/html", true),
    "js" => RawMimeExt::new("text/javascript", true),
    "json" => RawMimeExt::new("application/json", true),
    "markdown" => RawMimeExt::new("text/markdown", true),
    "md" => RawMimeExt::new("text/markdown", true),
    "mjs" => RawMimeExt::new("text/javascript", true),

    // Images
    "avif" => RawMimeExt::new("image/avif", false),
    "gif" => RawMimeExt::new("image/gif", false),
    "jpg" => RawMimeExt::new("image/jpeg", false),
    "jpeg" => RawMimeExt::new("image/jpeg", false),
    "png" => RawMimeExt::new("image/png", false),
    "svg" => RawMimeExt::new("image/svg+xml",false),
    "webp" => RawMimeExt::new("image/webp", false),

    // Fonts
    "eot" => RawMimeExt::new("application/vnd.ms-fontobject", false),
    "otf" => RawMimeExt::new("font/otf", false),
    "sfnt" => RawMimeExt::new("font/sfnt", false),
    "ttf" => RawMimeExt::new("font/ttf", false),
    "woff" => RawMimeExt::new("font/woff", false),
    "woff2" => RawMimeExt::new("font/woff2", false),
    // Others
    "pdf" => RawMimeExt::new("application/pdf", false),
    "wasm" => RawMimeExt::new("application/wasm", false),
    "webmanifest" => RawMimeExt::new("application/manifest+json", false),
};

/// Adapted from:
/// - https://github.com/rolldown/rolldown/pull/1406/files#diff-4b612e077c82ae0e05e50eb0d419e02c05a04b83c6ac5440c0d0c9d0c38af942
/// - https://github.com/evanw/esbuild/blob/fc37c2fa9de2ad77476a6d4a8f1516196b90187e/internal/helpers/mime.go#L5
///
/// Thanks to @ikkz and @evanw for the inspiration.
pub fn mime_type_by_extension(ext: &str) -> Option<RawMimeExt> {
  MIME_TYPES.get(ext).copied()
}

pub fn try_from_ext(ext: &str) -> anyhow::Result<MimeExt> {
  mime_type_by_extension(ext)
    .ok_or_else(|| anyhow::Error::msg(format!("No mime type found for extension: {ext}")))
    .and_then(MimeExt::try_from)
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
    assert_eq!(mime_type_by_extension("css").unwrap().mime_str, "text/css");
    assert_eq!(mime_type_by_extension("html").unwrap().mime_str, "text/html");
    assert_eq!(mime_type_by_extension("json").unwrap().mime_str, "application/json");
    assert_eq!(mime_type_by_extension("png").unwrap().mime_str, "image/png");
    assert_eq!(mime_type_by_extension("svg").unwrap().mime_str, "image/svg+xml");
    assert_eq!(mime_type_by_extension("woff2").unwrap().mime_str, "font/woff2");
    assert_eq!(mime_type_by_extension("pdf").unwrap().mime_str, "application/pdf");
    assert_eq!(mime_type_by_extension("wasm").unwrap().mime_str, "application/wasm");
    assert_eq!(
      mime_type_by_extension("webmanifest").unwrap().mime_str,
      "application/manifest+json"
    );
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
