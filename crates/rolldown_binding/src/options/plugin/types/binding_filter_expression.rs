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
  pub fn try_into_string(self) -> anyhow::Result<String> {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => inner.try_into_string(),
      other => anyhow::bail!("expected a string payload, but got {other:?}"),
    }
  }

  pub fn try_into_string_or_regex(self) -> anyhow::Result<StringOrRegex> {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => Ok(inner),
      other => anyhow::bail!("expected a string or regex payload, but got {other:?}"),
    }
  }

  pub fn try_into_number(self) -> anyhow::Result<u32> {
    match self {
      BindingFilterTokenPayloadInner::Number(v) => Ok(v),
      other => anyhow::bail!("expected a number payload, but got {other:?}"),
    }
  }

  pub fn try_into_regex(self) -> anyhow::Result<HybridRegex> {
    match self {
      BindingFilterTokenPayloadInner::StringOrRegex(inner) => inner.try_into_regex(),
      other => anyhow::bail!("expected a regex payload, but got {other:?}"),
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
  ImporterId,
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

pub fn normalized_tokens(tokens: Vec<BindingFilterToken>) -> anyhow::Result<Vec<Token>> {
  fn take_payload(
    token: &mut BindingFilterToken,
  ) -> anyhow::Result<BindingFilterTokenPayloadInner> {
    token
      .payload
      .take()
      .ok_or_else(|| anyhow::anyhow!("`{:?}` token should have a payload", token.kind))
      .map(BindingFilterTokenPayload::into_inner)
  }

  let mut ret: Vec<Token> = Vec::with_capacity(tokens.len());
  let mut iter = tokens.into_iter().peekable();
  while let Some(value) = iter.peek_mut() {
    match value.kind {
      FilterTokenKind::Id => {
        ret.push(Token::from(take_payload(value)?.try_into_string_or_regex()?));
        ret.push(Token::Id);
      }
      FilterTokenKind::ImporterId => {
        ret.push(Token::from(take_payload(value)?.try_into_string_or_regex()?));
        ret.push(Token::ImporterId);
      }
      FilterTokenKind::Code => {
        ret.push(Token::from(take_payload(value)?.try_into_string_or_regex()?));
        ret.push(Token::Code);
      }
      FilterTokenKind::ModuleType => {
        ret.push(Token::String(take_payload(value)?.try_into_string()?));
        ret.push(Token::ModuleType);
      }
      FilterTokenKind::And => {
        ret.push(Token::And(take_payload(value)?.try_into_number()?));
      }
      FilterTokenKind::Or => {
        ret.push(Token::Or(take_payload(value)?.try_into_number()?));
      }
      FilterTokenKind::Not => {
        ret.push(Token::Not);
      }
      FilterTokenKind::Include => ret.push(Token::Include),
      FilterTokenKind::Exclude => ret.push(Token::Exclude),
      FilterTokenKind::CleanUrl => ret.push(Token::CleanUrl),
      FilterTokenKind::QueryKey => {
        let query_key = take_payload(value)?.try_into_string()?;
        iter.next();
        let next_token = iter.peek_mut().ok_or_else(|| {
          anyhow::anyhow!("`QueryKey` should be followed by a `QueryValue` token")
        })?;
        if next_token.kind != FilterTokenKind::QueryValue {
          anyhow::bail!(
            "`QueryKey` should be followed by `QueryValue`, but got `{:?}`",
            next_token.kind
          );
        }
        let query_value = match take_payload(next_token)? {
          BindingFilterTokenPayloadInner::StringOrRegex(string_or_regex) => match string_or_regex {
            StringOrRegex::String(str) => Token::String(str),
            StringOrRegex::Regex(regexp) => Token::Regex(regexp),
          },
          BindingFilterTokenPayloadInner::Boolean(v) => Token::Boolean(v),
          BindingFilterTokenPayloadInner::Number(_) => {
            anyhow::bail!("number values are not supported for query filter values");
          }
        };
        ret.push(query_value);
        ret.push(Token::String(query_key));
        ret.push(Token::Query);
        iter.next();
        continue;
      }
      FilterTokenKind::QueryValue => {
        anyhow::bail!("`QueryValue` token should not appear without a preceding `QueryKey`");
      }
    }
    iter.next();
  }
  Ok(ret)
}
