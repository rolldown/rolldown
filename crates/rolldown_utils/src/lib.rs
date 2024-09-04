// We keep some standalone utilities here

pub mod base64;
mod bitset;
pub mod dataurl;
pub mod debug;
pub mod ecma_script;
pub mod futures;
pub mod global_reference;
pub mod indexmap;
pub mod light_guess;
pub mod mime;
pub mod path_buf_ext;
pub mod path_ext;
pub mod percent_encoding;
pub mod rayon;
pub mod rustc_hash;
pub mod sanitize_file_name;
pub mod xxhash;
pub use bitset::BitSet;
pub mod js_regex;
pub mod pattern_filter;
pub mod unique_arc;
