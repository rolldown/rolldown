mod common;

use std::marker::PhantomData;

use common::EchoPass;
use rolldown_utils::pass::RunToken;

fn main() {
  let _ = RunToken::<EchoPass> {
    _brand: todo!(),
    _lifetime: PhantomData,
    _pass: PhantomData,
  };
}
