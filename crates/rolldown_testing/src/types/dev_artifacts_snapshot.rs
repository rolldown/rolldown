use std::path::Path;

use rolldown::BundleOutput;
use rolldown_common::HmrUpdate;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_testing_config::TestMeta;
use sugar_path::SugarPath;

use super::{DevRoundOutput, SnapshotSection};
use crate::types::build_artifacts_snapshot::BuildArtifactsSnapshot;
use crate::utils::{snapshot as snapshot_utils, tweak_snapshot};

#[derive(Default)]
pub struct DevArtifactsSnapshot {
  pub builds: Vec<DevRoundOutput>,
}

impl DevArtifactsSnapshot {
  pub fn render(self, test_meta: &TestMeta) -> String {
    let mut root_section = SnapshotSection::root();

    for mut build_round in self.builds {
      if !build_round.overwritten_test_meta_snapshot {
        continue;
      }
      let mut build_round_sections = vec![];
      let cwd = build_round.cwd.as_ref().unwrap();

      if let Some(initial_output) = build_round.initial_output {
        match initial_output {
          Ok(bundle_output) => {
            let mut assets = bundle_output.assets;
            assets.sort_by_key(|c| c.filename().to_string());

            // Render `# Warnings`
            build_round_sections
              .extend(BuildArtifactsSnapshot::create_warning_section(bundle_output.warnings, cwd));

            // Render `# Assets`
            if let Some(assets_section) =
              BuildArtifactsSnapshot::create_assets_section(test_meta, &assets)
            {
              build_round_sections.push(assets_section);
            }
          }
          Err(errs) => {
            // Render `# Errors` (if build failed)
            build_round_sections.push(snapshot_utils::create_error_section(errs.into_vec(), cwd));
          }
        }
      }

      // Render `# HMR Steps N`
      for (step, hmr_result) in build_round.hmr_updates_by_steps.into_iter().enumerate() {
        let hmr_sections = Self::create_hmr_step_sections(
          test_meta,
          step,
          hmr_result,
          &mut build_round.rebuild_results,
          cwd,
        );
        build_round_sections.extend(hmr_sections);
      }

      // Wrap in variant section if needed
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

  fn create_hmr_error_section(
    step: usize,
    errs: Vec<BuildDiagnostic>,
    cwd: &Path,
  ) -> SnapshotSection {
    let mut hmr_section = SnapshotSection::with_title(format!("HMR Step {step}"));
    let errors_section = snapshot_utils::create_error_section(errs, cwd);
    hmr_section.add_child(errors_section);
    hmr_section
  }

  fn create_hmr_step_sections(
    test_meta: &TestMeta,
    step: usize,
    hmr_result: BuildResult<(Vec<rolldown_common::ClientHmrUpdate>, Vec<String>)>,
    rebuild_results: &mut Vec<BuildResult<BundleOutput>>,
    cwd: &Path,
  ) -> Vec<SnapshotSection> {
    match hmr_result {
      Ok((hmr_updates, _changed_files)) => hmr_updates
        .into_iter()
        .map(|hmr_update| {
          Self::create_hmr_output_section(
            test_meta,
            step,
            &hmr_update.update,
            vec![],
            rebuild_results,
            cwd,
          )
        })
        .collect(),
      Err(errs) => {
        vec![Self::create_hmr_error_section(step, errs.into_vec(), cwd)]
      }
    }
  }

  fn create_hmr_errors_section(errs: Vec<BuildDiagnostic>, cwd: &Path) -> Option<SnapshotSection> {
    if errs.is_empty() { None } else { Some(snapshot_utils::create_error_section(errs, cwd)) }
  }

  fn create_hmr_code_section(
    test_meta: &TestMeta,
    hmr_update: &HmrUpdate,
  ) -> Option<SnapshotSection> {
    match hmr_update {
      HmrUpdate::Patch(hmr_patch) if !hmr_patch.code.is_empty() => {
        let mut code_section = SnapshotSection::with_title("Code");
        let file_ext = snapshot_utils::get_normalized_extension(hmr_patch.filename.as_path());
        code_section.add_content(&format!("```{file_ext}\n"));
        code_section.add_content(&tweak_snapshot(
          &hmr_patch.code,
          test_meta.hidden_runtime_module,
          true,
        ));
        code_section.add_content("\n```");
        Some(code_section)
      }
      _ => None,
    }
  }

  fn create_hmr_full_reload_sections(
    test_meta: &TestMeta,
    hmr_update: &HmrUpdate,
    build_results: &mut Vec<BuildResult<BundleOutput>>,
    cwd: &Path,
  ) -> Option<Vec<SnapshotSection>> {
    match hmr_update {
      HmrUpdate::FullReload { .. } => {
        let build_result = build_results.remove(0);
        match build_result {
          Ok(build_output) => {
            let bundle_output_sections =
              BuildArtifactsSnapshot::create_bundle_output_sections(test_meta, build_output, cwd);
            Some(bundle_output_sections)
          }
          Err(errs) => {
            let errors_section = snapshot_utils::create_error_section(errs.into_vec(), cwd);
            Some(vec![errors_section])
          }
        }
      }
      _ => None,
    }
  }

  fn create_hmr_meta_section(hmr_update: &HmrUpdate) -> SnapshotSection {
    let mut meta_section = SnapshotSection::with_title("Meta");

    // Update type
    meta_section.add_content(&format!(
      "- update type: {}",
      match hmr_update {
        HmrUpdate::Patch(_) => "patch",
        HmrUpdate::FullReload { .. } => "full-reload",
        HmrUpdate::Noop => "noop",
      }
    ));

    // Type-specific metadata
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

    meta_section
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

    // Render `# Errors` (if present)
    if let Some(errors_section) = Self::create_hmr_errors_section(errs, cwd) {
      hmr_section.add_child(errors_section);
    }

    // Render `# Code` (for patches)
    if let Some(code_section) = Self::create_hmr_code_section(test_meta, hmr_update) {
      hmr_section.add_child(code_section);
    }

    // Render `# Assets` (for full reload)
    if let Some(bundle_sections) =
      Self::create_hmr_full_reload_sections(test_meta, hmr_update, build_results, cwd)
    {
      hmr_section.children.extend(bundle_sections);
    }

    // Render `# Meta`
    let meta_section = Self::create_hmr_meta_section(hmr_update);
    hmr_section.add_child(meta_section);

    hmr_section
  }
}
