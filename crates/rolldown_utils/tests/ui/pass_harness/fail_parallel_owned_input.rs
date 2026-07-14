use std::convert::Infallible;

use rolldown_utils::pass::{
  Pass, PassCtx, PassPipelineCtx, RawPassOutput, RunToken, run_infallible_pass,
};

#[derive(Clone, Copy)]
struct OwnedPass;

impl Pass for OwnedPass {
  type InputRead<'a> = ();
  type InputOwned = String;
  type OutputRead = ();
  type OutputOwned = String;
  type Error = Infallible;

  fn run<'run>(
    self,
    token: RunToken<'run, Self>,
    _cx: &mut PassCtx<'_>,
    (): Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    Ok(token.finish((), owned))
  }
}

fn main() {
  let owned = String::from("one owner");
  let mut left = PassPipelineCtx::new();
  let mut right = PassPipelineCtx::new();
  let _ = rayon::join(
    || run_infallible_pass(OwnedPass, &mut left, (), owned),
    || run_infallible_pass(OwnedPass, &mut right, (), owned),
  );
}
