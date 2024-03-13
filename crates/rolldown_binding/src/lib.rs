#[cfg(not(target_family = "wasm"))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod bundler;
pub mod options;
pub mod types;
pub mod utils;
scoped_tls::scoped_thread_local!(static NAPI_ENV: napi::Env);
