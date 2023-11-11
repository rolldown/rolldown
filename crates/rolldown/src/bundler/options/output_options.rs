use derivative::Derivative;

#[derive(Debug)]
pub enum OutputFormat {
  Esm,
}

#[derive(Derivative, Default)]
#[derivative(Debug)]
pub struct OutputOptions {
  pub entry_file_names: Option<String>,
  pub chunk_file_names: Option<String>,
  pub dir: Option<String>,
  pub format: Option<OutputFormat>,
}
