// We keep some standalone utilities here

/// Re-export of the shared scheduler, which lives in the `napi-async-runtime`
/// crate, under the in-repo `rolldown_utils::async_runtime::*` paths.
pub mod async_runtime {
  pub use napi_async_runtime::*;
  // Wasm: these names shadow the glob above with versions that run the thread-handoff hook
  // at every poll and blocking-closure start. Native keeps the plain re-export.
  // See internal-docs/wasi-shared-memory-grow/implementation.md
  #[cfg(target_family = "wasm")]
  pub use crate::thread_handoff::{
    HandoffHook, block_on, block_on_dyn, on_thread_handoff, set_thread_handoff_hook, spawn,
    spawn_blocking, spawn_detached, try_block_on_dyn, try_spawn, try_spawn_detached,
  };
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
#[cfg(target_family = "wasm")]
mod thread_handoff;
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
pub mod node_path;
pub use node_path::node_style_absolute;
pub mod pattern_filter;
pub mod replace_all_placeholder;
pub mod stabilize_id;
pub mod unique_arc;
pub mod url;
pub mod uuid;
