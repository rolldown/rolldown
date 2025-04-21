use crate::types::input_item::InputItem;

#[derive(valuable::Valuable, ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct SessionPreface {
  #[ts(type = "'SessionPreface'")]
  pub kind: &'static str,
  pub input: Vec<InputItem>,
  pub cwd: String,
  pub format: String,
  pub dir: Option<String>,
  pub minify: bool,
  pub platform: String,
  pub plugins: Vec<String>,
}
