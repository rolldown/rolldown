use regex::Regex;
use rolldown_fs::{FileSystem, OsFileSystem};
use rolldown_utils::path_ext::PathExt;
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

use import_map::{parse_from_json_with_options, ImportMapOptions};

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum ModuleInfo {
  Typed {
    #[serde(flatten)]
    details: TypedModuleDetails,
  },
  Error {
    specifier: String,
    error: String,
  },
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
enum TypedModuleDetails {
  #[serde(rename = "asserted")]
  Asserted {
    specifier: String,
    local: String,
    #[serde(rename = "mediaType")]
    media_type: DenoMediaType,
  },
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
  #[serde(rename = "node")]
  Node { specifier: String },
}

#[derive(Deserialize, Debug, Clone)]
struct NpmPackageInfo {
  name: String,
  // version: String,
  // #[serde(rename = "registryUrl")]
  // registry_url: String,
}

#[derive(Deserialize, Debug, Clone)]
// cspell:ignore Dmts, Dcts
enum DenoMediaType {
  JavaScript,
  Mjs,
  Cjs,
  JSX,
  TypeScript,
  Mts,
  Cts,
  Dts,
  Dmts,
  Dcts,
  TSX,
  Json,
  Wasm,
  TsBuildInfo,
  SourceMap,
  Unknown,
}

impl From<&DenoMediaType> for ModuleType {
  fn from(media_type: &DenoMediaType) -> Self {
    match media_type {
      DenoMediaType::JavaScript | DenoMediaType::Mjs | DenoMediaType::Cjs => ModuleType::Js,
      DenoMediaType::JSX => ModuleType::Jsx,
      DenoMediaType::TypeScript
      | DenoMediaType::Mts
      | DenoMediaType::Cts
      | DenoMediaType::Dts
      | DenoMediaType::Dmts
      | DenoMediaType::Dcts => ModuleType::Ts,
      DenoMediaType::TSX => ModuleType::Tsx,
      DenoMediaType::Json => ModuleType::Json,
      DenoMediaType::Wasm => ModuleType::Binary,
      DenoMediaType::TsBuildInfo | DenoMediaType::SourceMap => ModuleType::Text,
      DenoMediaType::Unknown => ModuleType::Empty,
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
struct DenoInfoJsonV1 {
  redirects: HashMap<String, String>,
  modules: Vec<ModuleInfo>,
  #[serde(rename = "npmPackages")]
  npm_packages: HashMap<String, NpmPackageInfo>,
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
  npm_package: Option<String>,
  local_path: Option<String>,
  redirected: String,
  module_type: Option<ModuleType>,
}

#[derive(Debug)]
pub struct DenoLoaderPlugin {
  resolve_cache: Mutex<HashMap<String, DenoResolveResult>>,
  pub import_map: String,
  pub import_map_base_url: String,
}

impl Default for DenoLoaderPlugin {
  fn default() -> Self {
    Self::new(r#"{}"#.to_string(), "file://".to_string())
  }
}

fn extract_path_from_specifier(specifier: &str) -> Option<String> {
  let re: Regex = Regex::new(r"(?:[^:]+:/?)?(?:@[^/]+/)?[^/@]+(?:@[^/]*)?(?:/(.+))?").unwrap();
  re.captures(specifier).and_then(|caps| caps.get(1).map(|m| m.as_str().to_string()))
}

impl DenoLoaderPlugin {
  pub fn new(import_map: String, import_map_base_url: String) -> Self {
    Self { resolve_cache: Mutex::new(HashMap::new()), import_map, import_map_base_url }
  }

  fn get_cached_info(&self, specifier: &str) -> Result<DenoResolveResult, &'static str> {
    let mut cache = self.resolve_cache.lock().unwrap();
    if let Some(cached) = cache.get(specifier).cloned() {
      return Ok(cached);
    }
    let info: DenoInfoJsonV1 = get_deno_info(specifier)?;
    for module in &info.modules {
      match module {
        ModuleInfo::Typed { details, .. } => match details {
          TypedModuleDetails::Node { specifier: _s } => {}
          TypedModuleDetails::Asserted { media_type, specifier: s, local }
          | TypedModuleDetails::Esm { media_type, specifier: s, local } => {
            let result = DenoResolveResult {
              local_path: Some(local.clone()),
              redirected: s.clone(),
              npm_package: None,
              module_type: Some(media_type.into()),
            };
            cache.insert(s.clone(), result.clone());
            for (key, value) in &info.redirects {
              if value == s {
                cache.insert(key.clone(), result.clone());
              }
            }
          }
          TypedModuleDetails::Npm { specifier: s, npm_package } => {
            let npm_package_base = info.npm_packages.get(npm_package).map(|pkg| pkg.name.clone());
            let npm_package_path = extract_path_from_specifier(specifier);
            let npm_package = match (npm_package_base, npm_package_path) {
              (Some(base), Some(path)) => format!("{}/{}", base, path),
              (Some(base), None) => base,
              _ => npm_package.to_string(),
            };
            let result = DenoResolveResult {
              local_path: None,
              redirected: s.clone(),
              npm_package: Some(npm_package),
              module_type: None,
            };
            cache.insert(s.clone(), result.clone());
            for (key, value) in &info.redirects {
              if value == s {
                cache.insert(key.clone(), result.clone());
              }
            }
          }
        },
        ModuleInfo::Error { specifier: s, error } => {
          eprintln!("Error for specifier {}: {}", s, error);
          // Optionally, you could return an error here instead of continuing
          // return Err("Error in module resolution");
        }
      }
    }

    cache.get(specifier).cloned().ok_or("Specifier not found in cache after processing")
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
      let id = if args.specifier.starts_with(".") || args.specifier.starts_with("/") {
        args
          .importer
          .and_then(|importer| url::Url::parse(importer).ok())
          .and_then(|base_url| base_url.join(&args.specifier).ok())
          .map(|url| if url.scheme() == "file" { url.path().to_string() } else { url.to_string() })
          .unwrap_or_else(|| args.specifier.to_string())
      } else {
        args.specifier.to_string()
      };

      let maybe_resolved = if id.starts_with(".") || id.starts_with("/") {
        id.to_string()
      } else {
        let import_map_base_url =
          url::Url::parse(&self.import_map_base_url).expect("is not an url");
        let import_map = parse_from_json_with_options(
          import_map_base_url.clone(),
          &self.import_map,
          ImportMapOptions { expand_imports: true, ..Default::default() },
        )
        .unwrap()
        .import_map;

        import_map
          .resolve(&id, &import_map_base_url)
          .ok()
          .map(|url| url.to_string())
          .unwrap_or_else(|| id.to_string())
      };

      if maybe_resolved.starts_with("node:") {
        return Ok(Some(HookResolveIdOutput {
          id: maybe_resolved,
          external: Some(true),
          ..Default::default()
        }));
      } else if maybe_resolved.starts_with("file:") {
        let final_id = url::Url::parse(&maybe_resolved)
          .map(|url| url.to_file_path().expect("error"))
          .expect("error")
          .as_path()
          .expect_to_str()
          .to_string();

        return Ok(Some(HookResolveIdOutput {
          id: final_id,
          external: Some(false),
          ..Default::default()
        }));
      } else if maybe_resolved.starts_with("jsr:") {
        let cached: DenoResolveResult = self.get_cached_info(&maybe_resolved).expect("info failed");

        return Ok(Some(HookResolveIdOutput {
          id: cached.redirected,
          external: Some(false),
          ..Default::default()
        }));
      } else if maybe_resolved.starts_with("npm:") {
        let cached: DenoResolveResult = self.get_cached_info(&maybe_resolved).expect("info failed");

        if let Some(npm_package) = cached.npm_package {
          return Ok(
            ctx
              .resolve(
                &npm_package,
                None,
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
        let content = String::from_utf8_lossy(
          &OsFileSystem::read(&OsFileSystem, Path::new(&local_path)).expect("cant read local path"),
        )
        .into_owned();
        let (code, module_type) = if cached.module_type.as_ref() == Some(&ModuleType::Json) {
          (format!("export default {}", content), Some(ModuleType::Js))
        } else {
          (content, cached.module_type)
        };

        Ok(Some(HookLoadOutput { code, module_type, ..Default::default() }))
      } else {
        Ok(None)
      }
    }
  }
}
