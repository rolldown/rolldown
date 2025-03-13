use std::{fs, io, path::Path};

use cow_utils::CowUtils;
use oxc::span::Span;
use proc_macro2::TokenStream;

mod rust;
use rust::{ecma_fmt, print_rust, replace_range_string, rust_fmt};

/// Get path for an rust output.
pub fn rust_output_path(krate: &str, path: &str) -> String {
  format!("{krate}/src/generated/{path}")
}

pub fn output_path(dir: &str, path: &str) -> String {
  format!("{dir}/generated/{path}")
}

/// Add a generated file warning to top of file.
pub fn add_header(code: &str, generator_path: &str, comment_start: &str) -> String {
  format!(
    "{comment_start} Auto-generated code, DO NOT EDIT DIRECTLY!\n\
        {comment_start} To edit this generated file you have to edit `{generator_path}`\n\n\
        {code}"
  )
}

/// An output from codegen.
///
/// Can be Rust, Javascript, or other formats.
pub enum Output {
  Rust { path: String, tokens: TokenStream },
  RustString { path: String, code: String },
  EcmaString { path: String, code: String },
  EcmaStringInline { path: String, code: String, span: Span },
}

impl Output {
  /// Convert [`Output`] to [`RawOutput`].
  ///
  /// This involves printing and formatting the output.
  pub fn into_raw(self, generator_path: &str) -> RawOutput {
    let generator_path = generator_path.cow_replace('\\', "/");

    let (path, code) = match self {
      Self::Rust { path, tokens } => {
        let code = print_rust(tokens, &generator_path);
        (path, code)
      }
      Self::RustString { path, code } => {
        let code = rust_fmt(&code);
        (path, code)
      }
      Self::EcmaString { path, code } => {
        let code = ecma_fmt(&code, &path);
        (path, code)
      }
      Self::EcmaStringInline { path, code, span } => {
        let original = fs::read_to_string(&path).unwrap();
        let code = replace_range_string(&original, span, &code);
        let code = ecma_fmt(&code, &path);
        (path, code)
      }
    };
    RawOutput { path, content: code.into_bytes() }
  }
}

/// A raw output from codegen.
///
/// Content is formatted, and in byte array form, ready to write to file.
#[derive(Debug)]
pub struct RawOutput {
  pub path: String,
  pub content: Vec<u8>,
}

impl RawOutput {
  /// Write [`RawOutput`] to file
  pub fn write_to_file(&self) -> io::Result<()> {
    write_to_file_impl(&self.content, &self.path)
  }
}

fn write_to_file_impl(data: &[u8], path: &str) -> io::Result<()> {
  // If contents hasn't changed, don't touch the file
  if let Ok(existing_data) = fs::read(path) {
    if existing_data == data {
      return Ok(());
    }
  }

  let path = Path::new(path);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::write(path, data)
}
