use arcstr::ArcStr;
use rolldown_common::{EmittedAsset, Output, OutputChunk};
use rolldown_utils::{concat_string, dashmap::FxDashMap};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;

use crate::{ModuleFederationPluginOption, utils::ResolvedRemoteModule};

#[allow(clippy::too_many_lines)]
pub async fn generate_manifest(
  ctx: &rolldown_plugin::PluginContext,
  args: &rolldown_plugin::HookGenerateBundleArgs<'_>,
  options: &ModuleFederationPluginOption,
  resolved_shared_modules: &FxDashMap<ArcStr, ResolvedRemoteModule>,
) -> anyhow::Result<()> {
  let chunks_map = args
    .bundle
    .iter()
    .filter_map(|output| {
      if let Output::Chunk(chunk) = output {
        Some((chunk.filename.clone(), chunk.as_ref()))
      } else {
        None
      }
    })
    .collect();
  let mut remote_to_sync_chunks = FxHashMap::default();
  let mut remote_to_async_chunks = FxHashMap::default();

  if let Some(expose) = options.exposes.as_ref() {
    for (key, value) in expose {
      let resolve_id = ctx.resolve(value, None, None).await??;
      collect_remote_chunks_relation(
        key,
        resolve_id.id.as_str(),
        &chunks_map,
        &mut remote_to_sync_chunks,
        &mut remote_to_async_chunks,
      );
    }
  }

  if let Some(shared) = options.shared.as_ref() {
    for key in shared.keys() {
      collect_remote_chunks_relation(
        key,
        resolved_shared_modules
          .get(key.as_str())
          .expect("shared module id already resolved")
          .id
          .as_str(),
        &chunks_map,
        &mut remote_to_sync_chunks,
        &mut remote_to_async_chunks,
      );
    }
  }

  let manifest = Manifest {
    id: options.name.clone(),
    name: options.name.clone(),
    meta_data: MetaData {
      name: options.name.clone(),
      r#type: "app".to_string(),
      build_info: BuildInfo {
        build_version: "1.0.0".to_string(),
        build_name: options.name.clone(),
      },
      remote_entry: RemoteEntry {
        name: options.filename.clone().expect("should have manifest filename"),
        r#type: "module".to_string(),
        ..Default::default()
      },
      ssr_remote_entry: RemoteEntry {
        name: options.filename.clone().expect("should have manifest filename"),
        r#type: "module".to_string(),
        ..Default::default()
      },
      types: MetaDataTypes::default(),
      global_name: options.name.clone(),
      plugin_version: String::new(),
      public_path: options.get_public_path.clone().unwrap_or_default(),
    },
    remotes: options
      .remotes
      .as_ref()
      .map(|r| {
        r.iter()
          .map(|remote| ManifestRemoteItem {
            federation_container_name: remote.entry.clone(),
            module_name: remote.name.clone(),
            alias: remote.name.clone(),
            entry: "*".to_string(),
          })
          .collect::<Vec<_>>()
      })
      .unwrap_or_default(),
    shared: options
      .shared
      .as_ref()
      .map(|shared| {
        shared
          .iter()
          .map(|(key, value)| RemoteModuleItem {
            id: concat_string!(options.name, ":", key),
            name: key.to_string(),
            version: resolved_shared_modules.get(key.as_str()).map_or_else(
              || value.version.clone().unwrap_or_default(),
              |v| v.value().version.as_deref().map(ToString::to_string).unwrap_or_default(),
            ),
            required_version: value.required_version.clone().unwrap_or_default(),
            assets: Assets {
              js: Asset {
                r#async: remote_to_async_chunks
                  .remove(key.as_str())
                  .map(|v| v.iter().map(ToString::to_string).collect::<Vec<_>>())
                  .unwrap_or_default(),
                sync: remote_to_sync_chunks
                  .remove(key.as_str())
                  .map(|v| v.iter().map(ToString::to_string).collect::<Vec<_>>())
                  .unwrap_or_default(),
              },
              css: Asset::default(),
            },
            ..Default::default()
          })
          .collect::<Vec<_>>()
      })
      .unwrap_or_default(),
    exposes: options
      .exposes
      .as_ref()
      .map(|exposes| {
        exposes
          .keys()
          .map(|key| {
            let format_key = key.replace("./", "");
            RemoteModuleItem {
              id: concat_string!(options.name, ":", format_key),
              name: format_key,
              path: key.to_string(),
              assets: Assets {
                js: Asset {
                  r#async: remote_to_async_chunks
                    .remove(key.as_str())
                    .map(|v| v.iter().map(ToString::to_string).collect::<Vec<_>>())
                    .unwrap_or_default(),
                  sync: remote_to_sync_chunks
                    .remove(key.as_str())
                    .map(|v| v.iter().map(ToString::to_string).collect::<Vec<_>>())
                    .unwrap_or_default(),
                },
                css: Asset::default(),
              },
              ..Default::default()
            }
          })
          .collect::<Vec<_>>()
      })
      .unwrap_or_default(),
  };

  ctx
    .emit_file_async(EmittedAsset {
      file_name: Some(
        options
          .manifest
          .as_ref()
          .expect("should have manifest option")
          .normalize_file_name()
          .into(),
      ),
      name: None,
      original_file_name: None,
      source: (serde_json::to_string_pretty(&manifest)
        .expect("should success serialize manifest json"))
      .into(),
    })
    .await?;

  Ok(())
}

fn collect_sync_chunks(
  chunks_map: &FxHashMap<ArcStr, &OutputChunk>,
  filename: &ArcStr,
  sync_chunks: &mut FxHashSet<ArcStr>,
) {
  if let Some(chunk) = chunks_map.get(filename) {
    sync_chunks.extend(chunk.imports.clone());
    for import in &chunk.imports {
      if !sync_chunks.contains(import) {
        collect_sync_chunks(chunks_map, import, sync_chunks);
      }
    }
  }
}

fn collect_remote_chunks_relation<'a>(
  remote_key: &'a str,
  module_id: &str,
  chunks_map: &FxHashMap<ArcStr, &OutputChunk>,
  remote_to_sync_chunks: &mut FxHashMap<&'a str, FxHashSet<ArcStr>>,
  remote_to_async_chunks: &mut FxHashMap<&'a str, Vec<ArcStr>>,
) {
  for (filename, chunk) in chunks_map {
    if let Some(facade_module_id) = &chunk.facade_module_id {
      if facade_module_id.as_ref() == module_id {
        let mut sync_chunks = FxHashSet::from_iter(vec![filename.clone()]);
        collect_sync_chunks(chunks_map, filename, &mut sync_chunks);
        remote_to_sync_chunks.insert(remote_key, sync_chunks);
        remote_to_async_chunks.insert(remote_key, chunk.dynamic_imports.clone());
      }
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
  pub id: String,
  pub name: String,
  pub meta_data: MetaData,
  pub shared: Vec<RemoteModuleItem>,
  pub remotes: Vec<ManifestRemoteItem>,
  pub exposes: Vec<RemoteModuleItem>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaData {
  pub name: String,
  pub r#type: String, // 'app'
  pub build_info: BuildInfo,
  pub remote_entry: RemoteEntry,
  pub ssr_remote_entry: RemoteEntry,
  pub types: MetaDataTypes,
  pub global_name: String,
  pub plugin_version: String,
  pub public_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildInfo {
  pub build_version: String,
  pub build_name: String,
}

#[derive(Default, Serialize)]
struct MetaDataTypes {
  pub path: String,
  pub name: String,
}

#[derive(Default, Serialize)]
struct RemoteEntry {
  pub name: String,
  pub path: String,
  pub r#type: String, // 'module'
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteModuleItem {
  pub id: String,
  pub name: String,
  pub version: String,
  pub required_version: String,
  pub path: String,
  pub assets: Assets,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRemoteItem {
  pub federation_container_name: String,
  pub module_name: String,
  pub alias: String,
  pub entry: String,
}

#[derive(Default, Serialize)]
struct Asset {
  pub r#async: Vec<String>,
  pub sync: Vec<String>,
}

#[derive(Default, Serialize)]
struct Assets {
  pub js: Asset,
  pub css: Asset,
}
