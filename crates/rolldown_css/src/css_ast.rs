use lightningcss::stylesheet;

pub struct CssAst {
  pub stylesheet: stylesheet::StyleSheet<'static, 'static>,
}
