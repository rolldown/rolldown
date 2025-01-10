use rustc_hash::FxHashMap;

#[derive(Debug)]
pub struct Remote {
  pub r#type: Option<String>,
  pub name: String,
  pub entry: String,
  pub entry_global_name: Option<String>,
  pub share_scope: Option<String>,
}

#[derive(Debug)]
pub struct Shared {
  pub name: String,
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
  pub expose: FxHashMap<String, String>,
  pub remotes: FxHashMap<String, Remote>,
  pub shared: FxHashMap<String, Shared>,
}
