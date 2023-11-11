use derivative::Derivative;

use super::file_name_template::FileNameTemplate;

#[derive(Debug)]
pub enum OutputFormat {
  Esm,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct OutputOptions {
  pub entry_file_names: FileNameTemplate,
  pub chunk_file_names: FileNameTemplate,
  pub dir: String,
  pub format: OutputFormat,
}

impl Default for OutputOptions {
  fn default() -> Self {
    Self {
      entry_file_names: FileNameTemplate::from("[name].js".to_string()),
      chunk_file_names: FileNameTemplate::from("[name]-[hash].js".to_string()),
      dir: "dist".into(),
      format: OutputFormat::Esm,
    }
  }
}
