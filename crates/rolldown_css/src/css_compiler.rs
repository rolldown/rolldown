use lightningcss::{
  stylesheet::{ParserOptions, StyleSheet},
  traits::IntoOwned,
};

use crate::css_ast::CssAst;

pub struct CssCompiler;

impl CssCompiler {
  pub fn parse(source: &str, filename: String) -> anyhow::Result<CssAst> {
    let options = ParserOptions { filename: filename.clone(), ..Default::default() };
    let stylesheet =
      StyleSheet::parse(source, options.clone()).map_err(lightningcss::error::Error::into_owned)?;

    let stylesheet = StyleSheet::new(
      stylesheet.sources,
      stylesheet.rules.into_owned(),
      ParserOptions { filename, ..Default::default() },
    );
    Ok(CssAst { stylesheet })
  }
}

#[test]
fn basic_test() {
  use lightningcss::printer::PrinterOptions;
  let ast = CssCompiler::parse(".bar { color: green; }", "Noop".to_string()).unwrap();
  let res = ast.stylesheet.to_css(PrinterOptions::default()).unwrap();

  assert_eq!(res.code, ".bar {\n  color: green;\n}\n");
}
