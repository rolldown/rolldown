// https://github.com/evanw/esbuild/blob/fc37c2fa9de2ad77476a6d4a8f1516196b90187e/internal/helpers/mime.go#L5
pub fn mime_type_by_extension(ext: &str) -> Option<&'static str> {
  let mime = match ext.to_lowercase().as_str() {
    // Text
    ".css" => "text/css; charset=utf-8",
    ".htm" | ".html" => "text/html; charset=utf-8",
    ".js" | ".mjs" => "text/javascript; charset=utf-8",
    ".json" => "application/json; charset=utf-8",
    ".markdown" | ".md" => "text/markdown; charset=utf-8",
    ".xhtml" => "application/xhtml+xml; charset=utf-8",
    ".xml" => "text/xml; charset=utf-8",
    // Images
    ".avif" => "image/avif",
    ".gif" => "image/gif",
    ".jpeg" | ".jpg" => "image/jpeg",
    ".png" => "image/png",
    ".svg" => "image/svg+xml",
    ".webp" => "image/webp",
    // Fonts
    ".eot" => "application/vnd.ms-fontobject",
    ".otf" => "font/otf",
    ".sfnt" => "font/sfnt",
    ".ttf" => "font/ttf",
    ".woff" => "font/woff",
    ".woff2" => "font/woff2",
    // Other
    ".pdf" => "application/pdf",
    ".wasm" => "application/wasm",
    ".webmanifest" => "application/manifest+json",
    _ => "",
  };
  if mime.is_empty() {
    None
  } else {
    Some(mime)
  }
}

pub fn guess_mime_type(ext: &str, _content: &[u8]) -> String {
  mime_type_by_extension(ext)
    .unwrap_or(
      // TODO: Use the same algorithm with esbuild to determine the MIME by content, see https://pkg.go.dev/net/http#DetectContentType
      "application/octet-stream",
    )
    .replacen("; ", ";", 1)
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn test_mime_type_by_extension() {
    assert_eq!(guess_mime_type(".css", ".main {}".as_bytes()), "text/css;charset=utf-8");
  }
}
