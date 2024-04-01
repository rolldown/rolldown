use core::hash;

use regex::Regex;
#[derive(Debug)]
pub struct FileNameTemplate {
  template: String,
}

impl FileNameTemplate {
  #[allow(dead_code)]
  pub fn new(template: String) -> Self {
    Self { template }
  }
}

impl From<String> for FileNameTemplate {
  fn from(template: String) -> Self {
    Self { template }
  }
}

#[derive(Debug, Default)]
pub struct FileNameRenderOptions<'me> {
  pub name: Option<&'me str>,
  pub hash: Option<&'me str>,
}

impl FileNameTemplate {
  pub fn render(&self, options: &FileNameRenderOptions) -> String {
    let hash_regex: Regex = Regex::new(r"\[(\w+)(:\d+)?]").unwrap();
    let hash = options.hash.unwrap();
    let mut tmp = self.template.clone();
    if let Some(name) = options.name {
      tmp = tmp.replace("[name]", name);
    }

    if let Some(hash_cap) = hash_regex.captures(&tmp) {
      if let Some(hash_len) = hash_cap.get(2) {}
      if let Some(hash_str) = hash_cap.get(1) {
        tmp = tmp.replace("[hash]", hash)
      }
    };

    if let Some(hash) = options.hash {
      tmp = tmp.replace("[hash]", hash)
    }
    tmp
  }
}

#[test]
fn file_name_template_render() {
  let file_name_template = FileNameTemplate { template: "[name]-[hash:8].js".to_string() };

  let name_res =
    file_name_template.render(&FileNameRenderOptions { name: Some("test"), hash: Some("123") });

  assert_eq!(name_res, "test-123.js")
}
