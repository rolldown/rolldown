use oxc::codegen::{self, Codegen, Gen};

pub trait ToSourceString {
  fn to_source_string(&self) -> String;
}

impl<T> ToSourceString for T
where
  T: Gen,
{
  fn to_source_string(&self) -> String {
    let mut codegen = Codegen::new();
    self.r#gen(&mut codegen, codegen::Context::default());
    codegen.into_source_text()
  }
}
