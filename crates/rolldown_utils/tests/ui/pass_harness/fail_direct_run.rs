mod common;

use common::EchoPass;
use rolldown_utils::pass::{Pass, PassCtx};

fn direct(cx: &mut PassCtx<'_>) {
  let _ = EchoPass.run(cx, &1, 2);
}

fn main() {}
