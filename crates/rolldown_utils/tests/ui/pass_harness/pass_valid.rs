mod common;

use common::EchoPass;
use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

fn main() {
  let mut pipeline = PassPipelineCtx::new();
  let (read, owned) = run_infallible_pass(EchoPass, &mut pipeline, &1, 2);
  assert_eq!(*read, 1);
  assert_eq!(owned, 2);
}
