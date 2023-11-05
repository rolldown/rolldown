use std::{path::Path, sync::Arc};

use rolldown_error::BuildError;
use rolldown_fs::FileSystem;
use sugar_path::AsPath;

use super::{
  bundle::asset::Asset,
  graph::graph::Graph,
  options::{
    normalized_input_options::NormalizedInputOptions,
    normalized_output_options::NormalizedOutputOptions,
  },
};
use crate::{bundler::bundle::bundle::Bundle, plugin::plugin::BoxPlugin, InputOptions};

type BuildResult<T> = Result<T, Vec<BuildError>>;

pub struct Bundler {
  input_options: NormalizedInputOptions,
  _plugins: Vec<BoxPlugin>,
  fs: Arc<dyn FileSystem>,
}

impl Bundler {
  pub fn new(input_options: InputOptions, fs: Arc<dyn FileSystem>) -> Self {
    // rolldown_tracing::enable_tracing_on_demand();
    let normalized = NormalizedInputOptions::from_input_options(input_options);
    Self { input_options: normalized, _plugins: vec![], fs }
  }

  pub fn with_plugins(
    input_options: InputOptions,
    plugins: Vec<BoxPlugin>,
    fs: Arc<dyn FileSystem>,
  ) -> Self {
    // rolldown_tracing::enable_tracing_on_demand();
    let normalized = NormalizedInputOptions::from_input_options(input_options);
    Self { input_options: normalized, _plugins: plugins, fs }
  }

  pub async fn write(&mut self, output_options: crate::OutputOptions) -> BuildResult<Vec<Asset>> {
    let dir = output_options.dir.clone().unwrap_or_else(|| {
      self.input_options.cwd.as_path().join("dist").to_string_lossy().to_string()
    });
    let normalized = NormalizedOutputOptions::from_output_options(output_options);

    let assets = self.build(normalized, self.fs.clone()).await?;

    self.fs.create_dir_all(dir.as_path()).unwrap_or_else(|_| {
      panic!(
        "Could not create directory for output chunks: {:?} \ncwd: {}",
        dir.as_path(),
        self.input_options.cwd.display()
      )
    });
    for chunk in &assets {
      let dest = dir.as_path().join(&chunk.file_name);
      if let Some(p) = dest.parent() {
        if !p.exists() {
          self.fs.create_dir_all(p).unwrap();
        }
      };
      self.fs::write(dest, &chunk.content).unwrap_or_else(|_| {
        panic!("Failed to write file in {:?}", dir.as_path().join(&chunk.file_name))
      });
    }

    Ok(assets)
  }

  pub async fn generate(
    &mut self,
    output_options: crate::OutputOptions,
  ) -> BuildResult<Vec<Asset>> {
    let normalized = NormalizedOutputOptions::from_output_options(output_options);
    self.build(normalized, self.fs.clone()).await
  }

  async fn build(
    &mut self,
    output_options: NormalizedOutputOptions,
    fs: Arc<dyn FileSystem>,
  ) -> BuildResult<Vec<Asset>> {
    tracing::trace!("NormalizedInputOptions {:#?}", self.input_options);
    tracing::trace!("NormalizedOutputOptions: {output_options:#?}",);

    let mut graph = Graph::default();
    graph.generate_module_graph(&self.input_options, fs).await?;

    let mut bundle = Bundle::new(&mut graph, &output_options);
    let assets = bundle.generate(&self.input_options);

    Ok(assets)
  }
}
