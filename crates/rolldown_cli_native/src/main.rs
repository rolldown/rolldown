#[global_allocator]
static ALLOC: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

use std::path::PathBuf;
use std::time::Instant;

// Hold trace guard alive until program exit
use rolldown_tracing::try_init_tracing;

use rolldown::{
  Bundler, BundlerOptions, InputItem, IsExternal, OutputFormat, RawMinifyOptions, SourceMapType,
};
use rolldown_common::bundler_options::CommentsOptions;
use rolldown_utils::js_regex::HybridRegex;
use rolldown_utils::pattern_filter::StringOrRegex;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
  let _trace_guard = try_init_tracing();

  let args: Vec<String> = std::env::args().collect();

  // Native Rolldown CLI — bypasses Node.js for maximum performance
  //
  // Usage: rolldown_cli_native [options]
  //   --entry <path>         Entry point (default: ./src/index.jsx)
  //   --dir <path>           Output directory (default: ./dist)
  //   --format <esm|cjs>     Output format (default: esm)
  //   --minify               Enable minification
  //   --sourcemap            Enable sourcemaps
  //   --cwd <path>           Working directory
  //   --external-css         Externalize .css imports
  //   --no-comments          Strip comments
  //   --define K=V           Define global constant (repeatable)

  let mut entry = String::from("./src/index.jsx");
  let mut dir = String::from("./dist");
  let mut format = OutputFormat::Esm;
  let mut minify = false;
  let mut sourcemap = false;
  let mut cwd: Option<PathBuf> = None;
  let mut external_css = false;
  let mut external_patterns: Vec<String> = Vec::new();
  let mut no_comments = false;
  let mut defines: Vec<(String, String)> = Vec::new();

  let mut i = 1;
  while i < args.len() {
    match args[i].as_str() {
      "--entry" => {
        i += 1;
        entry = args[i].clone();
      }
      "--dir" => {
        i += 1;
        dir = args[i].clone();
      }
      "--format" => {
        i += 1;
        format = match args[i].as_str() {
          "cjs" => OutputFormat::Cjs,
          "iife" => OutputFormat::Iife,
          _ => OutputFormat::Esm,
        };
      }
      "--minify" => {
        minify = true;
      }
      "--sourcemap" => {
        sourcemap = true;
      }
      "--cwd" => {
        i += 1;
        cwd = Some(PathBuf::from(&args[i]));
      }
      "--external-css" => {
        external_css = true;
      }
      "--external-pattern" => {
        i += 1;
        external_patterns.push(args[i].clone());
      }
      "--no-comments" => {
        no_comments = true;
      }
      "--define" => {
        i += 1;
        if let Some((k, v)) = args[i].split_once('=') {
          defines.push((k.to_string(), v.to_string()));
        }
      }
      _ => {
        eprintln!("Unknown argument: {}", args[i]);
        std::process::exit(1);
      }
    }
    i += 1;
  }

  // Merge --external-css into patterns
  if external_css {
    external_patterns.push("*.css".to_string());
  }

  // Convert glob patterns to StringOrRegex for zero-overhead matching (no async Fn overhead).
  let external = if !external_patterns.is_empty() {
    let sor_patterns: Vec<StringOrRegex> = external_patterns
      .into_iter()
      .map(|pat| {
        if pat.contains('*') {
          // Convert glob to regex: *.css -> ^.*\.css$
          let regex_str = format!("^{}$", pat.replace('.', r"\.").replace('*', ".*"));
          StringOrRegex::Regex(HybridRegex::new(&regex_str).expect("invalid regex pattern"))
        } else {
          StringOrRegex::String(pat)
        }
      })
      .collect();
    Some(IsExternal::StringOrRegex(sor_patterns))
  } else {
    None
  };

  let define = if defines.is_empty() {
    None
  } else {
    let mut map = rolldown_utils::indexmap::FxIndexMap::default();
    for (k, v) in defines {
      map.insert(k, v);
    }
    Some(map)
  };

  let start = Instant::now();

  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem { name: Some("main".to_string()), import: entry }]),
    cwd,
    dir: Some(dir),
    format: Some(format),
    minify: if minify { Some(RawMinifyOptions::Bool(true)) } else { None },
    sourcemap: if sourcemap { Some(SourceMapType::File) } else { None },
    external,
    define,
    comments: if no_comments {
      Some(CommentsOptions { legal: false, annotation: false, jsdoc: false })
    } else {
      None
    },
    ..Default::default()
  })
  .expect("Failed to create bundler");

  let result = bundler.write().await;
  let elapsed = start.elapsed();

  match result {
    Ok(output) => {
      eprintln!(
        "Finished in {:.2} ms ({} assets)",
        elapsed.as_secs_f64() * 1000.0,
        output.assets.len()
      );
    }
    Err(e) => {
      eprintln!("Build failed: {e:?}");
      std::process::exit(1);
    }
  }
}
