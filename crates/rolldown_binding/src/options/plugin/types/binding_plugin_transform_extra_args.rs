// This struct is used to both pass to JS and receive from JS:
// - Pass to JS: Created in js_plugin.rs:210 and passed to JS plugin transform hooks
// - Receive from JS: Received in binding_callable_builtin_plugin.rs:119 when calling builtin plugins from JS
#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct BindingTransformHookExtraArgs {
  pub module_type: String,
}
