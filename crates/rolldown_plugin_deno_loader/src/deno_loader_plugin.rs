use rolldown_fs::{FileSystem, OsFileSystem};
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext, PluginContextResolveOptions,
};

use import_map::parse_from_json;

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
enum ModuleInfo {
  #[serde(rename = "esm")]
  Esm {
    local: String,
    specifier: String,
    #[serde(rename = "mediaType")]
    media_type: DenoMediaType,
  },
  #[serde(rename = "npm")]
  Npm {
    specifier: String,
    #[serde(rename = "npmPackage")]
    npm_package: String,
  },
}

#[derive(Deserialize, Debug, Clone)]
// cspell:ignore Dmts
enum DenoMediaType {
  TypeScript,
  Tsx,
  JavaScript,
  Jsx,
  Json,
  Dmts,
  Mjs,
}

#[derive(Deserialize, Debug, Clone)]
struct DenoInfoJsonV1 {
  redirects: HashMap<String, String>,
  modules: Vec<ModuleInfo>,
}

fn follow_redirects(
  initial: &str,
  redirects: &HashMap<String, String>,
) -> Result<String, &'static str> {
  let mut current = initial.to_string();
  let mut seen = std::collections::HashSet::new();

  while let Some(next) = redirects.get(&current) {
    if !seen.insert(current.clone()) {
      return Err("Circular redirect detected");
    }
    current = next.clone();
  }

  Ok(current)
}

fn get_deno_info(specifier: &str) -> Result<DenoInfoJsonV1, &'static str> {
  let output = std::process::Command::new("deno")
    .args(["info", "--json", specifier])
    .output()
    .expect("Failed to execute deno info command");

  if !output.status.success() {
    return Err("deno info command failed");
  }

  Ok(serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output"))
}

#[derive(Debug, Clone)]
struct DenoResolveResult {
  info: DenoInfoJsonV1,
  local_path: Option<String>,
  redirected: String,
}

#[derive(Debug)]
pub struct DenoLoaderPlugin {
  resolve_cache: Mutex<HashMap<String, DenoResolveResult>>,
  pub import_map_string: String,
}

impl Default for DenoLoaderPlugin {
  fn default() -> Self {
    Self::new(r#"{}"#.to_string())
  }
}

impl DenoLoaderPlugin {
  pub fn new(import_map_string: String) -> Self {
    Self { resolve_cache: Mutex::new(HashMap::new()), import_map_string }
  }

  fn get_cached_info(&self, specifier: &str) -> Result<DenoResolveResult, &'static str> {
    if let Some(cached) = self.resolve_cache.lock().unwrap().get(specifier).cloned() {
      return Ok(cached);
    }

    let info = get_deno_info(specifier)?;
    let redirected = follow_redirects(specifier, &info.redirects)?;
    let local_path = info.modules.iter().find_map(|m| match m {
      ModuleInfo::Esm { specifier: s, local, .. } if s == &redirected => Some(local.clone()),
      _ => None,
    });

    let result = DenoResolveResult { info, local_path, redirected };
    self.resolve_cache.lock().unwrap().insert(specifier.to_string(), result.clone());
    Ok(result)
  }
}

impl Plugin for DenoLoaderPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:deno-loader")
  }

  fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> impl std::future::Future<Output = HookResolveIdReturn> {
    async {
      let id = if args.specifier.starts_with('.') {
        args
          .importer
          .and_then(|importer| url::Url::parse(importer).ok())
          .and_then(|base_url| base_url.join(&args.specifier).ok())
          .map(|joined_url| {
            if joined_url.scheme() == "file" {
              joined_url.path().to_string()
            } else {
              joined_url.to_string()
            }
          })
          .unwrap_or_else(|| args.specifier.to_string())
      } else {
        args.specifier.to_string()
      };

      let base_url = ctx
        .cwd()
        .to_str()
        .and_then(|s| url::Url::from_file_path(s).ok())
        .unwrap_or_else(|| url::Url::parse("file:///").unwrap());

      let import_map =
        parse_from_json(base_url.clone(), &self.import_map_string).unwrap().import_map;

      let maybe_resolved = import_map
        .resolve(&id, &base_url)
        .ok()
        .map(|url| url.to_string())
        .unwrap_or_else(|| id.to_string());

      if maybe_resolved.starts_with("jsr:") {
        let cached: DenoResolveResult = self.get_cached_info(&maybe_resolved).expect("info failed");

        return Ok(Some(HookResolveIdOutput {
          id: cached.redirected,
          external: Some(false),
          ..Default::default()
        }));
      } else if maybe_resolved.starts_with("npm:") {
        let cached: DenoResolveResult = self.get_cached_info(&maybe_resolved).expect("info failed");

        if let Some(ModuleInfo::Npm { npm_package, .. }) = cached.info.modules.into_iter().find(
          |m| matches!(m, ModuleInfo::Npm { specifier, .. } if specifier == &cached.redirected),
        ) {
          let package_name = npm_package.split('@').next().unwrap_or(&npm_package).to_string();
          return Ok(
            ctx
              .resolve(
                &package_name,
                args.importer,
                Some(PluginContextResolveOptions {
                  import_kind: args.kind,
                  skip_self: true,
                  custom: Arc::clone(&args.custom),
                }),
              )
              .await?
              .map(|resolved_id| {
                Some(HookResolveIdOutput { id: resolved_id.id.to_string(), ..Default::default() })
              })?,
          );
        }
      } else if maybe_resolved.starts_with("http:") || maybe_resolved.starts_with("https:") {
        return Ok(Some(HookResolveIdOutput {
          id: maybe_resolved.to_string(),
          external: Some(false),
          ..Default::default()
        }));
      }

      Ok(None)
    }
  }

  fn load(
    &self,
    _ctx: &PluginContext,
    args: &HookLoadArgs<'_>,
  ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
    async {
      if args.id.starts_with("jsr:")
        || args.id.starts_with("http:")
        || args.id.starts_with("https:")
      {
        let cached = self.get_cached_info(args.id).expect("info failed");
        let local_path = cached.local_path.expect("local path not found");
        // Return the specifier as the id to tell rolldown that this data url is handled by the plugin. Don't fallback to
        // the default resolve behavior and mark it as external.
        Ok(Some(HookLoadOutput {
          code: String::from_utf8_lossy(
            &OsFileSystem::read(&OsFileSystem, Path::new(&local_path))
              .expect("cant read local path"),
          )
          .into_owned(),
          module_type: Some(ModuleType::Tsx),
          ..Default::default()
        }))
      } else {
        Ok(None)
      }
    }
  }
}
