use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rolldown::{
    Bundler, BundlerOptions, InputItem, IsExternal, OutputFormat, RawMinifyOptions, SourceMapType,
};
use rolldown_common::bundler_options::CommentsOptions;

#[tokio::main]
async fn main() {
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

    let external = if external_css {
        Some(IsExternal::Fn(Some(Arc::new(
            |specifier: &str,
             _importer: Option<&str>,
             _is_resolved: bool|
             -> std::pin::Pin<
                Box<dyn std::future::Future<Output = anyhow::Result<bool>> + Send + 'static>,
            > { let ends_with_css = specifier.ends_with(".css");
                Box::pin(async move { Ok(ends_with_css) })
            },
        ))))
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
        input: Some(vec![InputItem {
            name: Some("main".to_string()),
            import: entry,
        }]),
        cwd,
        dir: Some(dir),
        format: Some(format),
        minify: if minify {
            Some(RawMinifyOptions::Bool(true))
        } else {
            None
        },
        sourcemap: if sourcemap {
            Some(SourceMapType::File)
        } else {
            None
        },
        external,
        define,
        comments: if no_comments {
            Some(CommentsOptions::None)
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
