#[derive(valuable::Valuable, ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct InputItem {
  pub name: Option<String>,
  pub import: String,
}
