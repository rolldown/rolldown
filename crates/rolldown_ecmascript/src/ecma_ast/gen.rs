use oxc::codegen::{self, Codegen, CodegenOptions, Gen};

pub trait ToSourceString {
  fn to_source_string(&self) -> String;
}

impl<T> ToSourceString for T
where
  T: Gen,
{
  fn to_source_string(&self) -> String {
    let mut codegen =
      Codegen::new().with_options(CodegenOptions { comments: false, ..CodegenOptions::default() });
    self.r#gen(&mut codegen, codegen::Context::default());
    codegen.into_source_text()
  }
}
