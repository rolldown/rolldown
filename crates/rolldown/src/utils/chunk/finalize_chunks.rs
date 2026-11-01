use std::{borrow::Cow, hash::Hash, mem, sync::Arc};

use arcstr::ArcStr;
use itertools::Itertools;
use oxc_index::IndexVec;
use rolldown_common::{
  Asset, ChunkIdx, HashCharacters, InsChunkIdx, InstantiationKind, NormalizedBundlerOptions,
  PathsOutputOption, SourceMapType, StrOrBytes,
};
use rolldown_error::BuildResult;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  base64::to_url_safe_base64,
  concat_string,
  hash_placeholder::{
    HASH_PLACEHOLDER_LEFT_FINDER, extract_hash_placeholders, replace_placeholder_with_hash,
    visit_with_placeholders_defaulted,
  },
  indexmap::FxIndexSet,
  rayon::{
    IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
  },
  rustc_hash::FxHashSetExt,
  xxhash::{encode_hash_with_base, xxhash_base64_url},
};
use rustc_hash::{FxHashMap, FxHashSet};
use xxhash_rust::xxh3::Xxh3;

use crate::{
  chunk_graph::ChunkGraph,
  stages::link_stage::LinkStageOutput,
  type_alias::{AssetVec, IndexChunkToInstances, IndexInstantiatedChunks},
  types::scan_stage_cache::{CachedChunkFinalize, CachedMultiModuleChunk},
  utils::process_code_and_sourcemap::prepare_sourcemap,
};

/// A chunk that instantiating rendered with a multi-module chunk key, whose finalize results the
/// cache entry under that key records.
struct CachedChunkState<'a> {
  key: [u128; 2],
  /// The code finalizing received, which `previous` was derived from when it is set.
  code: Arc<str>,
  placeholders: Arc<[String]>,
  previous: Option<&'a CachedChunkFinalize>,
}

/// Finds the cached record of a chunk's code, if instantiating rendered it with a key. Render
/// hooks and minifying may rewrite chunk code, so the record applies only when the code finalizing
/// received is the code it was derived from; otherwise the chunk starts a new record.
fn cached_chunk_state<'a>(
  ins_chunk: &rolldown_common::InstantiatedChunk,
  multi_module_chunks: Option<&FxHashMap<ChunkIdx, [u128; 2]>>,
  cached: Option<&'a FxHashMap<[u128; 2], CachedMultiModuleChunk>>,
) -> Option<CachedChunkState<'a>> {
  let key = *multi_module_chunks?.get(&ins_chunk.originate_from)?;
  let entry = cached?.get(&key)?;
  let StrOrBytes::Str(code) = &ins_chunk.content else {
    return None;
  };
  let previous = entry.finalized.as_ref().filter(|finalized| *finalized.code == **code);
  Some(match previous {
    Some(finalized) => CachedChunkState {
      key,
      code: Arc::clone(&finalized.code),
      placeholders: Arc::clone(&finalized.placeholders),
      previous,
    },
    None => CachedChunkState {
      key,
      code: if *entry.content == **code {
        Arc::clone(&entry.content)
      } else {
        Arc::from(code.as_str())
      },
      placeholders: extract_hash_placeholders(code, &HASH_PLACEHOLDER_LEFT_FINDER)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>()
        .into(),
      previous: None,
    },
  })
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[tracing::instrument(level = "debug", skip_all)]
pub async fn finalize_assets(
  chunk_graph: &ChunkGraph,
  link_output: &LinkStageOutput,
  index_instantiated_chunks: IndexInstantiatedChunks,
  index_chunk_to_instances: &IndexChunkToInstances,
  hash_characters: HashCharacters,
  options: &NormalizedBundlerOptions,
  resolved_paths: Option<&PathsOutputOption>,
  multi_module_chunks: Option<&FxHashMap<ChunkIdx, [u128; 2]>>,
  mut cache: Option<&mut crate::types::scan_stage_cache::ScanStageCache>,
) -> BuildResult<AssetVec> {
  let ins_chunk_idx_by_placeholder = index_instantiated_chunks
    .iter_enumerated()
    .filter_map(|(ins_chunk_idx, ins_chunk)| {
      ins_chunk.preliminary_filename.hash_placeholder().map(move |placeholders| {
        placeholders.iter().map(move |hash_placeholder| (hash_placeholder.as_str(), ins_chunk_idx))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let cached_chunks = cache.as_deref().map(|c| &c.cached_multi_module_chunks);
  let index_cached_chunks: IndexVec<InsChunkIdx, Option<CachedChunkState<'_>>> =
    index_instantiated_chunks
      .par_iter()
      .map(|ins_chunk| cached_chunk_state(ins_chunk, multi_module_chunks, cached_chunks))
      .collect::<Vec<_>>()
      .into();
  let mut content_hash_updates: Vec<([u128; 2], Vec<bool>, String)> = Vec::new();

  let hash_base = hash_characters.base();
  let needs_chunk_hashes = !ins_chunk_idx_by_placeholder.is_empty()
    || options.sourcemap_debug_ids
    || index_instantiated_chunks.iter().any(|c| c.augment_chunk_hash.is_some());
  let mut index_final_hashes: IndexVec<InsChunkIdx, (String, u128)> = if needs_chunk_hashes {
    let index_direct_dependencies: IndexVec<InsChunkIdx, Vec<InsChunkIdx>> =
    index_instantiated_chunks
      .par_iter()
      .enumerate()
      .map(|(asset_idx, asset)| {
        if let Some(cached) = &index_cached_chunks[InsChunkIdx::from(asset_idx)] {
          cached
            .placeholders
            .iter()
            .filter_map(|placeholder| {
              ins_chunk_idx_by_placeholder.get(placeholder.as_str()).copied()
            })
            .collect_vec()
        } else if let StrOrBytes::Str(content) = &asset.content {
          extract_hash_placeholders(content, &HASH_PLACEHOLDER_LEFT_FINDER)
            .iter()
            .filter_map(|placeholder| ins_chunk_idx_by_placeholder.get(placeholder).copied())
            .collect_vec()
        } else {
          vec![]
        }
      })
      .collect::<Vec<_>>()
      .into();

    // Normalize rolldown-generated placeholders before hashing so the hash is stable across
    // their (transient) index drift. See `internal-docs/chunk-hash/implementation.md` for the full reasoning.
    let hash_str_content = |content: &str| {
      let mut hasher = Xxh3::default();
      visit_with_placeholders_defaulted(
        content,
        &HASH_PLACEHOLDER_LEFT_FINDER,
        |placeholder| ins_chunk_idx_by_placeholder.contains_key(placeholder),
        |bytes| hasher.update(bytes),
      );
      to_url_safe_base64(hasher.digest128().to_le_bytes())
    };
    let (standalone_hashes, hash_updates): (Vec<String>, Vec<_>) = index_instantiated_chunks
    .par_iter()
    .enumerate()
    .map(|(asset_idx, chunk)| {
      let mut update = None;
      let mut hash = if let Some(cached) = &index_cached_chunks[InsChunkIdx::from(asset_idx)] {
        // The hash depends on the code and on which of its placeholders name chunks of this build.
        let known = cached
          .placeholders
          .iter()
          .map(|placeholder| ins_chunk_idx_by_placeholder.contains_key(placeholder.as_str()))
          .collect::<Vec<_>>();
        match cached.previous.and_then(|previous| previous.content_hash.as_ref()) {
          Some((cached_known, hash)) if *cached_known == known => hash.clone(),
          _ => {
            let hash = hash_str_content(&cached.code);
            update = Some((cached.key, known, hash.clone()));
            hash
          }
        }
      } else {
        match &chunk.content {
          StrOrBytes::Str(content) => hash_str_content(content),
          StrOrBytes::Bytes(_) => xxhash_base64_url(chunk.content.as_bytes()),
        }
      };
      if let Some(augment_chunk_hash) = &chunk.augment_chunk_hash {
        hash.push_str(augment_chunk_hash);
        hash = xxhash_base64_url(hash.as_bytes());
      }
      (hash, update)
    })
    .unzip();
    content_hash_updates.extend(hash_updates.into_iter().flatten());
    let index_standalone_content_hashes: IndexVec<InsChunkIdx, String> = standalone_hashes.into();

    let mut hashes: IndexVec<InsChunkIdx, (String, u128)> = (0..index_instantiated_chunks.len())
    .into_par_iter()
    .map(|asset_idx| {
      let mut hasher = Xxh3::default();
      let asset_idx = InsChunkIdx::from(asset_idx);
      index_standalone_content_hashes[asset_idx].hash(&mut hasher);

      // Calculate this asset's transitive closure only for the duration of its hash. Retaining
      // every closure at once makes memory quadratic for output graphs with many shared chunks.
      let dependencies = collect_transitive_dependencies(asset_idx, &index_direct_dependencies);
      dependencies.into_iter().for_each(|dep_id| {
        index_standalone_content_hashes[dep_id].hash(&mut hasher);
      });

      let digested = hasher.digest128();
      (encode_hash_with_base(&digested.to_le_bytes(), hash_base), digested)
    })
    .collect::<Vec<_>>()
    .into();

    deconflict_filenames(&index_instantiated_chunks, &mut hashes, hash_base);
    hashes
  } else {
    IndexVec::from_vec(vec![(String::new(), 0); index_instantiated_chunks.len()])
  };
  let _ = &mut index_final_hashes;

  let chunk_hashes_by_placeholder = index_final_hashes
    .iter_enumerated()
    .filter_map(|(idx, (hash, _))| {
      index_instantiated_chunks[idx].preliminary_filename.hash_placeholder().map(|placeholders| {
        placeholders.iter().map(|placeholder| (placeholder.clone(), &hash[..placeholder.len()]))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let index_sourcemap_hash_placeholders: IndexVec<InsChunkIdx, Option<Vec<String>>> =
    index_instantiated_chunks
      .iter()
      .map(|chunk| {
        chunk
          .preliminary_sourcemap_filename
          .as_ref()
          .and_then(|filename| filename.hash_placeholder())
          .map(<[String]>::to_vec)
      })
      .collect::<Vec<_>>()
      .into();

  let mut assets: AssetVec = index_instantiated_chunks
    .into_par_iter()
    .enumerate()
    .map(|(asset_idx, mut instantiated_chunk)| {
      let asset_idx = InsChunkIdx::from(asset_idx);

      let filename: ArcStr = if chunk_hashes_by_placeholder.is_empty() {
        instantiated_chunk.preliminary_filename.as_str().into()
      } else {
        replace_placeholder_with_hash(
          instantiated_chunk.preliminary_filename.as_str(),
          &chunk_hashes_by_placeholder,
          &HASH_PLACEHOLDER_LEFT_FINDER,
        )
        .into()
      };

      // Only chunks that actually produced a map report a sourcemap filename; Rollup reports
      // `sourcemapFileName: null` otherwise.
      let preliminary_filename_str = if instantiated_chunk.map.is_some() {
        instantiated_chunk.preliminary_sourcemap_filename.as_ref().map(|f| f.as_str())
      } else {
        None
      };

      if let InstantiationKind::Ecma(ecma_meta) = &mut instantiated_chunk.kind {
        let (_, debug_id) = index_final_hashes[asset_idx];
        ecma_meta.debug_id = debug_id;
        ecma_meta.sourcemap_filename = preliminary_filename_str.map(|str| {
          if chunk_hashes_by_placeholder.is_empty() {
            str.into()
          } else {
            replace_placeholder_with_hash(
              str,
              &chunk_hashes_by_placeholder,
              &HASH_PLACEHOLDER_LEFT_FINDER,
            )
            .into()
          }
        });
      }
      if let Some(cached) = &index_cached_chunks[asset_idx] {
        if !cached.placeholders.is_empty() && !chunk_hashes_by_placeholder.is_empty() {
          if let Cow::Owned(replaced) = replace_placeholder_with_hash(
            &cached.code,
            &chunk_hashes_by_placeholder,
            &HASH_PLACEHOLDER_LEFT_FINDER,
          ) {
            instantiated_chunk.content = StrOrBytes::Str(replaced);
          }
        }
      } else if !chunk_hashes_by_placeholder.is_empty()
        && let StrOrBytes::Str(content) = &mut instantiated_chunk.content
      {
        if let Cow::Owned(replaced) = replace_placeholder_with_hash(
          content,
          &chunk_hashes_by_placeholder,
          &HASH_PLACEHOLDER_LEFT_FINDER,
        ) {
          *content = replaced;
        }
      }

      instantiated_chunk.finalize(filename)
    })
    .collect();

  let new_records: Vec<_> = index_cached_chunks
    .into_iter()
    .flatten()
    .filter(|cached| cached.previous.is_none())
    .map(|cached| (cached.key, cached.code, cached.placeholders))
    .collect();
  if let Some(c) = cache.as_deref_mut() {
    let chunks = &mut c.cached_multi_module_chunks;
    for (key, code, placeholders) in new_records {
      if let Some(entry) = chunks.get_mut(&key) {
        entry.finalized =
          Some(CachedChunkFinalize { code, placeholders, content_hash: None });
      }
    }
    for (key, known, hash) in content_hash_updates {
      if let Some(finalized) = chunks.get_mut(&key).and_then(|entry| entry.finalized.as_mut()) {
        finalized.content_hash = Some((known, hash));
      }
    }
  }

  let index_ins_chunk_to_filename: IndexVec<InsChunkIdx, ArcStr> =
    assets.iter().map(|ins_chunk| ins_chunk.filename.clone()).collect::<Vec<_>>().into();

  assets.par_iter_mut().for_each(|ins_chunk| {
    if let (InstantiationKind::Ecma(ecma_meta), Some(originate_from)) =
      (&mut ins_chunk.meta, ins_chunk.originate_from)
    {
      let chunk = &chunk_graph.chunk_table[originate_from];
      ecma_meta.imports = chunk
        .cross_chunk_imports
        .iter()
        .flat_map(|importee_idx| {
          let instances = &index_chunk_to_instances[*importee_idx];
          if instances.is_empty() && chunk_hashes_by_placeholder.is_empty() {
            chunk_graph.chunk_table[*importee_idx]
              .preliminary_filename
              .as_ref()
              .map(|f| ArcStr::from(f.as_str()))
              .into_iter()
              .collect::<Vec<_>>()
          } else {
            instances
              .iter()
              .map(|importee_asset_idx| index_ins_chunk_to_filename[*importee_asset_idx].clone())
              .collect::<Vec<_>>()
          }
        })
        .chain(super::static_external_imports(chunk, &link_output.module_table).map(|idx| {
          link_output.module_table[idx]
            .as_external()
            .expect("static external imports should only contain external modules")
            .get_file_name(resolved_paths)
        }))
        .collect();

      ecma_meta.dynamic_imports = chunk
        .cross_chunk_dynamic_imports
        .iter()
        .flat_map(|importee_idx| {
          let instances = &index_chunk_to_instances[*importee_idx];
          if instances.is_empty() && chunk_hashes_by_placeholder.is_empty() {
            chunk_graph.chunk_table[*importee_idx]
              .preliminary_filename
              .as_ref()
              .map(|f| ArcStr::from(f.as_str()))
              .into_iter()
              .collect::<Vec<_>>()
          } else {
            instances
              .iter()
              .map(|importee_asset_idx| index_ins_chunk_to_filename[*importee_asset_idx].clone())
              .collect::<Vec<_>>()
          }
        })
        .chain(chunk.dynamic_imports_from_external_modules.iter().map(|idx| {
          link_output.module_table[*idx]
            .as_external()
            .expect("dynamic_imports_from_external_modules should only contain external modules")
            .get_file_name(resolved_paths)
        }))
        .collect();
    }
  });

  finalize_sourcemaps(&mut assets, &index_sourcemap_hash_placeholders, options, hash_base, cache)
    .await?;

  Ok(assets)
}

async fn finalize_sourcemaps(
  assets: &mut AssetVec,
  index_sourcemap_hash_placeholders: &IndexVec<InsChunkIdx, Option<Vec<String>>>,
  options: &NormalizedBundlerOptions,
  hash_base: u8,
  mut cache: Option<&mut crate::types::scan_stage_cache::ScanStageCache>,
) -> BuildResult<()> {
  let can_cache_maps = matches!(
    options.sourcemap,
    Some(SourceMapType::File | SourceMapType::Hidden)
  ) && !options.sourcemap_debug_ids;
  let content_filter = cache
    .as_ref()
    .and_then(|c| c.sourcemap_content_filter.as_ref().map(|h| Arc::clone(&h.0)));

  let cached_hits: Vec<Option<( [u128; 2], Option<crate::types::scan_stage_cache::CachedChunkSourcemap> )>> =
    assets
      .iter()
      .enumerate()
      .map(|(_idx, asset)| {
        if !can_cache_maps {
          return None;
        }
        let chunk_idx = asset.originate_from?;
        let c = cache.as_ref()?;
        let &digest = c.current_chunk_map_digests.get(&chunk_idx)?;
        let hit = c.cached_chunk_sourcemaps.get(&digest).cloned();
        Some((digest, hit))
      })
      .collect();

  let can_parallel_prepare = !matches!(
    options.sourcemap_ignore_list,
    Some(rolldown_common::SourceMapIgnoreList::Fn(_))
  );
  let mut pre_redacted_flags = vec![false; assets.len()];
  if can_parallel_prepare {
    let filter_ref = content_filter.as_ref();
    let prep_results: Vec<anyhow::Result<bool>> = assets
      .par_iter_mut()
      .zip(cached_hits.par_iter())
      .map(|(asset, cached_state)| -> anyhow::Result<bool> {
        let filename = asset.filename.clone();
        if let (InstantiationKind::Ecma(ecma_meta), Some(map)) =
          (&mut asset.meta, asset.map.as_mut())
        {
          let file_path = options.cwd.as_path().join(&options.out_dir).join(filename.as_str());
          ecma_meta.file_dir =
            file_path.parent().expect("chunk file name should have a parent").to_path_buf();
          if let Some((_, Some(hit))) = cached_state {
            return Ok(hit.pre_redacted);
          }
          crate::utils::process_code_and_sourcemap::poll_ready_or_block_on(prepare_sourcemap(
            options,
            map,
            &ecma_meta.file_dir,
            filename.as_str(),
          ))?;
          if let Some(filter) = filter_ref {
            return map.redact_source_contents(|src, content| filter(src, content));
          }
        }
        Ok(false)
      })
      .collect();
    for (i, res) in prep_results.into_iter().enumerate() {
      pre_redacted_flags[i] = res?;
    }
  } else {
    for (i, asset) in assets.iter_mut().enumerate() {
      let filename = asset.filename.clone();
      if let (InstantiationKind::Ecma(ecma_meta), Some(map)) = (&mut asset.meta, asset.map.as_mut())
      {
        let file_path = options.cwd.as_path().join(&options.out_dir).join(filename.as_str());
        ecma_meta.file_dir =
          file_path.parent().expect("chunk file name should have a parent").to_path_buf();
        if let Some((_, Some(hit))) = &cached_hits[i] {
          pre_redacted_flags[i] = hit.pre_redacted;
          continue;
        }
        prepare_sourcemap(options, map, &ecma_meta.file_dir, filename.as_str()).await?;
        if let Some(filter) = &content_filter {
          pre_redacted_flags[i] = map.redact_source_contents(|src, content| filter(src, content))?;
        }
      }
    }
  }

  let precomputed_sourcemap_json: IndexVec<InsChunkIdx, Option<String>> = assets
    .par_iter()
    .enumerate()
    .map(|(idx, asset)| {
      if let Some((_, Some(hit))) = &cached_hits[idx] {
        if hit.filename == asset.filename {
          return Some(hit.json.to_string());
        }
        let old_base = std::path::Path::new(hit.filename.as_str())
          .file_name()
          .map(|f| f.to_string_lossy())
          .unwrap_or_default();
        let new_base = std::path::Path::new(asset.filename.as_str())
          .file_name()
          .map(|f| f.to_string_lossy())
          .unwrap_or_default();
        let old_needle = format!("\"file\":\"{old_base}\"");
        let new_needle = format!("\"file\":\"{new_base}\"");
        return Some(hit.json.replacen(&old_needle, &new_needle, 1));
      }
      let idx = InsChunkIdx::from(idx);
      let (Some(map), InstantiationKind::Ecma(ecma_meta)) = (&asset.map, &asset.meta) else {
        return None;
      };
      let needs_hash = index_sourcemap_hash_placeholders[idx].is_some();
      let emits_json = matches!(
        options.sourcemap,
        Some(SourceMapType::File | SourceMapType::Hidden)
      );
      if !needs_hash && !emits_json {
        return None;
      }
      if options.sourcemap_debug_ids && options.sourcemap.is_some() {
        let mut map = map.clone();
        let debug_id_str = crate::utils::uuid::uuid_v4_string_from_u128(ecma_meta.debug_id);
        map.set_debug_id(&debug_id_str);
        Some(map.to_json_string_with_null_contents(pre_redacted_flags[idx.index()]))
      } else {
        Some(map.to_json_string_with_null_contents(pre_redacted_flags[idx.index()]))
      }
    })
    .collect::<Vec<_>>()
    .into();

  if let Some(c) = cache.as_deref_mut() {
    if !can_cache_maps {
      for asset in assets.iter() {
        if let Some(chunk_idx) = asset.originate_from
          && let Some(&digest) = c.current_chunk_map_digests.get(&chunk_idx)
          && let Some((pre_hash, pls, post_code)) = c.current_chunk_minify_state.get(&chunk_idx)
        {
          c.cached_chunk_sourcemaps.insert(
            digest,
            crate::types::scan_stage_cache::CachedChunkSourcemap {
              filename: asset.filename.clone(),
              pre_minify_hash: *pre_hash,
              pre_minify_placeholders: pls.clone(),
              post_minify_content: post_code.clone(),
              json: arcstr::ArcStr::default(),
              names: Vec::new(),
              original_file_names: Vec::new(),
              pre_redacted: false,
            },
          );
        }
      }
    }
    for (idx, state) in cached_hits.iter().enumerate() {
      let ins_idx = InsChunkIdx::from(idx);
      if let Some((digest, hit_opt)) = state
        && hit_opt.is_none()
        && let Some(json) = &precomputed_sourcemap_json[ins_idx]
      {
        let (pre_minify_hash, pre_minify_placeholders, post_minify_content) = assets[idx]
          .originate_from
          .and_then(|c_idx| c.current_chunk_minify_state.get(&c_idx))
          .cloned()
          .unwrap_or(([0u8; 32], Vec::new(), None));
        c.cached_chunk_sourcemaps.insert(
          *digest,
          crate::types::scan_stage_cache::CachedChunkSourcemap {
            filename: assets[idx].filename.clone(),
            pre_minify_hash,
            pre_minify_placeholders,
            post_minify_content,
            json: arcstr::ArcStr::from(json.as_str()),
            names: Vec::new(),
            original_file_names: Vec::new(),
            pre_redacted: pre_redacted_flags[idx],
          },
        );
      }
    }
    if !c.current_chunk_map_digests.is_empty() {
      let live_digests: FxHashSet<[u128; 2]> =
        c.current_chunk_map_digests.values().copied().collect();
      c.cached_chunk_sourcemaps.retain(|digest, _| live_digests.contains(digest));
    }
  }

  // Inline sourcemaps also resolve `[hash]`: the name is still reported on the output chunk
  // even though no `.map` asset is written.
  let sourcemap_final_hashes: Vec<(InsChunkIdx, String)> = assets
    .par_iter()
    .enumerate()
    .filter_map(|(idx, _asset)| {
      let idx = InsChunkIdx::from(idx);
      index_sourcemap_hash_placeholders[idx].as_ref()?;
      let json = precomputed_sourcemap_json[idx].as_ref()?;
      let mut hasher = Xxh3::default();
      hasher.update(json.as_bytes());
      let hash = encode_hash_with_base(&hasher.digest128().to_le_bytes(), hash_base);
      Some((idx, hash))
    })
    .collect();
  let sourcemap_hashes_by_placeholder =
    get_sourcemap_hashes_by_placeholder(&sourcemap_final_hashes, index_sourcemap_hash_placeholders)
      .collect::<FxHashMap<_, _>>();

  assets.par_iter_mut().for_each(|asset| {
    if let InstantiationKind::Ecma(ecma_meta) = &mut asset.meta
      && let Some(sourcemap_filename) = &mut ecma_meta.sourcemap_filename
      && let Cow::Owned(replaced) = replace_placeholder_with_hash(
        sourcemap_filename,
        &sourcemap_hashes_by_placeholder,
        &HASH_PLACEHOLDER_LEFT_FINDER,
      )
    {
      *sourcemap_filename = replaced;
    }
  });

  let derived_assets = assets
    .par_iter_mut()
    .zip(precomputed_sourcemap_json.into_par_iter())
    .zip(cached_hits.into_par_iter())
    .map(|((asset, precomputed_json), cached_state)| -> BuildResult<Option<Asset>> {
      match &mut asset.meta {
        InstantiationKind::Ecma(ecma_meta) => {
          let asset_code = mem::take(&mut asset.content);
          let mut code = asset_code.try_into_string()?;
          let mut derived = None;
          if let Some(map) = asset.map.as_mut() {
            let map_filename = ecma_meta
              .sourcemap_filename
              .clone()
              .unwrap_or_else(|| concat_string!(asset.filename, ".map"));
            if let Some(sourcemap_asset) = crate::utils::process_code_and_sourcemap::emit_sourcemap_with_json(
              options,
              &mut code,
              map,
              &map_filename,
              ecma_meta.debug_id,
              /*is_css*/ false,
              precomputed_json,
            )? {
              if ecma_meta.sourcemap_filename.is_none() {
                let sourcemap_filename =
                  if matches!(options.sourcemap, Some(SourceMapType::Inline) | None) {
                    None
                  } else {
                    Some(sourcemap_asset.filename.to_string())
                  };
                ecma_meta.sourcemap_filename = sourcemap_filename;
              }
              let (names, original_file_names) = if let Some((_, Some(hit))) = cached_state {
                (hit.names, hit.original_file_names)
              } else {
                (sourcemap_asset.names, sourcemap_asset.original_file_names)
              };
              derived = Some(Asset {
                originate_from: asset.originate_from,
                content: sourcemap_asset.source,
                filename: sourcemap_asset.filename.clone(),
                map: None,
                meta: InstantiationKind::Sourcemap(Box::new(rolldown_common::SourcemapAssetMeta {
                  names,
                  original_file_names,
                })),
              });
            }
          }
          asset.content = code.into();
          Ok(derived)
        }
        InstantiationKind::None | InstantiationKind::Sourcemap(_) => Ok(None),
      }
    })
    .collect::<BuildResult<Vec<_>>>()?;

  if let Some(c) = cache.as_deref_mut() {
    for (idx, derived_opt) in derived_assets.iter().enumerate() {
      if pre_redacted_flags.get(idx).copied().unwrap_or(false)
        && let Some(derived) = derived_opt
      {
        c.pre_redacted_maps.insert(derived.filename.clone());
      }
    }
  }

  assets.extend(derived_assets.into_iter().flatten());

  Ok(())
}

fn get_sourcemap_hashes_by_placeholder<'a>(
  sourcemap_final_hashes: &'a [(InsChunkIdx, String)],
  index_sourcemap_hash_placeholders: &IndexVec<InsChunkIdx, Option<Vec<String>>>,
) -> impl Iterator<Item = (String, &'a str)> {
  sourcemap_final_hashes
    .iter()
    .filter_map(|(idx, hash)| {
      index_sourcemap_hash_placeholders[*idx].as_ref().map(|placeholders| {
        placeholders.iter().map(|placeholder| (placeholder.clone(), &hash[..placeholder.len()]))
      })
    })
    .flatten()
}

fn collect_transitive_dependencies(
  index: InsChunkIdx,
  index_direct_dependencies: &IndexVec<InsChunkIdx, Vec<InsChunkIdx>>,
) -> FxIndexSet<InsChunkIdx> {
  fn traverse(
    index: InsChunkIdx,
    dep_map: &IndexVec<InsChunkIdx, Vec<InsChunkIdx>>,
    visited: &mut FxIndexSet<InsChunkIdx>,
  ) {
    for dep_index in &dep_map[index] {
      if !visited.contains(dep_index) {
        visited.insert(*dep_index);
        traverse(*dep_index, dep_map, visited);
      }
    }
  }

  let mut visited_deps = FxIndexSet::default();
  traverse(index, index_direct_dependencies, &mut visited_deps);
  visited_deps
}

/// Walks chunks in a deterministic order and rehashes any chunk whose resolved file name would
/// collide with a previously assigned one. Comparison is case-insensitive so the output is safe
/// to write on case-insensitive filesystems (macOS HFS+, Windows NTFS).
///
/// Mirrors Rollup's `generateFinalHashes` rehash loop.
fn deconflict_filenames(
  index_instantiated_chunks: &IndexInstantiatedChunks,
  index_final_hashes: &mut IndexVec<InsChunkIdx, (String, u128)>,
  hash_base: u8,
) {
  let mut taken: FxHashSet<String> = FxHashSet::with_capacity(index_instantiated_chunks.len());
  for (asset_idx, chunk) in index_instantiated_chunks.iter_enumerated() {
    let preliminary_filename = chunk.preliminary_filename.as_str();
    let Some(placeholders) = chunk.preliminary_filename.hash_placeholder() else {
      taken.insert(preliminary_filename.to_lowercase());
      continue;
    };
    loop {
      let candidate =
        resolve_filename(preliminary_filename, placeholders, &index_final_hashes[asset_idx].0);
      if taken.insert(candidate.to_lowercase()) {
        break;
      }
      index_final_hashes[asset_idx] = rehash(&index_final_hashes[asset_idx].0, hash_base);
    }
  }
}

fn resolve_filename(preliminary_filename: &str, placeholders: &[String], hash_str: &str) -> String {
  let mut result = preliminary_filename.to_string();
  for placeholder in placeholders {
    result = result.replace(placeholder.as_str(), &hash_str[..placeholder.len()]);
  }
  result
}

fn rehash(prev_hash_str: &str, hash_base: u8) -> (String, u128) {
  let mut hasher = Xxh3::default();
  hasher.update(prev_hash_str.as_bytes());
  let digest = hasher.digest128();
  (encode_hash_with_base(&digest.to_le_bytes(), hash_base), digest)
}
