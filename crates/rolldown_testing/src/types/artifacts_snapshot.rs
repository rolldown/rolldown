use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::Path;

use rolldown::BundleOutput;
use rolldown_common::{HmrUpdate, Output};
use rolldown_error::{BuildDiagnostic, BuildResult, DiagnosticOptions};
use rolldown_sourcemap::SourcemapVisualizer;
use rolldown_testing_config::TestMeta;
use sugar_path::SugarPath;

use super::{BuildRoundOutput, SnapshotSection};
use crate::utils::tweak_snapshot;

#[derive(Default)]
pub struct ArtifactsSnapshot {
  pub builds: Vec<BuildRoundOutput>,
}

impl ArtifactsSnapshot {
  pub fn render(self, test_meta: &TestMeta) -> String {
    let mut root_section = SnapshotSection::root();

    for mut build_round in self.builds {
      if !build_round.overwritten_test_meta_snapshot {
        continue;
      }
      let mut build_round_sections = vec![];

      if let Some(initial_output) = build_round.initial_output {
        match initial_output {
          Ok(bundle_output) => {
            build_round_sections.extend(Self::create_bundle_output_sections(
              test_meta,
              bundle_output,
              build_round.cwd.as_ref().unwrap(),
            ));
          }

          Err(errs) => {
            build_round_sections
              .push(Self::create_error_section(errs.into_vec(), build_round.cwd.as_ref().unwrap()));
          }
        }
      }

      if !build_round.hmr_updates_by_steps.is_empty() {
        for (step, hmr_result) in build_round.hmr_updates_by_steps.into_iter().enumerate() {
          match hmr_result {
            Ok((hmr_updates, _changed_files)) => {
              for hmr_update in hmr_updates {
                let hmr_section = Self::create_hmr_output_section(
                  test_meta,
                  step,
                  &hmr_update.update,
                  vec![],
                  &mut build_round.rebuild_results,
                  build_round.cwd.as_ref().unwrap(),
                );
                build_round_sections.push(hmr_section);
              }
            }
            Err(errs) => {
              let hmr_section = Self::create_hmr_error_section(
                step,
                errs.into_vec(),
                build_round.cwd.as_ref().unwrap(),
              );
              build_round_sections.push(hmr_section);
            }
          }
        }
      }

      if let Some(debug_title) = &build_round.debug_title {
        let mut build_round_section =
          SnapshotSection::with_title(format!("Variant: {debug_title}"));
        build_round_section.children = build_round_sections;
        root_section.add_child(build_round_section);
      } else {
        root_section.children.extend(build_round_sections);
      }
    }

    root_section.render()
  }

  fn create_error_section(errs: Vec<BuildDiagnostic>, cwd: &Path) -> SnapshotSection {
    let mut errors = errs;

    let mut errors_section = SnapshotSection::with_title("Errors");
    errors.sort_by_key(|e| e.kind().to_string());

    let diagnostics = errors
      .into_iter()
      .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

    let mut rendered_diagnostics = diagnostics
      .map(|(code, diagnostic)| {
        let mut child = SnapshotSection::with_title(code.to_string());
        child.add_content("```text\n");
        child.add_content(&diagnostic.to_string());
        child.add_content("\n```");
        child
      })
      .collect::<Vec<_>>();

    // FIXME: For compatibility with previous snapshots, we still sort by title first. Will use a performant way later.
    rendered_diagnostics.sort_by_cached_key(SnapshotSection::render);

    for diag in rendered_diagnostics {
      errors_section.add_child(diag);
    }
    errors_section
  }

  fn create_bundle_output_sections(
    test_meta: &TestMeta,
    bundle_output: BundleOutput,
    cwd: &Path,
  ) -> Vec<SnapshotSection> {
    let mut sections = Vec::new();

    // Warnings section
    let warnings = bundle_output.warnings;
    if !warnings.is_empty() {
      let mut warnings_section = SnapshotSection::with_title("warnings");
      let diagnostics = warnings
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          let mut child = SnapshotSection::with_title(code.to_string());
          child.add_content("```text\n");
          child.add_content(&diagnostic.to_string());
          child.add_content("\n```");
          child
        })
        .collect::<Vec<_>>();

      // FIXME: use a performant sorting technique.
      rendered_diagnostics.sort_by_cached_key(SnapshotSection::render);

      for diag in rendered_diagnostics {
        warnings_section.add_child(diag);
      }
      sections.push(warnings_section);
    }

    // Assets section
    let mut assets = bundle_output.assets;
    if !assets.is_empty() {
      let mut assets_section = SnapshotSection::with_title("Assets");
      assets.sort_by_key(|c| c.filename().to_string());

      for asset in &assets {
        let filename = asset.filename();
        let file_ext = filename.as_path().extension().and_then(OsStr::to_str).map_or(
          "unknown",
          |ext| match ext {
            "mjs" | "cjs" => "js",
            _ => ext,
          },
        );

        match asset {
          Output::Chunk(output_chunk) => {
            let content = &output_chunk.code;
            let content = tweak_snapshot(content, test_meta.hidden_runtime_module, true);

            let mut asset_child = SnapshotSection::with_title(asset.filename().to_string());
            asset_child.add_content(&format!("```{file_ext}\n"));
            asset_child.add_content(&content);
            asset_child.add_content("\n```");
            assets_section.add_child(asset_child);
          }
          Output::Asset(output_asset) => {
            if file_ext == "map" {
              // Skip sourcemap for now
              continue;
            }
            match &output_asset.source {
              rolldown_common::StrOrBytes::Str(content) => {
                let mut asset_child = SnapshotSection::with_title(asset.filename().to_string());
                asset_child.add_content(&format!("```{file_ext}\n"));
                asset_child.add_content(content);
                asset_child.add_content("\n```");
                assets_section.add_child(asset_child);
              }
              rolldown_common::StrOrBytes::Bytes(bytes) => {
                let mut asset_child = SnapshotSection::with_title(asset.filename().to_string());
                if test_meta.snapshot_bytes {
                  asset_child.add_content(&format!("```{file_ext}\n"));
                  asset_child.add_content(&String::from_utf8_lossy(bytes));
                  asset_child.add_content("\n```");
                }
                assets_section.add_child(asset_child);
              }
            }
          }
        }
      }
      sections.push(assets_section);
    }

    // Output Stats section
    if test_meta.snapshot_output_stats {
      let mut output_stats = SnapshotSection::with_title("Output Stats");
      let stats = assets
        .iter()
        .flat_map(|asset| match asset {
          Output::Chunk(chunk) => {
            vec![Cow::Owned(format!(
              "- {}, is_entry {}, is_dynamic_entry {}, exports {:?}",
              chunk.filename.as_str(),
              chunk.is_entry,
              chunk.is_dynamic_entry,
              chunk.exports.iter().map(ToString::to_string).collect::<Vec<_>>()
            ))]
          }
          Output::Asset(_) => vec![],
        })
        .collect::<Vec<_>>()
        .join("\n");
      output_stats.add_content(&stats);
      sections.push(output_stats);
    }

    // Sourcemap Visualizer section
    if test_meta.visualize_sourcemap {
      let mut sourcemap_section = SnapshotSection::with_title("Sourcemap Visualizer");
      sourcemap_section.add_content("```\n");
      let visualizer_result = assets
        .iter()
        .filter_map(|asset| match asset {
          Output::Chunk(chunk) => chunk
            .map
            .as_ref()
            .map(|sourcemap| SourcemapVisualizer::new(&chunk.code, sourcemap).get_text()),
          Output::Asset(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
      sourcemap_section.add_content(&visualizer_result);
      sourcemap_section.add_content("```");
      sections.push(sourcemap_section);
    }
    sections
  }

  fn create_hmr_error_section(
    step: usize,
    errs: Vec<BuildDiagnostic>,
    cwd: &Path,
  ) -> SnapshotSection {
    let mut hmr_section = SnapshotSection::with_title(format!("HMR Step {step}"));
    let errors_section = Self::create_error_section(errs, cwd);
    hmr_section.add_child(errors_section);
    hmr_section
  }

  fn create_hmr_output_section(
    test_meta: &TestMeta,
    step: usize,
    hmr_update: &HmrUpdate,
    errs: Vec<BuildDiagnostic>,
    build_results: &mut Vec<BuildResult<BundleOutput>>,
    cwd: &Path,
  ) -> SnapshotSection {
    let mut hmr_section = SnapshotSection::with_title(format!("HMR Step {step}"));

    // Errors section
    let errors = errs;
    if !errors.is_empty() {
      let errors_section = Self::create_error_section(errors, cwd);
      hmr_section.add_child(errors_section);
    }

    // Code section
    match hmr_update {
      HmrUpdate::Patch(hmr_patch) if !hmr_patch.code.is_empty() => {
        let mut code_section = SnapshotSection::with_title("Code");
        let file_ext = hmr_patch.filename.as_path().extension().and_then(OsStr::to_str).map_or(
          "unknown",
          |ext| match ext {
            "mjs" | "cjs" => "js",
            _ => ext,
          },
        );
        code_section.add_content(&format!("```{file_ext}\n"));
        code_section.add_content(&tweak_snapshot(
          &hmr_patch.code,
          test_meta.hidden_runtime_module,
          true,
        ));
        code_section.add_content("\n```");
        hmr_section.add_child(code_section);
      }
      HmrUpdate::FullReload { .. } => {
        let build_result = build_results.remove(0);
        match build_result {
          Ok(build_output) => {
            let bundle_output_sections =
              Self::create_bundle_output_sections(test_meta, build_output, cwd);
            hmr_section.children.extend(bundle_output_sections);
          }
          Err(errs) => {
            let errors_section = Self::create_error_section(errs.into_vec(), cwd);
            hmr_section.add_child(errors_section);
          }
        }
      }
      _ => {}
    }

    // Meta section
    let mut meta_section = SnapshotSection::with_title("Meta");
    meta_section.add_content(&format!(
      "- update type: {}",
      match hmr_update {
        HmrUpdate::Patch(_) => "patch",
        HmrUpdate::FullReload { .. } => "full-reload",
        HmrUpdate::Noop => "noop",
      }
    ));

    match hmr_update {
      HmrUpdate::Patch(hmr_patch) => {
        let mut boundaries = SnapshotSection::with_title("Hmr Boundaries");
        let meta = hmr_patch
          .hmr_boundaries
          .iter()
          .map(|boundary| {
            format!(
              "- boundary: {}, accepted_via: {}",
              boundary.boundary.as_str(),
              boundary.accepted_via.as_str()
            )
          })
          .collect::<Vec<_>>();
        boundaries.add_content(&meta.join("\n"));
        meta_section.add_child(boundaries);
      }
      HmrUpdate::FullReload { reason } => {
        meta_section.add_content(&format!("\n- reason: {reason}"));
      }
      HmrUpdate::Noop => {}
    }

    hmr_section.add_child(meta_section);

    hmr_section
  }
}
