// Port from https://github.com/7086cmd/mime_more/blob/main/src/light_guess.rs
use mime::Mime;
use phf::{phf_map, Map};
use std::str::FromStr;

pub static MIME_TYPES: Map<&'static str, &'static str> = phf_map! {
    // Text
    "txt" => "text/plain",
    "css" => "text/css",
    "htm" => "text/html",
    "html" => "text/html",
    "js" => "text/javascript",
    "mjs" => "text/javascript",
    "jsx" => "text/javascript",
    "json" => "application/json",
    "yaml" => "text/x-yaml",
    "yml" => "text/x-yaml",
    "toml" => "text/x-toml",
    "markdown" => "text/markdown",
    "md" => "text/markdown",
    "xml" => "text/xml",
    "csv" => "text/csv",
    "tsv" => "text/tab-separated-values",
    // Images
    "bmp" => "image/bmp",
    "avif" => "image/avif",
    "gif" => "image/gif",
    "ico" => "image/x-icon",
    "icns" => "image/x-icns",
    "jpg" => "image/jpeg",
    "jpeg" => "image/jpeg",
    "png" => "image/png",
    "svg" => "image/svg+xml",
    "webp" => "image/webp",
    // Fonts
    "otf" => "font/otf",
    "ttf" => "font/ttf",
    "ttc" => "font/collection",
    "woff" => "font/woff",
    "woff2" => "font/woff2",
    "eot" => "application/vnd.ms-fontobject",
    "sfnt" => "font/sfnt",
    // Audios
    "aac" => "audio/aac",
    "midi" => "audio/midi",
    "mid" => "audio/midi",
    "mp3" => "audio/mpeg",
    "ogg" => "audio/ogg",
    "oga" => "audio/ogg",
    "wav" => "audio/wav",
    "weba" => "audio/webm",
    "flac" => "audio/flac",
    "m3u8" => "audio/x-mpegurl",
    "m4a" => "audio/m4a",
    // Videos
    "avi" => "video/x-msvideo",
    "mpeg" => "video/mpeg",
    "ogv" => "video/ogg",
    "ivf" => "video/x-ivf",
    "webm" => "video/webm",
    "mp4" => "video/mp4",
    "flv" => "video/x-flv",
    "ts" => "audio/vnd.dlna.mpeg-tts", // Though I write TypeScript, this is not TypeScript
    "mov" => "video/quicktime",
    "wmv" => "video/x-ms-wmv",
    // Other
    "pdf" => "application/pdf",
    "wasm" => "application/wasm",
    "webmanifest" => "application/manifest+json",
};

/// Adapted from:
/// - https://github.com/rolldown/rolldown/pull/1406/files#diff-4b612e077c82ae0e05e50eb0d419e02c05a04b83c6ac5440c0d0c9d0c38af942
/// - https://github.com/evanw/esbuild/blob/fc37c2fa9de2ad77476a6d4a8f1516196b90187e/internal/helpers/mime.go#L5
///
/// Thanks to @ikkz and @evanw for the inspiration.
pub fn mime_type_by_extension(ext: &str) -> Option<&'static str> {
  MIME_TYPES.get(ext).copied()
}

pub fn try_from_ext(ext: &str) -> anyhow::Result<Mime> {
  mime_type_by_extension(ext)
    .ok_or_else(|| anyhow::Error::msg(format!("No mime type found for extension: {ext}")))
    .and_then(|mime| Ok(Mime::from_str(mime)?))
}

pub fn try_from_path(path: &std::path::Path) -> anyhow::Result<Mime> {
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
    assert_eq!(mime_type_by_extension("txt").unwrap(), "text/plain");
    assert_eq!(mime_type_by_extension("css").unwrap(), "text/css");
    assert_eq!(mime_type_by_extension("html").unwrap(), "text/html");
    assert_eq!(mime_type_by_extension("json").unwrap(), "application/json");
    assert_eq!(mime_type_by_extension("png").unwrap(), "image/png");
    assert_eq!(mime_type_by_extension("svg").unwrap(), "image/svg+xml");
    assert_eq!(mime_type_by_extension("woff2").unwrap(), "font/woff2");
    assert_eq!(mime_type_by_extension("aac").unwrap(), "audio/aac");
    assert_eq!(mime_type_by_extension("avi").unwrap(), "video/x-msvideo");
    assert_eq!(mime_type_by_extension("pdf").unwrap(), "application/pdf");
    assert_eq!(mime_type_by_extension("wasm").unwrap(), "application/wasm");
    assert_eq!(mime_type_by_extension("webmanifest").unwrap(), "application/manifest+json");
  }

  #[test]
  fn unknown_extensions() {
    assert!(mime_type_by_extension("unknown").is_none());
  }

  #[test]
  fn try_from_exts() {
    assert!(matches!(try_from_ext("png").unwrap().subtype(), mime::PNG));
    assert!(matches!(try_from_ext("svg").unwrap().subtype(), mime::SVG));
    assert!(matches!(try_from_ext("woff2").unwrap().type_(), mime::FONT));
  }
}
