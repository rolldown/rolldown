use std::convert::Infallible;

use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

#[derive(Clone, Copy)]
struct FirstPass;

#[derive(Clone, Copy)]
struct SecondPass;

impl Pass for FirstPass {
  type InputRead<'a> = ();
  type InputOwned = ();
  type OutputRead = ();
  type OutputOwned = ();
  type Error = Infallible;

  fn run<'run>(
    self,
    token: RunToken<'run, Self>,
    cx: &mut PassCtx<'_>,
    (): Self::InputRead<'_>,
    (): Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    SecondPass.run(token, cx, (), ())
  }
}

impl Pass for SecondPass {
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

fn main() {}
