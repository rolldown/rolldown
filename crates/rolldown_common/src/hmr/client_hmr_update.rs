use super::hmr_update::HmrUpdate;

#[derive(Debug)]
pub struct ClientHmrUpdate {
  pub client_id: String,
  pub update: HmrUpdate,
}
