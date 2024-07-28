use std::sync::Arc;

use rolldown::{Bundler, BundlerOptions, InputItem, ModuleType, SourceMapType};
use rolldown_plugin::{HookResolveIdOutput, Plugin};
use rolldown_testing::workspace;
use sugar_path::SugarPath;
use url::Url;

// cargo run --example deno_bundle

#[derive(Debug)]
struct HttpImportPlugin;

impl Plugin for HttpImportPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "http-import".into()
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    match args.importer {
      Some(importer) if importer.starts_with("http") => {
        let resolved = Url::parse(importer)?.join(args.specifier)?;
        return Ok(Some(HookResolveIdOutput { id: resolved.to_string(), ..Default::default() }));
      }
      None => {
        if args.specifier.starts_with("http") {
          return Ok(Some(HookResolveIdOutput {
            id: args.specifier.to_string(),
            ..Default::default()
          }));
        }
      }
      _ => {}
    }

    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id.starts_with("http") {
      let content = reqwest::get(args.id).await?.text().await?;
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: content,
        module_type: Some(ModuleType::Ts),
        ..Default::default()
      }));
    }

    Ok(None)
  }
}

#[tokio::main]
async fn main() {
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("text".to_string()),
        import: "https://deno.land/std@0.224.0/text/mod.ts".to_string(),
      }]),
      entry_filenames: Some("[name].bundle.js".to_string()),
      cwd: Some(workspace::crate_dir("rolldown").join("./examples").normalize()),
      sourcemap: Some(SourceMapType::File),
      ..Default::default()
    },
    vec![Arc::new(HttpImportPlugin)],
  );

  let result = bundler.write().await.unwrap();

  for err in result.errors {
    eprintln!("{}", err.into_diagnostic());
  }
}
