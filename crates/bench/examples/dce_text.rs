//! PoC helper: runs oxc `Minifier::dce` on a JS file (parse -> DCE -> print), like
//! `minify_chunks` does with `minify: 'dce-only'`. Usage: `dce_text <in.js> <out.js>`.
use oxc::{
  allocator::Allocator,
  codegen::Codegen,
  minifier::{CompressOptions, Minifier, MinifierOptions},
  parser::Parser,
  span::SourceType,
};

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let source = std::fs::read_to_string(&args[1]).unwrap();
  let allocator = Allocator::default();
  let mut program = Parser::new(&allocator, &source, SourceType::mjs()).parse().program;
  let options = MinifierOptions {
    mangle: None,
    mangle_properties: None,
    compress: Some(CompressOptions::dce()),
  };
  Minifier::new(options).dce(&allocator, &mut program);
  std::fs::write(&args[2], Codegen::new().build(&program).code).unwrap();
}
