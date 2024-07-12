use arcstr::ArcStr;
use lightningcss::stylesheet::StyleSheet;

pub fn parse_to_css_ast(source: &ArcStr) -> StyleSheet<'static, 'static> {
  let ast = StyleSheet::parse(&source, Default::default()).unwrap();
  unsafe { std::mem::transmute(ast) }
}
