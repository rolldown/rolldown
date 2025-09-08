use napi::Either;

pub fn mangle_options_to_napi_mangle_options(
  mangle: &oxc::minifier::MangleOptions,
) -> oxc_minify_napi::MangleOptions {
  oxc_minify_napi::MangleOptions {
    toplevel: Some(mangle.top_level),
    keep_names: {
      let keep_names = oxc_minify_napi::MangleOptionsKeepNames {
        function: mangle.keep_names.function,
        class: mangle.keep_names.class,
      };
      Some(Either::B(keep_names))
    },
    debug: Some(mangle.debug),
  }
}

pub fn compress_options_to_napi_compress_options(
  compress: &oxc::minifier::CompressOptions,
) -> oxc_minify_napi::CompressOptions {
  oxc_minify_napi::CompressOptions {
    target: Some(compress.target.to_string()),
    drop_console: Some(compress.drop_console),
    drop_debugger: Some(compress.drop_debugger),
    unused: Some(match compress.unused {
      oxc::minifier::CompressOptionsUnused::Remove => "remove".to_owned(),
      oxc::minifier::CompressOptionsUnused::KeepAssign => "keep-assign".to_owned(),
      oxc::minifier::CompressOptionsUnused::Keep => "keep".to_owned(),
    }),
    keep_names: {
      let keep_names = oxc_minify_napi::CompressOptionsKeepNames {
        function: compress.keep_names.function,
        class: compress.keep_names.class,
      };
      Some(keep_names)
    },
  }
}

pub fn codegen_options_to_napi_codegen_options(
  remove_whitespace: bool,
) -> oxc_minify_napi::CodegenOptions {
  oxc_minify_napi::CodegenOptions { remove_whitespace: Some(remove_whitespace) }
}
