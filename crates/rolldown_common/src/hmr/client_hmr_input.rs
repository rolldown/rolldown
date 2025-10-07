use rustc_hash::FxHashSet;

#[derive(Debug)]
pub struct ClientHmrInput {
  pub client_id: String,
  pub executed_modules: FxHashSet<String>,
}
