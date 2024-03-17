#[derive(Debug)]
pub struct InputItem {
  pub name: Option<String>,
  pub import: String,
}

impl From<String> for InputItem {
  fn from(value: String) -> Self {
    Self { name: None, import: value }
  }
}
