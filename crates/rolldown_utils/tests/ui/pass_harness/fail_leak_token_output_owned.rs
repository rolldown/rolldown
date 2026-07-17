use std::convert::Infallible;

use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

#[derive(Clone, Copy)]
struct LeakingPass;

impl Pass for LeakingPass {
  type InputRead<'a> = ();
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = RunToken<'static, Self>;
  type Error = Infallible;

  fn run<'run>(
    self,
    token: RunToken<'run, Self>,
    _cx: &mut PassCtx<'_>,
    (): Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish((), token))
  }
}

fn main() {}
