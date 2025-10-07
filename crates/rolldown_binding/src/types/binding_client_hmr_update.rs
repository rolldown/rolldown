use napi_derive::napi;

use super::binding_hmr_update::BindingHmrUpdate;

#[napi]
#[derive(Debug)]
pub struct BindingClientHmrUpdate {
  client_id: String,
  update: BindingHmrUpdate,
}

#[napi]
impl BindingClientHmrUpdate {
  pub fn new(client_id: String, update: BindingHmrUpdate) -> Self {
    Self { client_id, update }
  }

  #[napi(getter)]
  pub fn client_id(&self) -> String {
    self.client_id.clone()
  }

  #[napi(getter)]
  pub fn update(&mut self) -> BindingHmrUpdate {
    std::mem::replace(&mut self.update, BindingHmrUpdate::Noop)
  }
}

impl From<rolldown_common::ClientHmrUpdate> for BindingClientHmrUpdate {
  fn from(value: rolldown_common::ClientHmrUpdate) -> Self {
    Self { client_id: value.client_id, update: BindingHmrUpdate::from(value.update) }
  }
}
