// We keep some standalone utilities here

pub mod base64;
mod bitset;
pub mod dashmap;
pub mod dataurl;
pub mod debug;
pub mod ecmascript;
pub mod futures;
pub mod global_reference;
pub mod indexmap;
pub mod light_guess;
pub mod mime;
pub mod percent_encoding;
pub mod rayon;
pub mod rustc_hash;
pub mod sanitize_filename;
pub mod xxhash;
pub use bitset::BitSet;
pub mod clean_url;
pub mod concat_string;
pub mod hash_placeholder;
pub mod index_vec_ext;
pub mod js_regex;
pub mod pattern_filter;
pub mod replace_all_placeholder;
pub mod unique_arc;
