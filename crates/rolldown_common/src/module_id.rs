index_vec::define_index_type! {
    pub struct ModuleId = u32;
}

impl Default for ModuleId {
  fn default() -> Self {
    Self::from_raw(u32::MAX)
  }
}

impl ModuleId {
  pub fn is_valid(&self) -> bool {
    self.raw() < u32::MAX
  }
}
