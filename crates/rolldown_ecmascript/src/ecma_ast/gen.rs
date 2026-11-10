use oxc::codegen::{Codegen, CodegenOptions, CommentOptions, Context, Gen};

pub trait ToSourceString {
  fn to_source_string(&self) -> String;
  /// `to_source_string` with the `ascii_only` option of the output, for text that goes into
  /// the output with no second print.
  fn to_output_source_string(&self, ascii_only: bool) -> String;
}

impl<T> ToSourceString for T
where
  T: Gen,
{
  fn to_source_string(&self) -> String {
    let mut codegen = Codegen::new().with_options(CodegenOptions {
      comments: CommentOptions { normal: false, ..CommentOptions::default() },
      ..CodegenOptions::default()
    });
    self.r#gen(&mut codegen, Context::default());
    codegen.into_source_text()
  }

  fn to_output_source_string(&self, ascii_only: bool) -> String {
    let mut codegen = Codegen::new().with_options(CodegenOptions {
      comments: CommentOptions { normal: false, ..CommentOptions::default() },
      ascii_only,
      ..CodegenOptions::default()
    });
    self.r#gen(&mut codegen, Context::default());
    codegen.into_source_text()
  }
}
