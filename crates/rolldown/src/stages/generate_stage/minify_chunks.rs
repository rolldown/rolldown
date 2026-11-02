use std::{collections::BTreeMap, sync::OnceLock};

use oxc::{
  codegen::{self, CodegenOptions, CommentOptions},
  minifier::MinifierOptions as OxcMinifierOptions,
};
use oxc_allocator::AllocatorPool;
use rolldown_common::{
  ChunkIdx, InstantiatedChunk, InstantiationKind, MinifyOptions, NormalizedBundlerOptions, Output,
};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_error::{BuildDiagnostic, BuildResult, InvalidOptionType};
use rolldown_sourcemap::collapse_sourcemaps;
use rolldown_utils::poc_prof::{Phase, Timer};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use rustc_hash::FxHashSet;

use crate::type_alias::IndexInstantiatedChunks;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn minify_chunks(
    options: &NormalizedBundlerOptions,
    chunks: &mut IndexInstantiatedChunks,
    ast_dce_chunks: &FxHashSet<ChunkIdx>,
    ast_property_cache: Option<oxc::minifier::ManglePropertyCache>,
    deferred: super::chunk_ast_dce::DeferredChunks,
    module_table: &rolldown_common::ModuleTable,
  ) -> BuildResult<Option<BTreeMap<String, Option<String>>>> {
    let Some((compress, minify_options, remove_whitespace, ascii_only)) =
      chunk_minify_options(&options.minify)
    else {
      return Ok(None);
    };
    if minify_options.mangle_properties.is_some()
      && chunks.iter().filter(|chunk| is_minifiable_ecma_chunk(chunk)).take(2).count() > 1
    {
      return Err(
        BuildDiagnostic::invalid_option(InvalidOptionType::ManglePropertiesWithMultipleChunks)
          .into(),
      );
    }
    let property_mangle_cache = OnceLock::new();
    if let Some(cache) = ast_property_cache {
      property_mangle_cache.set(cache).expect("only one property-mangled chunk is permitted");
    }
    for (idx, pending) in deferred.pending {
      let chunk =
        chunks.iter_mut().find(|chunk| chunk.originate_from == idx).expect("pending chunk output");
      let optimized = pending.optimize(
        options,
        module_table,
        chunk.preliminary_filename.as_str(),
        minify_options,
      );
      chunk.content = optimized.code.into();
      chunk.map = optimized.map;
      if let InstantiationKind::Ecma(meta) = &mut chunk.kind {
        let rendered = &meta.rendered_chunk;
        let mut modules = rendered.modules.clone();
        let module_indices: rustc_hash::FxHashMap<_, _> =
          modules.keys.iter().enumerate().map(|(index, id)| (id, index)).collect();
        for (module_idx, sources) in optimized.modules {
          let id = &module_table[module_idx].as_normal().expect("normal metadata donor").id;
          if let Some(&index) = module_indices.get(id) {
            let previous = &modules.values[index];
            modules.values[index] = std::sync::Arc::new(rolldown_common::RenderedModule::new(
              sources,
              previous.rendered_exports.clone(),
              previous.exec_order,
            ));
          }
        }
        // Hook arguments are immutable Arc snapshots. Publish a new final metadata object.
        meta.rendered_chunk = std::sync::Arc::new(rolldown_common::RollupRenderedChunk {
          originate_from: rendered.originate_from,
          name: rendered.name.clone(),
          is_entry: rendered.is_entry,
          is_dynamic_entry: rendered.is_dynamic_entry,
          facade_module_id: rendered.facade_module_id.clone(),
          module_ids: rendered.module_ids.clone(),
          exports: rendered.exports.clone(),
          filename: rendered.filename.clone(),
          modules,
          imports: rendered.imports.clone(),
          dynamic_imports: rendered.dynamic_imports.clone(),
        });
      }
      if let Some(cache) = optimized.property_cache {
        property_mangle_cache.set(cache).expect("one property-mangled chunk");
      }
    }
    let allocator_pool = AllocatorPool::new(rayon::current_num_threads());
    let source_type = options.format.source_type().with_jsx(true);

    chunks.par_iter_mut().try_for_each(|chunk| -> BuildResult<()> {
      if !is_minifiable_ecma_chunk(chunk) {
        return Ok(());
      }
      // This chunk was already optimized as a retained AST.
      if ast_dce_chunks.contains(&chunk.originate_from)
        && !deferred.replaced.contains(&chunk.originate_from)
        && !deferred.rendered_text.contains(&chunk.originate_from)
      {
        #[cfg(feature = "testing")]
        super::chunk_ast_dce::record_coverage(
          options,
          "late_minify",
          "skipped_ast",
          Some(chunk.preliminary_filename.as_str()),
        );
        return Ok(());
      }
      let replaced = deferred.replaced.contains(&chunk.originate_from);
      let addon_text = deferred.rendered_text.contains(&chunk.originate_from);
      assert!(
        replaced || addon_text,
        "chunk AST pipeline has no admitted input for {:?}",
        chunk.preliminary_filename
      );
      #[cfg(feature = "testing")]
      super::chunk_ast_dce::record_coverage(
        options,
        "late_minify",
        if replaced { "hook_text" } else { "addon_text" },
        Some(chunk.preliminary_filename.as_str()),
      );
      let codegen_options = CodegenOptions {
        minify: remove_whitespace,
        ascii_only: options.charset.is_ascii() || ascii_only,
        comments: CommentOptions {
          normal: !remove_whitespace,
          jsdoc: options.comments.jsdoc && !remove_whitespace,
          annotation: options.comments.annotation,
          legal: if options.comments.legal || !remove_whitespace {
            codegen::LegalComment::Inline
          } else {
            codegen::LegalComment::None
          },
        },
        ..CodegenOptions::default()
      };

      let allocator_guard = allocator_pool.get();
      // The minify map borrows the pre-minify `chunk.content` (as `sourcesContent`,
      // which the collapse discards), so collapse before swapping in the minified
      // content instead of paying an `into_owned` copy of the whole chunk text.
      let (minified_content, collapsed_map) = {
        // TODO: Do we need to ensure `chunk.preliminary_filename` to be absolute path?
        let (minified_content, new_map, chunk_property_mangle_cache) = optimize_final_text(
          &allocator_guard,
          chunk.content.try_as_inner_str()?,
          source_type,
          chunk.map.is_some(),
          chunk.preliminary_filename.as_str(),
          compress,
          minify_options.clone(),
          codegen_options,
        )?;
        if let Some(cache) = chunk_property_mangle_cache {
          property_mangle_cache
            .set(cache)
            .expect("property mangling is limited to one JavaScript chunk");
        }
        let t = Timer::start();
        let collapsed_map = match (&chunk.map, &new_map) {
          (Some(origin_map), Some(new_map)) => Some(collapse_sourcemaps(&[origin_map, new_map])),
          _ => {
            // TODO: Map is dirty. Should we reset the `chunk.map` to `None`?
            None
          }
        };
        t.stop(Phase::MinifyCollapse);
        (minified_content, collapsed_map)
      };
      chunk.content = minified_content.into();
      if let Some(map) = collapsed_map {
        chunk.map = Some(map);
      }
      Ok(())
    })?;

    Ok(property_mangle_cache.into_inner().map(|cache| {
      cache
        .into_iter()
        .map(|(original, target)| {
          (original.into_string(), target.map(oxc_str::CompactStr::into_string))
        })
        .collect()
    }))
  }

  /// Checks the final output after `generateBundle`.
  ///
  /// Plugins can emit prebuilt chunks after `minify_chunks`, so the early check alone is not
  /// enough.
  pub(crate) fn validate_mangle_properties_output(
    options: &NormalizedBundlerOptions,
    outputs: &[Output],
  ) -> BuildResult<()> {
    let Some((_, minify_options, _, _)) = chunk_minify_options(&options.minify) else {
      return Ok(());
    };
    if minify_options.mangle_properties.is_some()
      && outputs
        .iter()
        .filter(|output| {
          matches!(output, Output::Chunk(chunk) if !test_d_ts_pattern(chunk.filename.as_str()))
        })
        .take(2)
        .count()
        > 1
    {
      return Err(
        BuildDiagnostic::invalid_option(InvalidOptionType::ManglePropertiesWithMultipleChunks)
          .into(),
      );
    }
    Ok(())
  }
}

/// Parse actual hook/addon text into the shared AST optimizer. The map borrows this input until composition.
#[expect(clippy::too_many_arguments)]
fn optimize_final_text<'a>(
  allocator: &'a oxc_allocator::Allocator,
  source_text: &'a str,
  source_type: oxc::span::SourceType,
  enable_sourcemap: bool,
  filename: &str,
  compress: bool,
  minify_options: OxcMinifierOptions,
  codegen_options: CodegenOptions,
) -> BuildResult<(
  String,
  Option<oxc_sourcemap::SourceMap<'a>>,
  Option<oxc::minifier::ManglePropertyCache>,
)> {
  use oxc::{
    codegen::Codegen,
    parser::{ParseOptions, Parser},
  };
  let t = Timer::start();
  let ret = Parser::new(allocator, source_text, source_type)
    .with_options(ParseOptions { preserve_parens: false, ..ParseOptions::default() })
    .parse();
  if ret.fatal_error || !ret.diagnostics.is_empty() {
    return Err(
      BuildDiagnostic::from_oxc_diagnostics(
        ret.diagnostics,
        &arcstr::ArcStr::from(source_text),
        filename,
        rolldown_error::Severity::Error,
        rolldown_error::EventKind::ParseError,
      )
      .into(),
    );
  }
  let mut program = ret.program;
  t.stop(Phase::MinifyParse);
  let t = Timer::start();
  let ret = EcmaCompiler::minify_program(allocator, &mut program, minify_options, compress, false);
  t.stop(Phase::MinifyDce);
  let t = Timer::start();
  let property_mangle_cache = ret.property_mangle_cache;
  let ret = Codegen::new()
    .with_options(CodegenOptions {
      source_map_path: enable_sourcemap.then(|| std::path::PathBuf::from(filename)),
      ..codegen_options
    })
    .with_scoping(ret.scoping)
    .with_private_member_mappings(ret.class_private_mappings)
    .build(&program);
  t.stop(Phase::MinifyCodegen);
  Ok((ret.code, ret.map, property_mangle_cache))
}

fn chunk_minify_options(
  options: &MinifyOptions,
) -> Option<(bool, &OxcMinifierOptions, bool, bool)> {
  match options {
    MinifyOptions::Disabled => None,
    MinifyOptions::DeadCodeEliminationOnly(options) => Some((false, options, false, false)),
    MinifyOptions::Enabled(options) => {
      Some((true, &options.options, options.remove_whitespace, options.ascii_only))
    }
  }
}

fn is_minifiable_ecma_chunk(chunk: &InstantiatedChunk) -> bool {
  matches!(chunk.kind, InstantiationKind::Ecma(_))
    && !test_d_ts_pattern(chunk.preliminary_filename.as_str())
}

fn test_d_ts_pattern(input: &str) -> bool {
  input.ends_with(".d.ts") || input.ends_with(".d.cts") || input.ends_with(".d.mts")
}

#[cfg(test)]
mod tests {
  use super::*;

  fn chunk(id: u32, filename: &str, ecma: bool) -> InstantiatedChunk {
    use rolldown_common::{EcmaAssetMeta, Modules, PreliminaryFilename, RollupRenderedChunk};

    let filename: arcstr::ArcStr = filename.into();
    let preliminary_filename = PreliminaryFilename::new(filename.clone(), None);
    InstantiatedChunk {
      originate_from: ChunkIdx::from_raw(id),
      content: "export const value = 1 + 2;".to_owned().into(),
      map: Some(rolldown_sourcemap::empty_sourcemap()),
      kind: if ecma {
        EcmaAssetMeta {
          rendered_chunk: std::sync::Arc::new(RollupRenderedChunk {
            originate_from: ChunkIdx::from_raw(id),
            name: "test".into(),
            is_entry: true,
            is_dynamic_entry: false,
            facade_module_id: None,
            module_ids: vec![],
            exports: vec![],
            filename,
            modules: Modules { keys: vec![], values: vec![] },
            imports: vec![],
            dynamic_imports: vec![],
          }),
          debug_id: 0,
          imports: vec![],
          dynamic_imports: vec![],
          sourcemap_filename: None,
          file_dir: std::path::PathBuf::default(),
          preliminary_filename: preliminary_filename.clone(),
        }
        .into()
      } else {
        InstantiationKind::None
      },
      augment_chunk_hash: None,
      preliminary_filename,
      preliminary_sourcemap_filename: None,
      post_banner: None,
      post_footer: None,
    }
  }

  fn minify_options() -> NormalizedBundlerOptions {
    let mut options = NormalizedBundlerOptions::default();
    options.minify = rolldown_common::RawMinifyOptions::Bool(true).normalize(&options);
    options
  }

  #[test]
  fn property_validation_precedes_chunk_input_invariant() {
    let mut options = minify_options();
    let MinifyOptions::Enabled(ref mut minify) = options.minify else { unreachable!() };
    minify.options.mangle_properties =
      Some(oxc::minifier::ManglePropertiesOptions::from_pattern("^private_").unwrap());
    let mut chunks =
      [chunk(0, "first.js", true), chunk(1, "second.js", true)].into_iter().collect();
    let error = GenerateStage::minify_chunks(
      &options,
      &mut chunks,
      &FxHashSet::default(),
      None,
      super::super::chunk_ast_dce::DeferredChunks::default(),
      &rolldown_common::ModuleTable::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("property mangling is currently supported only"));
  }

  #[test]
  fn test_edge_cases() {
    assert!(test_d_ts_pattern(".d.ts"));
    assert!(test_d_ts_pattern(".d.cts"));
    assert!(test_d_ts_pattern(".d.mts"));
  }

  #[test]
  fn test_invalid_patterns_wrong_extension() {
    assert!(!test_d_ts_pattern(".d.tsx"));
    assert!(!test_d_ts_pattern(".d.ctsx"));
    assert!(!test_d_ts_pattern(".d.mtsx"));
    assert!(!test_d_ts_pattern(".d.cjs"));
  }

  #[test]
  fn test_invalid_patterns_missing_d() {
    assert!(!test_d_ts_pattern(".c.ts"));
    assert!(!test_d_ts_pattern(".m.ts"));
    assert!(!test_d_ts_pattern("abc.ts"));
    assert!(!test_d_ts_pattern("d.ts"));
  }

  #[test]
  fn test_invalid_patterns_extra_chars() {
    assert!(!test_d_ts_pattern(".da.ts"));
    assert!(!test_d_ts_pattern(".d.ats"));
    assert!(!test_d_ts_pattern(".d.tsa"));
  }

  #[test]
  fn test_invalid_patterns_short_input() {
    assert!(!test_d_ts_pattern(".d"));
    assert!(!test_d_ts_pattern(".t"));
    assert!(!test_d_ts_pattern("."));
    assert!(!test_d_ts_pattern(""));
    assert!(!test_d_ts_pattern(".ts")); // added test
  }
}
