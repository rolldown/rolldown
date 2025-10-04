use std::borrow::Cow;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::Path;

use rolldown::BundleOutput;
use rolldown_common::{HmrUpdate, Output};
use rolldown_error::{BuildDiagnostic, DiagnosticOptions};
use rolldown_sourcemap::SourcemapVisualizer;
use rolldown_testing_config::TestMeta;
use sugar_path::SugarPath;

use super::BuildRoundOutput;
use crate::utils::tweak_snapshot;

#[derive(Default)]
pub struct ArtifactsSnapshot {
  pub builds: Vec<BuildRoundOutput>,
}

impl ArtifactsSnapshot {
  pub fn render(self, test_meta: &TestMeta) -> String {
    let mut ret = String::new();
    for mut build_snapshot in self.builds {
      if !build_snapshot.overwritten_test_meta_snapshot {
        continue;
      }

      if let Some(debug_title) = &build_snapshot.debug_title {
        ret.push_str("\n---\n\n");
        ret.push_str("Variant: ");
        ret.push_str(debug_title);
        ret.push_str("\n\n");
      }

      if let Some(initial_output) = build_snapshot.initial_output {
        match initial_output {
          Ok(bundle_output) => {
            ret.push_str(&Self::render_bundle_output_to_string(
              test_meta,
              bundle_output,
              vec![],
              build_snapshot.cwd.as_ref().unwrap(),
              0,
            ));
          }
          Err(errs) => {
            ret.push_str(&Self::render_bundle_output_to_string(
              test_meta,
              BundleOutput::default(),
              errs.into_vec(),
              build_snapshot.cwd.as_ref().unwrap(),
              0,
            ));
          }
        }
      }

      if !build_snapshot.hmr_updates_by_steps.is_empty() {
        for (step, (hmr_updates, _changed_files)) in
          build_snapshot.hmr_updates_by_steps.iter().enumerate()
        {
          for hmr_update in hmr_updates {
            let snapshot_content = Self::render_hmr_output_to_string(
              test_meta,
              step,
              &hmr_update.update,
              vec![],
              &mut build_snapshot.rebuild_outputs,
              build_snapshot.cwd.as_ref().unwrap(),
            );
            ret.push_str(&snapshot_content);
          }
        }
      }
    }

    ret
  }

  #[expect(clippy::too_many_lines)]
  #[expect(clippy::if_not_else)]
  fn render_bundle_output_to_string(
    test_meta: &TestMeta,
    bundle_output: BundleOutput,
    errs: Vec<BuildDiagnostic>,
    cwd: &Path,
    heading_level: usize,
  ) -> String {
    let heading_prefix = "#".repeat(heading_level);
    let mut errors = errs;
    let errors_section = if !errors.is_empty() {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}# Errors\n\n").unwrap();
      errors.sort_by_key(|e| e.kind().to_string());
      let diagnostics = errors
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("{heading_prefix}## {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
          .join("\n")
        })
        .collect::<Vec<_>>();
      rendered_diagnostics.sort();
      let rendered = rendered_diagnostics.join("\n");
      snapshot.push_str(&rendered);
      snapshot
    } else {
      String::default()
    };

    let warnings = bundle_output.warnings;
    let warnings_section = if !warnings.is_empty() {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}# warnings\n\n").unwrap();
      let diagnostics = warnings
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));
      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("{heading_prefix}## {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
          .join("\n")
        })
        .collect::<Vec<_>>();

      // Make the snapshot consistent
      rendered_diagnostics.sort();
      snapshot.push_str(&rendered_diagnostics.join("\n"));
      snapshot
    } else {
      String::new()
    };

    let mut assets = bundle_output.assets;

    let assets_section = if !assets.is_empty() {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}# Assets\n\n").unwrap();
      assets.sort_by_key(|c| c.filename().to_string());
      let artifacts = assets
        .iter()
        .filter_map(|asset| {
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

              Some(vec![
                Cow::Owned(format!("{heading_prefix}## {}\n", asset.filename())),
                Cow::Owned(format!("```{file_ext}")),
                content,
                "```".into(),
              ])
            }
            Output::Asset(output_asset) => {
              if file_ext == "map" {
                // Skip sourcemap for now
                return None;
              }
              match &output_asset.source {
                rolldown_common::StrOrBytes::Str(content) => Some(vec![
                  Cow::Owned(format!("{heading_prefix}## {}\n", asset.filename())),
                  Cow::Owned(format!("```{file_ext}")),
                  Cow::Borrowed(content),
                  "```".into(),
                ]),
                rolldown_common::StrOrBytes::Bytes(bytes) => {
                  let mut ret =
                    vec![Cow::Owned(format!("{heading_prefix}## {}\n", asset.filename()))];
                  if test_meta.snapshot_bytes {
                    ret.extend([
                      Cow::Owned(format!("```{file_ext}")),
                      String::from_utf8_lossy(bytes),
                      "```".into(),
                    ]);
                  }
                  Some(ret)
                }
              }
            }
          }
        })
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
      snapshot.push_str(&artifacts);
      snapshot
    } else {
      String::new()
    };

    let output_stats_section = if test_meta.snapshot_output_stats {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}## Output Stats\n\n").unwrap();
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
      snapshot.push_str(&stats);
      snapshot
    } else {
      String::new()
    };

    let visualize_sourcemap_section = if test_meta.visualize_sourcemap {
      let mut snapshot = String::new();
      write!(snapshot, "{heading_prefix}# Sourcemap Visualizer\n\n").unwrap();
      snapshot.push_str("```\n");
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
      snapshot.push_str(&visualizer_result);
      snapshot.push_str("```");
      snapshot
    } else {
      String::new()
    };
    [
      errors_section,
      warnings_section,
      assets_section,
      output_stats_section,
      visualize_sourcemap_section,
    ]
    .join("\n")
    .trim()
    .to_owned()
  }

  #[expect(clippy::if_not_else)]
  fn render_hmr_output_to_string(
    test_meta: &TestMeta,
    step: usize,
    hmr_update: &HmrUpdate,
    errs: Vec<BuildDiagnostic>,
    build_outputs: &mut Vec<BundleOutput>,
    cwd: &Path,
  ) -> String {
    let mut errors = errs;
    let errors_section = if !errors.is_empty() {
      let mut snapshot = String::new();
      snapshot.push_str("## Errors\n\n");
      errors.sort_by_key(|e| e.kind().to_string());
      let diagnostics = errors
        .into_iter()
        .map(|e| (e.kind(), e.to_diagnostic_with(&DiagnosticOptions { cwd: cwd.to_path_buf() })));

      let mut rendered_diagnostics = diagnostics
        .map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("### {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
          .join("\n")
        })
        .collect::<Vec<_>>();
      rendered_diagnostics.sort();
      let rendered = rendered_diagnostics.join("\n");
      snapshot.push_str(&rendered);
      snapshot
    } else {
      String::default()
    };

    let code_section = match hmr_update {
      HmrUpdate::Patch(hmr_patch) if !hmr_patch.code.is_empty() => {
        let mut snapshot = String::new();
        write!(snapshot, "## Code\n\n").unwrap();
        let file_ext = hmr_patch.filename.as_path().extension().and_then(OsStr::to_str).map_or(
          "unknown",
          |ext| match ext {
            "mjs" | "cjs" => "js",
            _ => ext,
          },
        );
        writeln!(snapshot, "```{file_ext}").unwrap();
        snapshot.push_str(&tweak_snapshot(&hmr_patch.code, test_meta.hidden_runtime_module, true));
        snapshot.push_str("\n```");
        snapshot
      }
      HmrUpdate::FullReload { .. } => {
        let bundle_output = build_outputs.remove(0);
        Self::render_bundle_output_to_string(test_meta, bundle_output, vec![], cwd, 1)
      }
      _ => String::new(),
    };

    let meta_section = {
      let mut snapshot = String::new();
      snapshot.push_str("## Meta\n\n");
      writeln!(
        snapshot,
        "- update type: {}",
        match hmr_update {
          HmrUpdate::Patch(_) => "patch",
          HmrUpdate::FullReload { .. } => "full-reload",
          HmrUpdate::Noop => "noop",
        }
      )
      .unwrap();

      match hmr_update {
        HmrUpdate::Patch(hmr_patch) => {
          write!(snapshot, "### Hmr Boundaries\n\n").unwrap();
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
          snapshot.push_str(&meta.join("\n"));
        }
        HmrUpdate::FullReload { reason } => {
          writeln!(snapshot, "- reason: {reason}").unwrap();
        }
        HmrUpdate::Noop => {}
      }

      snapshot
    };

    "\n".to_owned()
      + [format!("# HMR Step {step}"), errors_section, code_section, meta_section].join("\n").trim()
  }
}
