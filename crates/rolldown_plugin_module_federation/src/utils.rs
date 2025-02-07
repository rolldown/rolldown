use crate::{ModuleFederationPluginOption, INIT_REMOTE_MODULE_PREFIX, INIT_SHARED_MODULE_PREFIX};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RemoteModuleType {
  Shared,
  Remote,
}

impl RemoteModuleType {
  pub fn is_shared(&self) -> bool {
    matches!(self, RemoteModuleType::Shared)
  }
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

pub fn get_remote_module_prefix(remote_module_type: RemoteModuleType) -> &'static str {
  if remote_module_type.is_shared() {
    INIT_SHARED_MODULE_PREFIX
  } else {
    INIT_REMOTE_MODULE_PREFIX
  }
}
