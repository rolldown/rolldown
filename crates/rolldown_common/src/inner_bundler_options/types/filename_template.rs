use rolldown_utils::replace_all_placeholder::{replace_all_placeholder, Replacer};

#[derive(Debug)]
pub struct FilenameTemplate {
  template: String,
}

impl FilenameTemplate {
  pub fn new(template: String) -> Self {
    Self { template }
  }

  pub fn template(&self) -> &str {
    &self.template
  }
}

impl From<String> for FilenameTemplate {
  fn from(template: String) -> Self {
    Self::new(template)
  }
}

impl FilenameTemplate {
  pub fn render(
    self,
    name: Option<&str>,
    extension: Option<&str>,
    hash_replacer: Option<impl Replacer>,
  ) -> String {
    let mut tmp = self.template;
    if let Some(name) = name {
      tmp = replace_all_placeholder(tmp, "[name]", name);
    }
    if let Some(hash_replacer) = hash_replacer {
      tmp = replace_all_placeholder(tmp, "[hash]", hash_replacer);
    }
    if let Some(ext) = extension {
      let extname = if ext.is_empty() { "" } else { &format!(".{ext}") };
      tmp = replace_all_placeholder(tmp, "[ext]", ext);
      tmp = replace_all_placeholder(tmp, "[extname]", extname);
    }
    tmp
  }

  pub fn has_hash_pattern(&self) -> bool {
    let start = self.template.find("[hash");
    start.is_some_and(|start| {
      let pattern = &self.template[start + 5..];
      pattern.starts_with(']') || (pattern.starts_with(':') && pattern.contains(']'))
    })
  }
}

#[test]
fn basic() {
  FilenameTemplate::new("[name]-[hash:8].js".to_string());
}

#[test]
fn hash_with_len() {
  let filename_template = FilenameTemplate::new("[name]-[hash:3]-[hash:3].js".to_string());

  let mut hash_iter = ["abc", "def"].iter();
  let hash_replacer = filename_template.has_hash_pattern().then_some(|_| hash_iter.next().unwrap());

  let filename = filename_template.render(Some("hello"), None, hash_replacer);

  assert_eq!(filename, "hello-abc-def.js");
}
