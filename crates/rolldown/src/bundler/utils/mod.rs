pub mod ast_scope;
pub mod ast_symbol;
pub mod bitset;
pub mod load_source;
pub mod renamer;
pub mod render_chunks;
pub mod resolve_id;
pub mod symbols;
pub mod transform_source;

pub(crate) fn is_in_rust_test_mode() -> bool {
  static TEST_MODE: once_cell::sync::Lazy<bool> =
    once_cell::sync::Lazy::new(|| std::env::var("ROLLDOWN_TEST").is_ok());
  *TEST_MODE
}
