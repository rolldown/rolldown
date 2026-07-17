use rolldown_utils::pass::Sealed;

fn inspect(sealed: Sealed<u32>) -> u32 {
  sealed.0
}

fn main() {}
