use rustc_hash::FxHashSet;

#[derive(Default)]
pub struct ClientSession {
  pub registered_modules: FxHashSet<String>,
}
