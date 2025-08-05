// Allow type complexity rule, because NAPI-RS requires the direct types to generate the TypeScript definitions.
#![allow(clippy::type_complexity)]
// Due to the bound of NAPI-RS, we need to use `String` though we only need `&str`.
#![allow(clippy::needless_pass_by_value)]
// Most of transmute are just change the lifetime `'a` to `'static`., the annotation, e.g.
//
// BindingTransformPluginContext::new(unsafe {
//   std::mem::transmute::<
//     &rolldown_plugin::TransformPluginContext<'_>,
//     &rolldown_plugin::TransformPluginContext<'_>,
//   >(ctx)
// }),
// Looks redundant
#![allow(clippy::missing_transmute_annotations)]

use napi_derive::napi;

#[cfg(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos")
))]
#[global_allocator]
static ALLOC: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

pub mod binding_bundler_impl;
pub mod options;
pub mod parallel_js_plugin_registry;
pub mod types;
pub mod utils;
pub use oxc_parser_napi;
pub use oxc_resolver_napi;
pub use oxc_transform_napi;
pub mod binding_bundler;
mod generated;
pub mod watcher;
pub mod worker_manager;

#[cfg(not(target_family = "wasm"))]
#[napi_derive::module_init]
pub fn init() {
  use napi::{bindgen_prelude::create_custom_tokio_runtime, tokio};
  let max_blocking_threads = std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS")
    .ok()
    .and_then(|v| v.parse::<usize>().ok())
    .unwrap_or(512); // default value in tokio implementation
  let rt = tokio::runtime::Builder::new_multi_thread()
    .max_blocking_threads(max_blocking_threads)
    .enable_all()
    .build()
    .expect("Failed to create tokio runtime");
  create_custom_tokio_runtime(rt);
}

#[napi]
/// Shutdown the tokio runtime manually.
///
/// This is required for the wasm target with `tokio_unstable` cfg.
/// In the wasm runtime, the `park` threads will hang there until the tokio::Runtime is shutdown.
pub fn shutdown_async_runtime() {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  napi::bindgen_prelude::shutdown_async_runtime();
}

#[napi]
/// Start the async runtime manually.
///
/// This is required when the async runtime is shutdown manually.
/// Usually it's used in test.
pub fn start_async_runtime() {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  napi::bindgen_prelude::start_async_runtime();
}
