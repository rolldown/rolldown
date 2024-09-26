pub mod css_generator;

use std::convert::Infallible;

use arcstr::ArcStr;
use lightningcss::{
  stylesheet::{ParserOptions, StyleSheet},
  visit_types,
  visitor::{Visit, VisitTypes, Visitor},
};
use oxc::index::IndexVec;
use rolldown_common::CssView;

pub fn create_css_view(id: String, source: &ArcStr) -> anyhow::Result<CssView> {
  let options = ParserOptions { filename: id, ..Default::default() };
  let mut stylesheet =
    StyleSheet::parse(source, options).map_err(lightningcss::error::Error::into_owned)?;

  let mut scanner = CssAstScanner::default();

  stylesheet.visit(&mut scanner)?;

  Ok(CssView { source: source.clone(), import_records: IndexVec::default() })
}

#[derive(Default)]
pub struct CssAstScanner {
  pub dependencies: Vec<String>,
}

impl<'i> Visitor<'i> for CssAstScanner {
  type Error = Infallible;

  fn visit_types(&self) -> VisitTypes {
    visit_types!(URLS | RULES)
  }

  fn visit_rule(&mut self, rule: &mut lightningcss::rules::CssRule<'i>) -> Result<(), Self::Error> {
    match rule {
      lightningcss::rules::CssRule::Import(import_rule) => {
        self.dependencies.push(import_rule.url.to_string());
      }
      _ => {}
    }
    Ok(())
  }
}
