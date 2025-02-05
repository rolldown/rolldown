use crate::ModuleFederationPluginOption;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum RemoteModuleType {
  Shared,
  Remote,
}

pub fn detect_remote_module_type(
  request: &str,
  options: &ModuleFederationPluginOption,
) -> Option<RemoteModuleType> {
  if let Some(remotes) = options.remotes.as_ref() {
    if remotes.iter().any(|remote| request.starts_with(&remote.name)) {
      return Some(RemoteModuleType::Remote);
    }
  }
  if let Some(shared) = options.shared.as_ref() {
    if shared.iter().any(|(key, _)| request == key) {
      return Some(RemoteModuleType::Shared);
    }
  }
  None
}
