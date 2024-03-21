use derivative::Derivative;

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

impl SourceMapType {
  pub fn is_hidden(&self) -> bool {
    matches!(self, Self::Hidden)
  }
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

#[derive(Derivative, Default)]
#[derivative(Debug)]
pub struct OutputOptions {
  pub entry_file_names: Option<String>,
  pub chunk_file_names: Option<String>,
  pub dir: Option<String>,
  pub format: Option<OutputFormat>,
  pub sourcemap: Option<SourceMapType>,
  pub banner: Option<String>,
}

// impl Default for OutputOptions {
//   fn default() -> Self {
//     Self {
//       entry_file_names: FileNameTemplate::from("[name].js".to_string()),
//       chunk_file_names: FileNameTemplate::from("[name]-[hash].js".to_string()),
//       dir: "dist".into(),
//       format: OutputFormat::Esm,
//       sourcemap: SourceMapType::Hidden,
//     }
//   }
// }
