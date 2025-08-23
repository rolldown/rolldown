use napi::threadsafe_function::ThreadsafeFunction;

#[napi_derive::napi(object, object_to_js = false)]
pub struct JsWatcherOptions {
  pub watch: ThreadsafeFunction<String>,
  pub unwatch: ThreadsafeFunction<String>,
}
