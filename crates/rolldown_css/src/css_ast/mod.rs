use arcstr::ArcStr;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};

pub fn parse_to_css_ast(source: &ArcStr) -> StyleSheet<'static, 'static> {
  let ast = StyleSheet::parse(source, ParserOptions::default()).unwrap();
  unsafe { std::mem::transmute(ast) } // It should be converted to 'static lifetime.
}
