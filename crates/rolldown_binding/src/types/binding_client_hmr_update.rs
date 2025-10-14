use super::binding_hmr_update::BindingHmrUpdate;

#[napi_derive::napi(object, object_from_js = false)]
#[derive(Debug)]
pub struct BindingClientHmrUpdate {
  pub client_id: String,
  pub update: BindingHmrUpdate,
}

impl From<rolldown_common::ClientHmrUpdate> for BindingClientHmrUpdate {
  fn from(value: rolldown_common::ClientHmrUpdate) -> Self {
    Self { client_id: value.client_id, update: BindingHmrUpdate::from(value.update) }
  }
}
