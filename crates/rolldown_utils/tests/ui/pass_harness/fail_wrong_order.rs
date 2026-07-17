mod common;

use common::EchoPass;
use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

fn main() {
  let mut pipeline = PassPipelineCtx::new();
  let _ = run_infallible_pass(EchoPass, &mut pipeline, &produced, 0);
  let produced = 1_u32;
}
