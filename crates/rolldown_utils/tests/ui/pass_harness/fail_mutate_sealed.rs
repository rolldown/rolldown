use std::convert::Infallible;

use rolldown_utils::pass::{
  Pass, PassCtx, PassPipelineCtx, RawPassOutput, RunToken, run_infallible_pass,
};

struct FinalValue(u32);

impl FinalValue {
  fn set(&mut self, value: u32) {
    self.0 = value;
  }
}

#[derive(Clone, Copy)]
struct FinalizePass;

impl Pass for FinalizePass {
  type InputRead<'a> = ();
  type InputOwned = ();
  type OutputRead = FinalValue;
  type OutputOwned = ();
  type Error = Infallible;

  fn run<'run>(
    self,
    token: RunToken<'run, Self>,
    _cx: &mut PassCtx<'_>,
    (): Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish(FinalValue(0), ()))
  }
}

fn main() {
  let mut pipeline = PassPipelineCtx::new();
  let (mut value, ()) = run_infallible_pass(FinalizePass, &mut pipeline, (), ());
  value.set(1);
}
