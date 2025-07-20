use std::path::Path;

use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

#[derive(Debug)]
pub struct Remote {
  pub r#type: Option<String>,
  pub entry: String,
  pub name: String,
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
pub struct Manifest {
  pub file_path: Option<String>,
  pub disable_assets_analyze: Option<bool>,
  pub file_name: Option<String>,
}

impl Manifest {
  pub fn normalize_file_name(&self) -> String {
    let file_name = self.file_name.as_deref().unwrap_or("mf-manifest.json");
    if let Some(file_path) = &self.file_path {
      return Path::new(file_path).join(file_name).to_slash_lossy().to_string();
    }
    file_name.into()
  }
}

/// https://module-federation.io/configure/index.html
#[derive(Debug)]
pub struct ModuleFederationPluginOption {
  pub name: String,
  pub filename: Option<String>,
  pub exposes: Option<FxHashMap<String, String>>,
  pub remotes: Option<Vec<Remote>>,
  pub shared: Option<FxHashMap<String, Shared>>,
  pub runtime_plugins: Option<Vec<String>>,
  pub manifest: Option<Manifest>,
  pub get_public_path: Option<String>,
}
