use crate::ModuleFederationPluginOption;

pub fn is_remote_module(request: &str, options: &ModuleFederationPluginOption) -> bool {
  if let Some(remotes) = options.remotes.as_ref() {
    if remotes.iter().any(|remote| request.starts_with(&remote.name)) {
      return true;
    }
  }
  false
}
