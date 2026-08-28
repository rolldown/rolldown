use napi::Either;
use rolldown_common::{ManglePropertiesPattern, ManglePropertiesPatterns};

fn mangle_properties_pattern_to_napi(pattern: &ManglePropertiesPattern) -> oxc_napi::JsRegExp {
  oxc_napi::JsRegExp::new(pattern.source.clone(), pattern.flags.clone())
}

pub fn mangle_options_to_napi_mangle_options(
  mangle: &oxc::minifier::MangleOptions,
) -> oxc_minify_napi::MangleOptions {
  oxc_minify_napi::MangleOptions {
    toplevel: mangle.top_level,
    keep_names: {
      let keep_names = oxc_minify_napi::MangleOptionsKeepNames {
        function: mangle.keep_names.function,
        class: mangle.keep_names.class,
      };
      Some(Either::B(keep_names))
    },
    reserved: Some(mangle.reserved.iter().map(ToString::to_string).collect()),
    debug: Some(mangle.debug),
  }
}

pub fn mangle_properties_options_to_napi_mangle_properties_options(
  mangle_properties: &oxc::minifier::ManglePropertiesOptions,
  patterns: &ManglePropertiesPatterns,
) -> oxc_minify_napi::ManglePropertiesOptions {
  oxc_minify_napi::ManglePropertiesOptions {
    include: mangle_properties_pattern_to_napi(&patterns.include),
    exclude: patterns.exclude.as_ref().map(mangle_properties_pattern_to_napi),
    reserved: Some(mangle_properties.reserved.iter().map(ToString::to_string).collect()),
    quoted: Some(mangle_properties.mangle_quoted),
    debug: Some(mangle_properties.debug),
    cache: Some(
      mangle_properties
        .cache
        .iter()
        .map(|(original, target)| {
          let target =
            target.as_ref().map_or(Either::B(false), |target| Either::A(target.to_string()));
          (original.to_string(), target)
        })
        .collect(),
    ),
  }
}

pub fn compress_options_to_napi_compress_options(
  compress: &oxc::minifier::CompressOptions,
) -> oxc_minify_napi::CompressOptions {
  oxc_minify_napi::CompressOptions {
    target: Some(napi::Either::A(compress.target.to_string())),
    drop_console: Some(compress.drop_console),
    drop_debugger: Some(compress.drop_debugger),
    drop_labels: Some(compress.drop_labels.iter().cloned().collect()),
    unused: Some(match compress.unused {
      oxc::minifier::CompressOptionsUnused::Remove => napi::Either::A(true),
      oxc::minifier::CompressOptionsUnused::KeepAssign => napi::Either::B("keep_assign".to_owned()),
      oxc::minifier::CompressOptionsUnused::Keep => napi::Either::A(false),
    }),
    keep_names: {
      let keep_names = oxc_minify_napi::CompressOptionsKeepNames {
        function: compress.keep_names.function,
        class: compress.keep_names.class,
      };
      Some(keep_names)
    },
    join_vars: Some(compress.join_vars),
    sequences: Some(compress.sequences),
    max_iterations: compress.max_iterations,
    // available in the root treeshake options
    treeshake: None,
  }
}

pub fn codegen_options_to_napi_codegen_options(
  remove_whitespace: bool,
) -> oxc_minify_napi::CodegenOptions {
  oxc_minify_napi::CodegenOptions {
    remove_whitespace: Some(remove_whitespace),
    legal_comments: None,
  }
}
