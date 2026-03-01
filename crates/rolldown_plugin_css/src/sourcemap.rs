use rolldown_sourcemap::{SourceJoiner, SourceMap, SourceMapSource};

/// Convert a lightningcss/parcel_sourcemap `SourceMap` to an oxc `SourceMap` via JSON.
///
/// lightningcss uses `parcel_sourcemap::SourceMap` internally, while rolldown uses
/// `oxc_sourcemap::SourceMap`. The most reliable bridge is JSON serialization.
pub fn parcel_to_oxc(sm: &mut parcel_sourcemap::SourceMap) -> anyhow::Result<SourceMap> {
  let json =
    sm.to_json(None).map_err(|e| anyhow::anyhow!("Source map serialization error: {e}"))?;
  SourceMap::from_json_string(&json).map_err(|e| anyhow::anyhow!("Source map parse error: {e}"))
}

/// Parse CSS with lightningcss and produce both the output code and an oxc `SourceMap`.
///
/// The `filename` is recorded as the source in the source map.
pub fn parse_css_with_sourcemap(css: &str, filename: &str) -> anyhow::Result<(String, SourceMap)> {
  let mut sm = parcel_sourcemap::SourceMap::new("/");

  let stylesheet = lightningcss::stylesheet::StyleSheet::parse(
    css,
    lightningcss::stylesheet::ParserOptions { filename: filename.to_owned(), ..Default::default() },
  )
  .map_err(|e| anyhow::anyhow!("CSS parse error in {filename}: {e}"))?;

  let result = stylesheet
    .to_css(lightningcss::printer::PrinterOptions {
      source_map: Some(&mut sm),
      ..Default::default()
    })
    .map_err(|e| anyhow::anyhow!("CSS printer error: {e}"))?;

  let oxc_map = parcel_to_oxc(&mut sm)?;
  Ok((result.code, oxc_map))
}

/// Minify CSS with lightningcss and produce both the minified code and an oxc `SourceMap`.
pub fn minify_css_with_sourcemap(css: &str, filename: &str) -> anyhow::Result<(String, SourceMap)> {
  let mut sm = parcel_sourcemap::SourceMap::new("/");

  let stylesheet = lightningcss::stylesheet::StyleSheet::parse(
    css,
    lightningcss::stylesheet::ParserOptions { filename: filename.to_owned(), ..Default::default() },
  )
  .map_err(|e| anyhow::anyhow!("CSS parse error during minification: {e}"))?;

  let result = stylesheet
    .to_css(lightningcss::printer::PrinterOptions {
      minify: true,
      source_map: Some(&mut sm),
      ..Default::default()
    })
    .map_err(|e| anyhow::anyhow!("CSS printer error during minification: {e}"))?;

  let oxc_map = parcel_to_oxc(&mut sm)?;
  Ok((result.code, oxc_map))
}

/// Join multiple (css_code, source_map) pairs into a single combined CSS string and source map.
///
/// This mirrors what `SourceJoiner` does for JS: each source is placed on successive lines
/// with the source map line offsets adjusted accordingly.
pub fn join_css_sourcemaps(parts: Vec<(String, SourceMap)>) -> (String, Option<SourceMap>) {
  let mut joiner = SourceJoiner::default();

  for (code, map) in parts {
    joiner.append_source(SourceMapSource::new(code, map));
  }

  joiner.join()
}

/// Append a `/*# sourceMappingURL=... */` comment to CSS content.
pub fn append_sourcemap_url(css: &str, map_filename: &str) -> String {
  format!("{css}\n/*# sourceMappingURL={map_filename} */\n")
}
