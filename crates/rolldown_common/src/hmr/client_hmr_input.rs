use rustc_hash::FxHashSet;

#[derive(Debug)]
pub struct ClientHmrInput<'a> {
  pub client_id: &'a str,
  pub executed_modules: &'a FxHashSet<String>,
}

impl ClientHmrInput<'_> {
  /// Check if a module is executed for this client.
  /// For the special testing client ID "rolldown-tests", all modules are considered executed.
  pub fn is_module_executed(&self, module_id: &str) -> bool {
    self.client_id == "rolldown-tests" || self.executed_modules.contains(module_id)
  }
}
