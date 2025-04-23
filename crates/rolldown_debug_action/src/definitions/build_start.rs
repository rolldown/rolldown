#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct BuildStart {
  #[ts(type = "'BuildStart'")]
  pub action: &'static str,
}
