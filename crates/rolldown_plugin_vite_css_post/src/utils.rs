use crate::ViteCssPostPlugin;

pub fn extract_index(id: &str) -> Option<&str> {
  let s = id.split_once("&index=")?.1;
  let end = s.as_bytes().iter().take_while(|b| b.is_ascii_digit()).count();
  (end > 0).then_some(&s[..end])
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct UrlEmitTasks {
  pub range: (usize, usize),
  pub replacement: String,
}

impl ViteCssPostPlugin {
  #[allow(clippy::unused_async)]
  pub async fn resolve_asset_urls_in_css(&self) -> String {
    todo!()
  }

  #[allow(clippy::unused_async)]
  pub async fn finalize_css(&self, _content: String) -> String {
    todo!()
  }
}
