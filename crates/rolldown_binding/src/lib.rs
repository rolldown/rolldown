#[cfg(all(
  not(all(target_os = "linux", target_env = "musl", target_arch = "aarch64")),
  not(debug_assertions)
))]
#[global_allocator]
static ALLOC: mimalloc_rust::GlobalMiMalloc = mimalloc_rust::GlobalMiMalloc;

pub mod bundler;
pub mod options;
pub mod output;
pub mod utils;
scoped_tls::scoped_thread_local!(static NAPI_ENV: napi::Env);
