use rustc_hash::FxHashSet;

#[derive(Default)]
pub struct ClientSession {
  pub executed_modules: FxHashSet<String>,
}
