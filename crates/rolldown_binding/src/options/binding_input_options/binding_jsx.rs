// TODO: support `preserve-react` mode
#[napi_derive::napi]
pub enum BindingJsx {
  Disable,
  Preserve,
  React,
  ReactJsx,
}
