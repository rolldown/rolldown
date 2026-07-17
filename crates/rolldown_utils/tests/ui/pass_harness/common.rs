use std::convert::Infallible;

use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

#[derive(Clone, Copy)]
pub struct EchoPass;

impl Pass for EchoPass {
  type InputRead<'a> = &'a u32;
  type InputOwned = u32;
  type OutputRead = u32;
  type OutputOwned = u32;
  type Error = Infallible;

  fn run<'run>(
    self,
    token: RunToken<'run, Self>,
    _cx: &mut PassCtx<'_>,
    read: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish(*read, owned))
  }
}

#[derive(Clone, Copy)]
pub struct OtherPass;

impl Pass for OtherPass {
  type InputRead<'a> = &'a u32;
  type InputOwned = u32;
  type OutputRead = u32;
  type OutputOwned = u32;
  type Error = Infallible;

  fn run<'run>(
    self,
    token: RunToken<'run, Self>,
    _cx: &mut PassCtx<'_>,
    read: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish(*read, owned))
  }
}
