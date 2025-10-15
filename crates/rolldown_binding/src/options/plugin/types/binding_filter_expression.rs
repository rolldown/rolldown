use napi::{
  bindgen_prelude::{Either4, FromNapiValue},
  sys,
};
use napi_derive::napi;
use rolldown_utils::{
  filter_expression::Token, js_regex::HybridRegex, pattern_filter::StringOrRegex,
};

use crate::types::js_regex::JsRegExp;

#[derive(Debug, Clone)]
pub enum BindingFilterTokenPayloadInner {
  StringOrRegex(StringOrRegex),
  Boolean(bool),
  Number(u32),
}

impl BindingFilterTokenPayloadInner {
  pub fn expect_string(self) -> String {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => inner.expect_string(),
      BindingFilterTokenPayloadInner::Number(_) | BindingFilterTokenPayloadInner::Boolean(_) => {
        unreachable!()
      }
    }
  }

  pub fn expect_string_or_regex(self) -> StringOrRegex {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => inner,
      BindingFilterTokenPayloadInner::Number(_) | BindingFilterTokenPayloadInner::Boolean(_) => {
        unreachable!()
      }
    }
  }

  pub fn expect_number(self) -> u32 {
    match self {
      BindingFilterTokenPayloadInner::Number(v) => v,
      BindingFilterTokenPayloadInner::StringOrRegex(_)
      | BindingFilterTokenPayloadInner::Boolean(_) => unreachable!(),
    }
  }

  pub fn expect_regex(self) -> HybridRegex {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => inner.expect_regex(),
      BindingFilterTokenPayloadInner::Number(_) | BindingFilterTokenPayloadInner::Boolean(_) => {
        unreachable!()
      }
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
      let value = Either4::<String, JsRegExp, u32, bool>::from_napi_value(env, napi_val)?;
      let value = match value {
        Either4::A(inner) => {
          BindingFilterTokenPayloadInner::StringOrRegex(StringOrRegex::String(inner))
        }
        Either4::B(inner) => {
          let reg = HybridRegex::with_flags(&inner.source, &inner.flags)?;
          BindingFilterTokenPayloadInner::StringOrRegex(StringOrRegex::Regex(reg))
        }
        Either4::C(inner) => BindingFilterTokenPayloadInner::Number(inner),
        Either4::D(inner) => BindingFilterTokenPayloadInner::Boolean(inner),
      };
      Ok(Self(value))
    }
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Clone)]
pub struct BindingFilterToken {
  pub kind: FilterTokenKind,
  #[napi(ts_type = "BindingStringOrRegex | number | boolean")]
  pub payload: Option<BindingFilterTokenPayload>,
}

#[napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
  QueryKey,
  QueryValue,
}

pub fn normalized_tokens(tokens: Vec<BindingFilterToken>) -> Vec<Token> {
  let mut ret: Vec<Token> = Vec::with_capacity(tokens.len());
  let mut iter = tokens.into_iter().peekable();
  while let Some(value) = iter.peek_mut() {
    match value.kind {
      FilterTokenKind::Id => {
        ret.push(Token::from(
          value
            .payload
            .take()
            .expect("`Id` should have payload")
            .into_inner()
            .expect_string_or_regex(),
        ));
        ret.push(Token::Id);
      }
      FilterTokenKind::Code => {
        ret.push(Token::from(
          value
            .payload
            .take()
            .expect("`Code` should have payload")
            .into_inner()
            .expect_string_or_regex(),
        ));
        ret.push(Token::Code);
      }
      FilterTokenKind::ModuleType => {
        ret.push(Token::String(
          value
            .payload
            .take()
            .expect("`ModuleType` should have payload")
            .into_inner()
            .expect_string(),
        ));
        ret.push(Token::ModuleType);
      }
      FilterTokenKind::And => {
        ret.push(Token::And(
          value.payload.take().expect("And should have payload").into_inner().expect_number(),
        ));
      }
      FilterTokenKind::Or => {
        ret.push(Token::Or(
          value.payload.take().expect("`Or` should have payload").into_inner().expect_number(),
        ));
      }
      FilterTokenKind::Not => {
        ret.push(Token::Not);
      }
      FilterTokenKind::Include => ret.push(Token::Include),
      FilterTokenKind::Exclude => ret.push(Token::Exclude),
      FilterTokenKind::CleanUrl => ret.push(Token::CleanUrl),
      FilterTokenKind::QueryKey => {
        let query_key = value
          .payload
          .take()
          .expect("`QueryKey` should have payload")
          .into_inner()
          .expect_string();
        iter.next();
        let Some(next_token) = iter.peek_mut() else {
          unreachable!("`QueryKey` should be followed by one Token");
        };
        if next_token.kind != FilterTokenKind::QueryValue {
          unreachable!("`QueryKey` should be followed by `QueryValue`");
        }
        let query_value =
          match next_token.payload.take().expect("`QueryValue` should have payload").into_inner() {
            BindingFilterTokenPayloadInner::StringOrRegex(string_or_regex) => match string_or_regex
            {
              StringOrRegex::String(str) => Token::String(str),
              StringOrRegex::Regex(regexp) => Token::Regex(regexp),
            },
            BindingFilterTokenPayloadInner::Boolean(v) => Token::Boolean(v),
            BindingFilterTokenPayloadInner::Number(_) => todo!(),
          };
        ret.push(query_value);
        ret.push(Token::String(query_key));
        ret.push(Token::Query);
        iter.next();
        continue;
      }
      FilterTokenKind::QueryValue => {
        unreachable!("`QueryValue` is not expected");
      }
    }
    iter.next();
  }
  ret
}
