use std::convert::Infallible;

use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

#[derive(Clone, Copy)]
struct InspectingPass;

impl Pass for InspectingPass {
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
    let raw = token.finish((), ());
    let _ = raw.read;
    Ok(raw)
  }
}

fn main() {}
