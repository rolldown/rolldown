oxc_index::define_index_type! {
    pub struct ModuleIdx = u32;
}

// Preserved module idx used for representing a module that is not in the module graph.
// e.g.
// create a module idx for `ImportRecord` for `require` ExpressionIdentifier
pub const DUMMY_MODULE_IDX: ModuleIdx = ModuleIdx::from_usize_unchecked(u32::MAX as usize);

impl ModuleIdx {
  #[inline]
  pub fn is_dummy(&self) -> bool {
    *self == DUMMY_MODULE_IDX
  }
}
