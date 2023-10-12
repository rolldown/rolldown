use derivative::Derivative;
use rustc_hash::FxHashMap;

#[derive(Derivative, Default)]
#[derivative(Debug)]
pub struct OutputOptions {
  pub entry_file_names: Option<String>,
  pub chunk_file_names: Option<String>,
  pub manual_chunks: Option<FxHashMap<String, Vec<String>>>,
  pub dir: Option<String>,
}
