use rustc_hash::FxHashSet;

#[derive(Debug)]
pub struct ClientHmrInput<'a> {
  pub client_id: &'a str,
  pub executed_modules: &'a FxHashSet<String>,
}
