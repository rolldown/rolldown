use rolldown_utils::pass::Sealed;

fn unwrap(sealed: Sealed<u32>) -> u32 {
  sealed.into_inner()
}

fn main() {}
