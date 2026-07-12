// We keep some standalone utilities here

/// Maximum number of physical workers a Rolldown-owned async runtime may create.
pub const MAX_ASYNC_RUNTIME_WORKER_THREADS: usize = 256;

/// Platform-realizable worker ceiling after applying Rolldown's production cap.
#[cfg(not(target_family = "wasm"))]
pub fn max_async_runtime_worker_threads() -> usize {
  MAX_ASYNC_RUNTIME_WORKER_THREADS.min(::rayon::max_num_threads())
}

/// WebAssembly builds use the current-thread executor.
#[cfg(target_family = "wasm")]
pub const fn max_async_runtime_worker_threads() -> usize {
  1
}

/// The shared scheduler now lives in the `napi-async-runtime` crate
/// (napi-rs workspace); this module re-exports its entire API so in-repo
/// paths like `rolldown_utils::async_runtime::spawn` keep working unchanged.
#[cfg(feature = "async-runtime")]
pub mod async_runtime {
  pub use napi_async_runtime::*;
}
pub mod base64;
mod bitset;
pub mod dashmap;
pub mod dataurl;
pub mod debug;
pub mod ecmascript;
pub mod futures;
pub mod index_bitset;
pub mod indexmap;
pub mod light_guess;
pub mod mime;
pub mod percent_encoding;
pub mod rayon;
pub mod rustc_hash;
pub mod sanitize_filename;
pub mod time;
pub mod xxhash;
pub use bitset::BitSet;
pub use index_bitset::IndexBitSet;
pub mod commondir;
pub mod concat_string;
pub mod filter_expression;
pub mod hash_placeholder;
pub mod index_vec_ext;
pub mod js_regex;
pub mod make_unique_name;
pub mod pattern_filter;
pub mod replace_all_placeholder;
pub mod stabilize_id;
pub mod unique_arc;
pub mod url;
pub mod uuid;
