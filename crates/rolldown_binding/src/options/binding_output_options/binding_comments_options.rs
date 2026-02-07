#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingCommentsOptions {
  pub legal: Option<bool>,
  pub annotation: Option<bool>,
  pub other: Option<bool>,
}
