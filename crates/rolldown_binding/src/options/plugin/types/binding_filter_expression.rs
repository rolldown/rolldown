use derive_more::Debug;

use itertools::Itertools;
use napi_derive::napi;
use rolldown_utils::filter_expression::Token;

use super::binding_js_or_regex::BindingStringOrRegex;

#[napi(object, object_to_js = false)]
#[derive(Debug, Clone)]
pub struct BindingFilterToken {
  pub kind: FilterTokenKind,
  pub value: Option<BindingStringOrRegex>,
}

#[napi(string_enum)]
#[derive(Debug, Clone, Copy)]
pub enum FilterTokenKind {
  Id,
  Code,
  ModuleType,
  And,
  Or,
  Not,
  Include,
  Exclude,
}

impl From<BindingFilterToken> for Token {
  fn from(value: BindingFilterToken) -> Self {
    match value.kind {
      FilterTokenKind::Id => Token::Id(value.value.expect("Id should have payload").inner()),
      FilterTokenKind::Code => Token::Code(value.value.expect("Code should have payload").inner()),
      FilterTokenKind::ModuleType => Token::ModuleType(
        value.value.expect("ModuleType should have payload").inner().expect_string(),
      ),
      FilterTokenKind::And => Token::And,
      FilterTokenKind::Or => Token::Or,
      FilterTokenKind::Not => Token::Not,
      FilterTokenKind::Include => Token::Include,
      FilterTokenKind::Exclude => Token::Exclude,
    }
  }
}
pub fn normalized_tokens(tokens: Vec<BindingFilterToken>) -> Vec<Token> {
  tokens.into_iter().map(Token::from).collect_vec()
}
