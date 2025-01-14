use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct Remote {
  pub r#type: Option<String>,
  pub entry: String,
  pub entry_global_name: Option<String>,
  pub share_scope: Option<String>,
}

#[derive(Debug)]
pub struct Shared {
  pub version: Option<String>,
  pub share_scope: Option<String>,
  pub singleton: Option<bool>,
  pub required_version: Option<String>,
  pub strict_version: Option<bool>,
}

#[derive(Debug)]
pub struct ModuleFederationPluginOption {
  pub name: String,
  pub filename: Option<String>,
  pub expose: Option<FxHashMap<String, String>>,
  pub remotes: Option<FxHashMap<String, Remote>>,
  pub shared: Option<FxHashMap<String, Shared>>,
}
