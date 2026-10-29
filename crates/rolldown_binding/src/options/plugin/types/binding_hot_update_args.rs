use napi_derive::napi;

#[napi(object)]
#[derive(Debug)]
pub struct BindingHotUpdateArgs {
  #[napi(ts_type = "'create' | 'update' | 'delete'")]
  pub kind: String,
  /// Normalized absolute path of the changed file.
  pub file: String,
  /// The affected module ids as currently computed (raw module ids).
  pub modules: Vec<String>,
}
