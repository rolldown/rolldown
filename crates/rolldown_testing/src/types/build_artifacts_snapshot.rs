use std::borrow::Cow;
use std::path::Path;

use rolldown::BundleOutput;
use rolldown_common::Output;
use rolldown_error::{BuildDiagnostic, DiagnosticOptions};
use rolldown_sourcemap::SourcemapVisualizer;
use rolldown_testing_config::TestMeta;
use sugar_path::SugarPath;

use super::{BuildRoundOutput, SnapshotSection};
use crate::utils::{snapshot as snapshot_utils, tweak_snapshot};

#[derive(Default)]
pub struct BuildArtifactsSnapshot {
  pub builds: Vec<BuildRoundOutput>,
}

impl BuildArtifactsSnapshot {
  pub fn render(self, test_meta: &TestMeta) -> String {
    let mut root_section = SnapshotSection::root();

    for build_round in self.builds {
      if !build_round.overwritten_test_meta_snapshot {
        continue;
      }
      let mut build_round_sections = vec![];
      if let Some(initial_output) = build_round.initial_output {
        let cwd = build_round.cwd.as_ref().unwrap();
        match initial_output {
          Ok(bundle_output) => {
            let mut assets = bundle_output.assets;
            assets.sort_by_key(|c| c.filename().to_string());

            // Render `# Warnings`
            build_round_sections.extend(Self::create_warning_section(bundle_output.warnings, cwd));

            // Render `# Assets`
            if let Some(assets_section) = Self::create_assets_section(test_meta, &assets) {
              build_round_sections.push(assets_section);
            }

            // Render `# Output Stats` (if enabled)
            if let Some(output_stats_section) =
              Self::create_output_stats_section(test_meta, &assets)
            {
              build_round_sections.push(output_stats_section);
            }

            // Render `# Sourcemap Visualizer` (if enabled)
            if let Some(sourcemap_section) =
              Self::create_sourcemap_visualizer_section(test_meta, &assets)
            {
              build_round_sections.push(sourcemap_section);
            }
          }

          Err(errs) => {
            // Render `# Errors` (if build failed)
            build_round_sections.push(snapshot_utils::create_error_section(errs.into_vec(), cwd));
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

  pub(crate) fn create_warning_section(
    warnings: Vec<BuildDiagnostic>,
    cwd: &Path,
  ) -> Option<SnapshotSection> {
    if warnings.is_empty() {
      return None;
    }
    let mut warnings_section = SnapshotSection::with_title("warnings");
    let diagnostics = warnings
      .into_iter()
      .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

    let rendered_diagnostics = snapshot_utils::render_diagnostics(diagnostics);

    for diag in rendered_diagnostics {
      warnings_section.add_child(diag);
    }
    Some(warnings_section)
  }

  pub fn create_assets_section(test_meta: &TestMeta, assets: &[Output]) -> Option<SnapshotSection> {
    if assets.is_empty() {
      None
    } else {
      let mut assets_section = SnapshotSection::with_title("Assets");

      for asset in assets {
        let filename = asset.filename();
        let file_ext = snapshot_utils::get_normalized_extension(filename.as_path());

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
      Some(assets_section)
    }
  }

  pub(crate) fn create_output_stats_section(
    test_meta: &TestMeta,
    assets: &[Output],
  ) -> Option<SnapshotSection> {
    if !test_meta.snapshot_output_stats {
      return None;
    }

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
    Some(output_stats)
  }

  pub(crate) fn create_sourcemap_visualizer_section(
    test_meta: &TestMeta,
    assets: &[Output],
  ) -> Option<SnapshotSection> {
    if !test_meta.visualize_sourcemap {
      return None;
    }

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
    Some(sourcemap_section)
  }

  pub(crate) fn create_bundle_output_sections(
    test_meta: &TestMeta,
    bundle_output: BundleOutput,
    cwd: &Path,
  ) -> Vec<SnapshotSection> {
    let mut sections = Vec::new();

    let mut assets = bundle_output.assets;
    assets.sort_by_key(|c| c.filename().to_string());

    // Render `# Warnings`
    sections.extend(Self::create_warning_section(bundle_output.warnings, cwd));

    // Render `# Assets`
    if let Some(assets_section) = Self::create_assets_section(test_meta, &assets) {
      sections.push(assets_section);
    }

    // Render `# Output Stats` (if enabled)
    if let Some(output_stats_section) = Self::create_output_stats_section(test_meta, &assets) {
      sections.push(output_stats_section);
    }

    // Render `# Sourcemap Visualizer` (if enabled)
    if let Some(sourcemap_section) = Self::create_sourcemap_visualizer_section(test_meta, &assets) {
      sections.push(sourcemap_section);
    }

    sections
  }
}
