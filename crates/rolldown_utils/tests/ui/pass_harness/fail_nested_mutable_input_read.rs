use std::convert::Infallible;

use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

struct MutableReads<'a> {
  value: &'a mut u32,
}

#[derive(Clone, Copy)]
struct MutableReadPass;

impl Pass for MutableReadPass {
  type InputRead<'a> = MutableReads<'a>;
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = ();
  type Error = Infallible;

  fn run<'run>(
    self,
    token: RunToken<'run, Self>,
    _cx: &mut PassCtx<'_>,
    _read: Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish((), ()))
  }
}

fn main() {}
