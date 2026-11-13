use std::{borrow::Cow, cmp::Reverse, collections::BinaryHeap, hash::Hash, mem, path::Path};

use arcstr::ArcStr;
use futures::future::try_join_all;
use itertools::Itertools;
use oxc_index::IndexVec;
use petgraph::{
  algo::kosaraju_scc,
  graph::{DiGraph, NodeIndex},
};
use rolldown_common::{
  Asset, HashCharacters, InsChunkIdx, InstantiatedChunk, InstantiationKind,
  NormalizedBundlerOptions, PathsOutputOption, PreliminaryFilename, SourceMapType, StrOrBytes,
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
  utils::process_code_and_sourcemap::{emit_sourcemap, prepare_sourcemap, sourcemap_file_url},
};

#[tracing::instrument(level = "debug", skip_all)]
pub async fn finalize_assets(
  chunk_graph: &ChunkGraph,
  link_output: &LinkStageOutput,
  mut index_instantiated_chunks: IndexInstantiatedChunks,
  index_chunk_to_instances: &IndexChunkToInstances,
  hash_characters: HashCharacters,
  options: &NormalizedBundlerOptions,
  resolved_paths: Option<&PathsOutputOption>,
) -> BuildResult<AssetVec> {
  // Prepare maps once, before hashing comments that contain map data or a map content hash.
  // Final chunk names are substituted below. See internal-docs/chunk-hash/implementation.md.
  try_join_all(index_instantiated_chunks.iter_mut().map(async |chunk| {
    if let (InstantiationKind::Ecma(ecma_meta), Some(map)) = (&mut chunk.kind, &mut chunk.map) {
      let filename = chunk.preliminary_filename.as_str();
      let file_path = options.cwd.join(&options.out_dir).join(filename);
      ecma_meta.file_dir =
        file_path.parent().expect("chunk file name should have a parent").to_path_buf();
      prepare_sourcemap(options, map, &ecma_meta.file_dir, filename).await?;
    }
    Ok::<(), anyhow::Error>(())
  }))
  .await?;

  let ins_chunk_idx_by_placeholder = index_instantiated_chunks
    .iter_enumerated()
    .filter_map(|(ins_chunk_idx, ins_chunk)| {
      ins_chunk.preliminary_filename.hash_placeholder().map(move |placeholders| {
        placeholders.iter().map(move |hash_placeholder| (hash_placeholder.as_str(), ins_chunk_idx))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let index_direct_dependencies: IndexVec<InsChunkIdx, Vec<InsChunkIdx>> =
    index_instantiated_chunks
      .par_iter()
      .map(|asset| {
        if let StrOrBytes::Str(content) = &asset.content {
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

  let hash_base = hash_characters.base();
  let index_standalone_content_hashes: IndexVec<InsChunkIdx, String> = index_instantiated_chunks
    .par_iter()
    .map(|chunk| {
      // Normalize rolldown-generated placeholders before hashing so the hash is stable across
      // their (transient) index drift. See `internal-docs/chunk-hash/implementation.md` for the full reasoning.
      let mut hash = match &chunk.content {
        StrOrBytes::Str(content) => {
          let mut hasher = Xxh3::default();
          visit_with_placeholders_defaulted(
            content,
            &HASH_PLACEHOLDER_LEFT_FINDER,
            |placeholder| ins_chunk_idx_by_placeholder.contains_key(placeholder),
            |bytes| hasher.update(bytes),
          );
          hash_sourcemap_comment(&mut hasher, chunk, &ins_chunk_idx_by_placeholder, options)?;
          to_url_safe_base64(hasher.digest128().to_le_bytes())
        }
        StrOrBytes::Bytes(_) => xxhash_base64_url(chunk.content.as_bytes()),
      };
      if let Some(augment_chunk_hash) = &chunk.augment_chunk_hash {
        hash.push_str(augment_chunk_hash);
        hash = xxhash_base64_url(hash.as_bytes());
      }
      Ok(hash)
    })
    .collect::<BuildResult<Vec<_>>>()?
    .into();

  let mut index_final_hashes: IndexVec<InsChunkIdx, (String, u128)> = (0
    ..index_instantiated_chunks.len())
    .into_par_iter()
    .map(|asset_idx| {
      let mut hasher = Xxh3::default();
      let asset_idx = InsChunkIdx::from(asset_idx);
      index_standalone_content_hashes[asset_idx].hash(&mut hasher);

      // Calculate this asset's transitive closure only for the duration of its hash. Retaining
      // every closure at once makes memory quadratic for output graphs with many shared chunks.
      visit_transitive_dependencies(asset_idx, &index_direct_dependencies, |dep_id| {
        index_standalone_content_hashes[dep_id].hash(&mut hasher);
      });

      let digested = hasher.digest128();
      (encode_hash_with_base(&digested.to_le_bytes(), hash_base), digested)
    })
    .collect::<Vec<_>>()
    .into();

  assign_final_hashes(
    |idx| &index_instantiated_chunks[idx].preliminary_filename,
    &index_direct_dependencies,
    &mut index_final_hashes,
    hash_base,
  );

  let chunk_hashes_by_placeholder = index_final_hashes
    .iter_enumerated()
    .filter_map(|(idx, (hash, _))| {
      index_instantiated_chunks[idx].preliminary_filename.hash_placeholder().map(|placeholders| {
        placeholders.iter().map(|placeholder| (placeholder.clone(), &hash[..placeholder.len()]))
      })
    })
    .flatten()
    .collect::<FxHashMap<_, _>>();

  let mut assets: AssetVec = index_instantiated_chunks
    .into_par_iter()
    .enumerate()
    .map(|(asset_idx, mut instantiated_chunk)| {
      let asset_idx = InsChunkIdx::from(asset_idx);

      let filename: ArcStr = replace_placeholder_with_hash(
        instantiated_chunk.preliminary_filename.as_str(),
        &chunk_hashes_by_placeholder,
        &HASH_PLACEHOLDER_LEFT_FINDER,
      )
      .into();

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
          replace_placeholder_with_hash(
            str,
            &chunk_hashes_by_placeholder,
            &HASH_PLACEHOLDER_LEFT_FINDER,
          )
          .into()
        });
        if let Some(map) = &mut instantiated_chunk.map {
          let file_path = options.cwd.join(&options.out_dir).join(filename.as_str());
          ecma_meta.file_dir =
            file_path.parent().expect("chunk file name should have a parent").to_path_buf();
          map.set_file(Path::new(filename.as_str()).file_name().unwrap().to_str().unwrap());
          // A callback may have included the preliminary map path in a source path.
          if map.get_sources().any(|source| source.contains("!~{")) {
            let sources = map
              .get_sources()
              .map(|source| {
                replace_placeholder_with_hash(
                  source,
                  &chunk_hashes_by_placeholder,
                  &HASH_PLACEHOLDER_LEFT_FINDER,
                )
                .into_owned()
              })
              .collect_vec();
            map.set_sources(sources);
          }
        }
      }
      if let StrOrBytes::Str(content) = &mut instantiated_chunk.content {
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
    .collect::<Vec<_>>();

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
        .flat_map(|importee_idx| &index_chunk_to_instances[*importee_idx])
        .map(|importee_asset_idx| index_ins_chunk_to_filename[*importee_asset_idx].clone())
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
        .flat_map(|importee_idx| &index_chunk_to_instances[*importee_idx])
        .map(|importee_asset_idx| index_ins_chunk_to_filename[*importee_asset_idx].clone())
        .chain(chunk.dynamic_imports_from_external_modules.iter().map(|idx| {
          link_output.module_table[*idx]
            .as_external()
            .expect("dynamic_imports_from_external_modules should only contain external modules")
            .get_file_name(resolved_paths)
        }))
        .collect();
    }
  });

  finalize_sourcemaps(&mut assets, &index_sourcemap_hash_placeholders, options, hash_base)?;

  Ok(assets)
}

fn hash_sourcemap_comment(
  hasher: &mut Xxh3,
  chunk: &InstantiatedChunk,
  chunks_by_placeholder: &FxHashMap<&str, InsChunkIdx>,
  options: &NormalizedBundlerOptions,
) -> BuildResult<()> {
  let (InstantiationKind::Ecma(_), Some(map), Some(sourcemap)) =
    (&chunk.kind, &chunk.map, options.sourcemap)
  else {
    return Ok(());
  };
  if options.sourcemap_debug_ids {
    // The debug ID is derived from this hash; only its presence is an independent input.
    hasher.update(b"\n//# debugId=00000000-0000-4000-8000-000000000000");
  }
  match sourcemap {
    SourceMapType::Hidden => {}
    SourceMapType::Inline => {
      // Hash raw JSON before Base64 encoding can hide generated placeholders.
      hasher.update(b"\n//# sourceMappingURL=data:application/json,");
      visit_with_placeholders_defaulted(
        &map.to_json_string(),
        &HASH_PLACEHOLDER_LEFT_FINDER,
        |placeholder| chunks_by_placeholder.contains_key(placeholder),
        |bytes| hasher.update(bytes),
      );
    }
    SourceMapType::File => {
      let mut map_filename = chunk.preliminary_sourcemap_filename.as_ref().map_or_else(
        || Cow::Owned(concat_string!(chunk.preliminary_filename.as_str(), ".map")),
        |filename| Cow::Borrowed(filename.as_str()),
      );
      if let Some(placeholders) =
        chunk.preliminary_sourcemap_filename.as_ref().and_then(|name| name.hash_placeholder())
      {
        // The emitted map hash includes the final chunk name, which depends on this hash.
        // Use the prepared map with normalized chunk placeholders as its stable hash input.
        let mut map_hasher = Xxh3::default();
        visit_with_placeholders_defaulted(
          &map.to_json_string(),
          &HASH_PLACEHOLDER_LEFT_FINDER,
          |placeholder| chunks_by_placeholder.contains_key(placeholder),
          |bytes| map_hasher.update(bytes),
        );
        let map_hash = encode_hash_with_base(
          &map_hasher.digest128().to_le_bytes(),
          options.hash_characters.base(),
        );
        let map_hashes = placeholders
          .iter()
          .map(|placeholder| (placeholder.clone(), &map_hash[..placeholder.len()]))
          .collect();
        map_filename = Cow::Owned(
          replace_placeholder_with_hash(&map_filename, &map_hashes, &HASH_PLACEHOLDER_LEFT_FINDER)
            .into_owned(),
        );
      }
      // Normalize the raw filename before URL encoding escapes the placeholder delimiters.
      let mut normalized_filename = Vec::with_capacity(map_filename.len());
      visit_with_placeholders_defaulted(
        &map_filename,
        &HASH_PLACEHOLDER_LEFT_FINDER,
        |placeholder| chunks_by_placeholder.contains_key(placeholder),
        |bytes| normalized_filename.extend_from_slice(bytes),
      );
      let normalized_filename =
        String::from_utf8(normalized_filename).expect("placeholder normalization preserves UTF-8");
      hasher.update(b"\n//# sourceMappingURL=");
      hasher.update(sourcemap_file_url(options, &normalized_filename, false)?.as_bytes());
    }
  }
  Ok(())
}

fn finalize_sourcemaps(
  assets: &mut AssetVec,
  index_sourcemap_hash_placeholders: &IndexVec<InsChunkIdx, Option<Vec<String>>>,
  options: &NormalizedBundlerOptions,
  hash_base: u8,
) -> BuildResult<()> {
  // Inline sourcemaps also resolve `[hash]`: the name is still reported on the output chunk
  // even though no `.map` asset is written.
  let sourcemap_final_hashes =
    generate_sourcemap_hashes_by_idx(assets, index_sourcemap_hash_placeholders, hash_base);
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

  let mut derived_assets = Vec::new();
  for asset in assets.iter_mut() {
    match &mut asset.meta {
      InstantiationKind::Ecma(ecma_meta) => {
        let asset_code = mem::take(&mut asset.content);
        let mut code = asset_code.try_into_string()?;
        if let Some(map) = asset.map.as_mut() {
          let map_filename = ecma_meta
            .sourcemap_filename
            .clone()
            .unwrap_or_else(|| concat_string!(asset.filename, ".map"));
          if let Some(sourcemap_asset) = emit_sourcemap(
            options,
            &mut code,
            map,
            &map_filename,
            ecma_meta.debug_id,
            /*is_css*/ false,
          )? {
            derived_assets.push(Asset {
              originate_from: None,
              content: sourcemap_asset.source,
              filename: sourcemap_asset.filename.clone(),
              map: None,
              meta: InstantiationKind::Sourcemap(Box::new(rolldown_common::SourcemapAssetMeta {
                names: sourcemap_asset.names,
                original_file_names: sourcemap_asset.original_file_names,
              })),
            });
            if ecma_meta.sourcemap_filename.is_none() {
              let sourcemap_filename =
                if matches!(options.sourcemap, Some(SourceMapType::Inline) | None) {
                  None
                } else {
                  Some(sourcemap_asset.filename.to_string())
                };
              ecma_meta.sourcemap_filename = sourcemap_filename;
            }
          }
        }
        asset.content = code.into();
      }
      InstantiationKind::None | InstantiationKind::Sourcemap(_) => {}
    }
  }

  assets.extend(derived_assets);

  Ok(())
}

fn generate_sourcemap_hashes_by_idx(
  assets: &AssetVec,
  index_sourcemap_hash_placeholders: &IndexVec<InsChunkIdx, Option<Vec<String>>>,
  hash_base: u8,
) -> Vec<(InsChunkIdx, String)> {
  assets
    .par_iter()
    .enumerate()
    .filter_map(|(idx, asset)| {
      let idx = InsChunkIdx::from(idx);
      let (Some(map), InstantiationKind::Ecma(_)) = (&asset.map, &asset.meta) else {
        return None;
      };
      index_sourcemap_hash_placeholders[idx].as_ref()?;
      let mut hasher = Xxh3::default();
      hasher.update(map.to_json_string().as_bytes());
      let hash = encode_hash_with_base(&hasher.digest128().to_le_bytes(), hash_base);
      Some((idx, hash))
    })
    .collect()
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

#[derive(Default)]
struct TransitiveDepsScratch {
  /// Indexed by `InsChunkIdx`; only entries listed in `touched` are ever `true`.
  visited: Vec<bool>,
  touched: Vec<InsChunkIdx>,
  /// Explicit DFS stack of `(chunk, next direct dependency to inspect)`.
  stack: Vec<(InsChunkIdx, usize)>,
}

thread_local! {
  static TRANSITIVE_DEPS_SCRATCH: std::cell::RefCell<TransitiveDepsScratch> =
    std::cell::RefCell::default();
}

/// Calls `visit` for every chunk transitively depended on by `index`, in depth-first preorder.
/// The root itself is only visited if a dependency cycle leads back to it.
///
/// This is called once per output chunk, so it reuses per-thread scratch storage and clears only
/// the entries it touched: hashing each closure into a fresh ordered hash set dominated
/// finalization for graphs with thousands of chunks sharing large common dependencies.
fn visit_transitive_dependencies(
  index: InsChunkIdx,
  index_direct_dependencies: &IndexVec<InsChunkIdx, Vec<InsChunkIdx>>,
  mut visit: impl FnMut(InsChunkIdx),
) {
  TRANSITIVE_DEPS_SCRATCH.with_borrow_mut(|scratch| {
    let TransitiveDepsScratch { visited, touched, stack } = scratch;
    if visited.len() < index_direct_dependencies.len() {
      visited.resize(index_direct_dependencies.len(), false);
    }
    stack.push((index, 0));
    while let Some(top) = stack.last_mut() {
      let (current, next) = *top;
      let Some(&dep) = index_direct_dependencies[current].get(next) else {
        stack.pop();
        continue;
      };
      top.1 += 1;
      if !visited[dep.index()] {
        visited[dep.index()] = true;
        touched.push(dep);
        visit(dep);
        stack.push((dep, 0));
      }
    }
    for idx in touched.drain(..) {
      visited[idx.index()] = false;
    }
  });
}

/// Turns each chunk's base hash (its content plus its transitive dependencies' contents) into its
/// final hash, so that no two resolved file names collide and a chunk's name changes whenever a
/// chunk it imports is renamed. Comparison is case-insensitive so the output is safe to write on
/// case-insensitive filesystems (macOS HFS+, Windows NTFS).
///
/// Chunks are settled one import cycle (strongly connected component) at a time, dependencies
/// first, so a chunk's dependencies have their final names before the chunk itself is named. Among
/// cycles that are ready, the one with the lowest chunk index goes first, so the lowest index keeps
/// its hash when unrelated chunks collide, as in Rollup's `generateFinalHashes`.
///
/// See internal-docs/chunk-hash/implementation.md.
fn assign_final_hashes<'a>(
  preliminary_filename: impl Fn(InsChunkIdx) -> &'a PreliminaryFilename,
  index_direct_dependencies: &IndexVec<InsChunkIdx, Vec<InsChunkIdx>>,
  index_hashes: &mut IndexVec<InsChunkIdx, (String, u128)>,
  hash_base: u8,
) {
  let chunk_count = index_direct_dependencies.len();
  let mut taken: FxHashSet<String> = FxHashSet::with_capacity(chunk_count);
  // File names without a hash cannot move out of the way, so claim them before any hashed name.
  for idx in index_direct_dependencies.indices() {
    let filename = preliminary_filename(idx);
    if filename.hash_placeholder().is_none() {
      taken.insert(filename.to_lowercase());
    }
  }

  let edge_count = index_direct_dependencies.iter().map(Vec::len).sum();
  let mut graph = DiGraph::<(), ()>::with_capacity(chunk_count, edge_count);
  for _ in 0..chunk_count {
    graph.add_node(());
  }
  for (importer, dependencies) in index_direct_dependencies.iter_enumerated() {
    for dependency in dependencies {
      graph.add_edge(NodeIndex::new(importer.index()), NodeIndex::new(dependency.index()), ());
    }
  }
  // `kosaraju_scc` is iterative; `tarjan_scc` recurses and overflows on deep import chains.
  let mut components: Vec<Vec<InsChunkIdx>> = kosaraju_scc(&graph)
    .into_iter()
    .map(|component| {
      let mut members =
        component.into_iter().map(|node| InsChunkIdx::from(node.index())).collect_vec();
      members.sort_unstable();
      members
    })
    .collect();
  drop(graph);
  let mut component_of: IndexVec<InsChunkIdx, usize> = IndexVec::from_vec(vec![0; chunk_count]);
  for (component_idx, members) in components.iter().enumerate() {
    for &member in members {
      component_of[member] = component_idx;
    }
  }

  // Kahn's algorithm over the components: a component is ready once every component it imports
  // has been settled.
  let mut pending_dependencies = vec![0usize; components.len()];
  let mut dependents: Vec<Vec<usize>> = vec![vec![]; components.len()];
  for (importer, dependencies) in index_direct_dependencies.iter_enumerated() {
    for &dependency in dependencies {
      let (from, to) = (component_of[importer], component_of[dependency]);
      if from != to {
        pending_dependencies[from] += 1;
        dependents[to].push(from);
      }
    }
  }
  let mut ready: BinaryHeap<Reverse<(InsChunkIdx, usize)>> = pending_dependencies
    .iter()
    .enumerate()
    .filter(|(_, pending)| **pending == 0)
    .map(|(component_idx, _)| Reverse((components[component_idx][0], component_idx)))
    .collect();

  // Whether a chunk's final hash differs from its base hash, i.e. its importers' content refers to
  // a name that their base hashes did not account for.
  let mut renamed: IndexVec<InsChunkIdx, bool> = IndexVec::from_vec(vec![false; chunk_count]);
  while let Some(Reverse((_, component_idx))) = ready.pop() {
    let members = mem::take(&mut components[component_idx]);
    let is_outside = |idx: InsChunkIdx| component_of[idx] != component_idx;

    let imports_renamed_chunk = members.iter().any(|&member| {
      index_direct_dependencies[member].iter().any(|&dep| is_outside(dep) && renamed[dep])
    });
    if imports_renamed_chunk {
      // Every member reaches every dependency of the component, so all of them are affected.
      for &member in &members {
        let mut hasher = Xxh3::default();
        index_hashes[member].0.hash(&mut hasher);
        visit_transitive_dependencies(member, index_direct_dependencies, |dep| {
          if is_outside(dep) {
            index_hashes[dep].0.hash(&mut hasher);
          }
        });
        let digest = hasher.digest128();
        index_hashes[member] = (encode_hash_with_base(&digest.to_le_bytes(), hash_base), digest);
      }
    }

    let resolve = |member: InsChunkIdx, index_hashes: &IndexVec<InsChunkIdx, (String, u128)>| {
      let filename = preliminary_filename(member);
      filename.hash_placeholder().map(|placeholders| {
        resolve_filename(filename, placeholders, &index_hashes[member].0).to_lowercase()
      })
    };
    let mut rehashed = false;
    loop {
      let collision = members.iter().position(|&member| {
        resolve(member, index_hashes).is_some_and(|candidate| !taken.insert(candidate))
      });
      let Some(collision) = collision else { break };
      // Release the names this attempt claimed before the collision.
      for &member in &members[..collision] {
        if let Some(candidate) = resolve(member, index_hashes) {
          taken.remove(&candidate);
        }
      }
      // Members of a cycle quote each other's names, so renaming one renames them all. The rank
      // separates byte-identical members, whose hashes would otherwise stay equal forever.
      let is_cycle = members.len() > 1;
      for (rank, &member) in members.iter().enumerate() {
        let rank = (rank as u64).to_le_bytes();
        let salt: &[u8] = if is_cycle { &rank } else { &[] };
        index_hashes[member] = rehash(&index_hashes[member].0, salt, hash_base);
      }
      rehashed = true;
    }

    if imports_renamed_chunk || rehashed {
      for &member in &members {
        renamed[member] = preliminary_filename(member).hash_placeholder().is_some();
      }
    }
    for &dependent in &dependents[component_idx] {
      pending_dependencies[dependent] -= 1;
      if pending_dependencies[dependent] == 0 {
        ready.push(Reverse((components[dependent][0], dependent)));
      }
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

fn rehash(prev_hash_str: &str, salt: &[u8], hash_base: u8) -> (String, u128) {
  let mut hasher = Xxh3::default();
  hasher.update(prev_hash_str.as_bytes());
  hasher.update(salt);
  let digest = hasher.digest128();
  (encode_hash_with_base(&digest.to_le_bytes(), hash_base), digest)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// The recursive, hash-set based traversal this replaced; its order defines chunk hashes.
  fn reference(
    index: InsChunkIdx,
    deps: &IndexVec<InsChunkIdx, Vec<InsChunkIdx>>,
  ) -> Vec<InsChunkIdx> {
    fn traverse(
      index: InsChunkIdx,
      deps: &IndexVec<InsChunkIdx, Vec<InsChunkIdx>>,
      out: &mut Vec<InsChunkIdx>,
    ) {
      for dep in &deps[index] {
        if !out.contains(dep) {
          out.push(*dep);
          traverse(*dep, deps, out);
        }
      }
    }
    let mut out = vec![];
    traverse(index, deps, &mut out);
    out
  }

  fn graph(edges: &[&[usize]]) -> IndexVec<InsChunkIdx, Vec<InsChunkIdx>> {
    edges.iter().map(|deps| deps.iter().map(|&d| InsChunkIdx::from(d)).collect()).collect()
  }

  #[test]
  fn transitive_dependencies_match_recursive_preorder() {
    let graphs = [
      // diamond with shared tail
      graph(&[&[1, 2], &[3], &[3, 4], &[4], &[]]),
      // cycle back to the root and a self loop
      graph(&[&[1], &[2, 1], &[0, 3], &[]]),
      // isolated and empty nodes
      graph(&[&[], &[0], &[]]),
      // dependency order is not index order
      graph(&[&[4, 2, 3], &[0], &[4], &[2, 1], &[]]),
    ];
    for deps in &graphs {
      // Traverse every root twice to check that per-thread scratch is reset between calls.
      for _ in 0..2 {
        for root in deps.indices() {
          let mut actual = vec![];
          visit_transitive_dependencies(root, deps, |dep| actual.push(dep));
          assert_eq!(actual, reference(root, deps), "root {root:?}");
        }
      }
    }
  }

  #[test]
  fn transitive_dependencies_handle_deep_chain_on_small_stack() {
    let n = 200_000;
    let deps: IndexVec<InsChunkIdx, Vec<InsChunkIdx>> =
      (0..n).map(|i| if i + 1 < n { vec![InsChunkIdx::from(i + 1)] } else { vec![] }).collect();
    // A recursive traversal overflows this stack long before the end of the chain.
    let count = std::thread::Builder::new()
      .stack_size(256 * 1024)
      .spawn(move || {
        let mut count = 0;
        visit_transitive_dependencies(InsChunkIdx::from(0), &deps, |_| count += 1);
        count
      })
      .unwrap()
      .join()
      .unwrap();
    assert_eq!(count, n - 1);
  }

  /// Runs `assign_final_hashes` on chunks given as `(file name template, direct dependencies,
  /// base hash input)` and returns their resolved file names.
  fn assign(chunks: &[(&str, &[usize], &str)]) -> Vec<String> {
    let filenames: IndexVec<InsChunkIdx, PreliminaryFilename> = chunks
      .iter()
      .enumerate()
      .map(|(idx, (template, _, _))| {
        if template.contains("[hash]") {
          let placeholder = format!("!~{{{idx:03}}}~");
          PreliminaryFilename::new(
            template.replace("[hash]", &placeholder).into(),
            Some(vec![placeholder]),
          )
        } else {
          PreliminaryFilename::new((*template).into(), None)
        }
      })
      .collect();
    let deps = graph(&chunks.iter().map(|(_, deps, _)| *deps).collect_vec());
    let mut hashes: IndexVec<InsChunkIdx, (String, u128)> = chunks
      .iter()
      .map(|(_, _, content)| {
        let digest = xxhash_rust::xxh3::xxh3_128(content.as_bytes());
        (encode_hash_with_base(&digest.to_le_bytes(), 64), digest)
      })
      .collect();
    assign_final_hashes(|idx| &filenames[idx], &deps, &mut hashes, 64);
    filenames
      .iter_enumerated()
      .map(|(idx, filename)| match filename.hash_placeholder() {
        Some(placeholders) => resolve_filename(filename, placeholders, &hashes[idx].0),
        None => filename.to_string(),
      })
      .collect()
  }

  // https://github.com/rolldown/rolldown/issues/11694
  #[test]
  fn renaming_a_dependency_renames_its_importers() {
    // `identity` and its importer `user`.
    let before = assign(&[("[hash].js", &[], "identity"), ("[hash].js", &[0], "user")]);
    // A byte-identical chunk with a lower index takes `identity`'s name.
    let after = assign(&[
      ("[hash].js", &[], "identity"),
      ("[hash].js", &[], "identity"),
      ("[hash].js", &[1], "user"),
      ("[hash].js", &[2], "app"),
    ]);
    assert_eq!(after[0], before[0]);
    assert_ne!(after[1], before[0]);
    // `user` now imports a different file name, so its own name must change as well.
    assert_ne!(after[2], before[1]);
    assert_eq!(after.iter().unique().count(), after.len());
    // And so must the name of every chunk that transitively imports it.
    let app_before = assign(&[
      ("[hash].js", &[], "identity"),
      ("[hash].js", &[0], "user"),
      ("[hash].js", &[1], "app"),
    ]);
    assert_ne!(after[3], app_before[2]);
  }

  #[test]
  fn byte_identical_members_of_an_import_cycle_get_distinct_names() {
    let names = assign(&[
      ("[hash].js", &[1], "cycle"),
      ("[hash].js", &[0], "cycle"),
      ("[hash].js", &[0], "importer"),
    ]);
    assert_eq!(names.iter().unique().count(), names.len());
  }

  #[test]
  fn hashed_names_avoid_fixed_names_regardless_of_order() {
    let [hashed] = <[String; 1]>::try_from(assign(&[("[hash].js", &[], "a")])).unwrap();
    let names = assign(&[("[hash].js", &[], "a"), (hashed.to_uppercase().as_str(), &[], "b")]);
    assert_ne!(names[0].to_lowercase(), hashed.to_lowercase());
    assert_eq!(names[1], hashed.to_uppercase());
  }

  #[test]
  fn final_hashes_handle_deep_chain_on_small_stack() {
    let n = 200_000;
    let contents = (0..n).map(|i| i.to_string()).collect_vec();
    let deps = (0..n).map(|i| if i + 1 < n { vec![i + 1] } else { vec![] }).collect_vec();
    // A recursive component search overflows this stack long before the end of the chain.
    let names = std::thread::Builder::new()
      .stack_size(256 * 1024)
      .spawn(move || {
        let chunks =
          (0..n).map(|i| ("[hash].js", deps[i].as_slice(), contents[i].as_str())).collect_vec();
        assign(&chunks)
      })
      .unwrap()
      .join()
      .unwrap();
    assert_eq!(names.len(), n);
  }
}
