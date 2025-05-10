use itertools::Itertools;
use napi::{
  bindgen_prelude::{Either3, FromNapiValue},
  sys,
};
use napi_derive::napi;
use rolldown_utils::{
  filter_expression::Token, js_regex::HybridRegex, pattern_filter::StringOrRegex,
};

use super::binding_js_or_regex::JsRegExp;

#[derive(Debug, Clone)]
pub enum BindingFilterTokenPayloadInner {
  StringOrRegex(StringOrRegex),
  Number(u32),
}
impl BindingFilterTokenPayloadInner {
  pub fn expect_string(self) -> String {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => inner.expect_string(),
      BindingFilterTokenPayloadInner::Number(_) => unreachable!(),
    }
  }

  pub fn expect_string_or_regex(self) -> StringOrRegex {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => inner,
      BindingFilterTokenPayloadInner::Number(_) => unreachable!(),
    }
  }

  pub fn expect_number(self) -> u32 {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(_) => unreachable!(),
      BindingFilterTokenPayloadInner::Number(v) => v,
    }
  }

  pub fn expect_regex(self) -> HybridRegex {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => inner.expect_regex(),
      BindingFilterTokenPayloadInner::Number(_) => unreachable!(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct BindingFilterTokenPayload(BindingFilterTokenPayloadInner);

impl BindingFilterTokenPayload {
  pub fn into_inner(self) -> BindingFilterTokenPayloadInner {
    self.0
  }
}

impl FromNapiValue for BindingFilterTokenPayload {
  unsafe fn from_napi_value(env: sys::napi_env, napi_val: sys::napi_value) -> napi::Result<Self> {
    unsafe {
      let value = Either3::<String, JsRegExp, u32>::from_napi_value(env, napi_val)?;
      let value = match value {
        Either3::A(inner) => {
          BindingFilterTokenPayloadInner::StringOrRegex(StringOrRegex::String(inner))
        }
        Either3::B(inner) => {
          let reg = HybridRegex::with_flags(&inner.source, &inner.flags)?;
          BindingFilterTokenPayloadInner::StringOrRegex(StringOrRegex::Regex(reg))
        }
        Either3::C(inner) => BindingFilterTokenPayloadInner::Number(inner),
      };
      Ok(Self(value))
    }
  }
}

#[napi(object, object_to_js = false)]
#[derive(Debug, Clone)]
pub struct BindingFilterToken {
  pub kind: FilterTokenKind,
  #[napi(ts_type = "BindingStringOrRegex | number")]
  pub payload: Option<BindingFilterTokenPayload>,
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
  CleanUrl,
}

impl From<BindingFilterToken> for Token {
  fn from(value: BindingFilterToken) -> Self {
    match value.kind {
      FilterTokenKind::Id => Token::Id(
        value.payload.expect("Id should have payload").into_inner().expect_string_or_regex(),
      ),
      FilterTokenKind::Code => Token::Code(
        value.payload.expect("Code should have payload").into_inner().expect_string_or_regex(),
      ),
      FilterTokenKind::ModuleType => Token::ModuleType(
        value.payload.expect("ModuleType should have payload").into_inner().expect_string(),
      ),
      FilterTokenKind::And => {
        Token::And(value.payload.expect("And should have payload").into_inner().expect_number())
      }
      FilterTokenKind::Or => {
        Token::Or(value.payload.expect("`Or` should have payload").into_inner().expect_number())
      }
      FilterTokenKind::Not => Token::Not,
      FilterTokenKind::Include => Token::Include,
      FilterTokenKind::Exclude => Token::Exclude,
      FilterTokenKind::CleanUrl => Token::CleanUrl,
    }
  }
}

pub fn normalized_tokens(tokens: Vec<BindingFilterToken>) -> Vec<Token> {
  tokens.into_iter().map(Token::from).collect_vec()
}
