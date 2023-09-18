#[derive(Debug)]
pub struct FileNameTemplate {
  template: String,
}

impl FileNameTemplate {
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
}

impl FileNameTemplate {
  pub fn render(&self, options: FileNameRenderOptions) -> String {
    let mut tmp = self.template.clone();
    if let Some(name) = options.name {
      tmp = tmp.replace("[name]", name);
    }
    tmp
  }
}
