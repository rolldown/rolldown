use oxc::codegen::{self, CodeGenerator, Gen};

pub trait ToSourceString {
  fn to_source_string(&self) -> String;
}

impl<T> ToSourceString for T
where
  T: Gen,
{
  fn to_source_string(&self) -> String {
    let mut codegen = CodeGenerator::new();
    self.gen(&mut codegen, codegen::Context::default());
    codegen.into_source_text()
  }
}
