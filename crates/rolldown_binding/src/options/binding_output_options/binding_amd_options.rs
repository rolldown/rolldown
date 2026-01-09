use napi_derive::napi;

#[napi(object)]
#[derive(Debug, Default)]
pub struct BindingAmdOptions {
  /// An ID to use for AMD/UMD bundles
  pub id: Option<String>,
}
