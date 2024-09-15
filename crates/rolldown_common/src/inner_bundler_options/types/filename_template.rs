use regex::Regex;

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

#[derive(Debug, Default)]
pub struct FileNameRenderOptions<'me> {
  pub name: Option<&'me str>,
  pub hash: Option<&'me str>,
  pub ext: Option<&'me str>,
}

impl FilenameTemplate {
  pub fn render(&self, options: &FileNameRenderOptions) -> String {
    let mut tmp = self.template.clone();
    if let Some(name) = options.name {
      tmp = tmp.replace("[name]", name);
    }
    if let Some(hash) = options.hash {
      let re = Regex::new(r"\[hash(?::(\d+))?\]").unwrap();
      tmp = re.replace(&tmp, hash).to_string();
    }
    if let Some(ext) = options.ext {
      tmp = tmp.replace("[ext]", ext).replace("[extname]", &format!(".{ext}"));
    }
    tmp
  }
}

#[test]
fn test_basic_replacement() {
  let template = FilenameTemplate::new("[name]-[hash:8].js".to_string());
  let options = FileNameRenderOptions { name: Some("file"), hash: Some("abcdef12"), ext: None };
  let result = template.render(&options);
  assert_eq!(result, "file-abcdef12.js");
}
