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

use napi::{bindgen_prelude::create_custom_tokio_runtime, tokio};
#[cfg(not(target_family = "wasm"))]
use napi_derive::module_init;

#[cfg(not(target_family = "wasm"))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(target_family = "wasm"))]
#[module_init]
pub fn init() {
  let max_blocking_threads = {
    std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS")
      .ok()
      .and_then(|v| v.parse::<usize>().ok())
      .unwrap_or({
        #[cfg(target_os = "macos")]
        {
          num_cpus::get_physical()
        }
        #[cfg(not(target_os = "macos"))]
        {
          // default value in tokio implementation
          512
        }
      })
  };
  let rt = tokio::runtime::Builder::new_multi_thread()
    .disable_lifo_slot()
    .max_blocking_threads(max_blocking_threads)
    .enable_all()
    .build()
    .expect("Failed to create tokio runtime");
  create_custom_tokio_runtime(rt);
}

pub mod bundler;
pub mod options;
pub mod parallel_js_plugin_registry;
pub mod types;
pub mod utils;
pub use oxc_parser_napi;
pub use oxc_transform_napi;
pub mod watcher;
pub mod worker_manager;
