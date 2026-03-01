use lightningcss::{
  printer::PrinterOptions,
  stylesheet::{ParserOptions, StyleSheet},
};

/// Minify CSS using lightningcss.
///
/// Parses the input CSS and re-serializes it with minification enabled,
/// producing compact output with whitespace removal and shorthand optimization.
pub fn minify_css(css: &str) -> anyhow::Result<String> {
  let stylesheet = StyleSheet::parse(css, ParserOptions::default())
    .map_err(|e| anyhow::anyhow!("CSS parse error during minification: {e}"))?;

  let result = stylesheet
    .to_css(PrinterOptions { minify: true, ..Default::default() })
    .map_err(|e| anyhow::anyhow!("CSS printer error during minification: {e}"))?;

  Ok(result.code)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_minify_css() {
    let input = ".foo {\n  color: red;\n  margin: 0px;\n}\n";
    let output = minify_css(input).unwrap();
    // lightningcss minification should remove whitespace
    assert!(!output.contains('\n') || output.len() < input.len());
    assert!(output.contains(".foo"));
    assert!(output.contains("red"));
  }

  #[test]
  fn test_minify_empty() {
    let output = minify_css("").unwrap();
    assert!(output.is_empty() || output.trim().is_empty());
  }
}
