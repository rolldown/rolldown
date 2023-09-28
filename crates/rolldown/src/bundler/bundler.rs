use sugar_path::AsPath;

use super::{
  bundle::asset::Asset,
  graph::graph::Graph,
  options::{
    normalized_input_options::NormalizedInputOptions,
    normalized_output_options::NormalizedOutputOptions,
  },
};
use crate::{bundler::bundle::bundle::Bundle, InputOptions};

pub struct Bundler {
  input_options: NormalizedInputOptions,
}

impl Bundler {
  pub fn new(input_options: InputOptions) -> Self {
    // rolldown_tracing::enable_tracing_on_demand();
    let normalized = NormalizedInputOptions::from_input_options(input_options);
    Self {
      input_options: normalized,
    }
  }

  pub async fn write(
    &mut self,
    output_options: crate::OutputOptions,
  ) -> anyhow::Result<Vec<Asset>> {
    let dir = output_options.dir.clone().unwrap_or_else(|| {
      self
        .input_options
        .cwd
        .as_path()
        .join("dist")
        .to_string_lossy()
        .to_string()
    });
    let normalized = NormalizedOutputOptions::from_output_options(output_options);

    let assets = self.build(normalized).await?;

    std::fs::create_dir_all(&dir).unwrap_or_else(|_| {
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
          std::fs::create_dir_all(p)?;
        }
      };
      std::fs::write(dest, &chunk.content).unwrap_or_else(|_| {
        panic!(
          "Failed to write file in {:?}",
          dir.as_path().join(&chunk.file_name)
        )
      });
    }

    Ok(assets)
  }

  pub async fn generate(
    &mut self,
    output_options: crate::OutputOptions,
  ) -> anyhow::Result<Vec<Asset>> {
    let normalized = NormalizedOutputOptions::from_output_options(output_options);
    self.build(normalized).await
  }

  async fn build(&mut self, output_options: NormalizedOutputOptions) -> anyhow::Result<Vec<Asset>> {
    tracing::trace!("NormalizedInputOptions {:#?}", self.input_options);
    tracing::trace!("NormalizedOutputOptions: {output_options:#?}",);

    let mut graph = Graph::default();
    graph.generate_module_graph(&self.input_options).await?;

    let mut bundle = Bundle::new(&mut graph, &output_options);
    let assets = bundle.generate(&self.input_options)?;

    Ok(assets)
  }
}
