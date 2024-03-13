// #[cfg(not(target_family = "wasm"))]
// #[global_allocator]
// static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(target_os = "windows")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod bundler;
pub mod options;
pub mod types;
pub mod utils;
scoped_tls::scoped_thread_local!(static NAPI_ENV: napi::Env);
