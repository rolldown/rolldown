use derivative::Derivative;

use super::file_name_template::FileNameTemplate;

#[derive(Debug)]
pub enum OutputFormat {
  Esm,
  Cjs,
}

#[derive(Debug)]
pub enum SourceMapType {
  File,
  Inline,
  Hidden,
}

impl From<String> for SourceMapType {
  fn from(value: String) -> Self {
    match value.as_str() {
      "file" => SourceMapType::File,
      "inline" => SourceMapType::Inline,
      "hidden" => SourceMapType::Hidden,
      _ => unreachable!("unknown sourcemap type"),
    }
  }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct OutputOptions {
  pub entry_file_names: FileNameTemplate,
  pub chunk_file_names: FileNameTemplate,
  pub dir: String,
  pub format: OutputFormat,
  pub sourcemap: SourceMapType,
}

impl Default for OutputOptions {
  fn default() -> Self {
    Self {
      entry_file_names: FileNameTemplate::from("[name].js".to_string()),
      chunk_file_names: FileNameTemplate::from("[name]-[hash].js".to_string()),
      dir: "dist".into(),
      format: OutputFormat::Esm,
      sourcemap: SourceMapType::Hidden,
    }
  }
}
