use std::convert::Infallible;

use rolldown_utils::pass::{
  Pass, PassCtx, PassPipelineCtx, RawPassOutput, RunToken, run_infallible_pass,
};

#[derive(Clone, Copy)]
struct StatefulPass(u8);

impl Pass for StatefulPass {
  type InputRead<'a> = ();
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = ();
  type Error = Infallible;

  fn run<'run>(
    self,
    token: RunToken<'run, Self>,
    _cx: &mut PassCtx<'_>,
    (): Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish((), ()))
  }
}

fn main() {
  let mut pipeline = PassPipelineCtx::new();
  let _ = run_infallible_pass(StatefulPass(1), &mut pipeline, (), ());
}
