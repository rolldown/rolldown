use rolldown_error::BuildResult;

use crate::types::generator::{GenerateContext, GenerateOutput, Generator};
use anyhow::Result;

pub struct FileGenerator;

impl Generator for FileGenerator {
  async fn instantiate_chunk<'a>(
    ctx: &mut GenerateContext<'a>,
  ) -> Result<BuildResult<GenerateOutput>> {
    todo!()
  }
}
