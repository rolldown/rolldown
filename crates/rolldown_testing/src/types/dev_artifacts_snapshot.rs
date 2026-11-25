use std::path::Path;

use rolldown::BundleOutput;
use rolldown_common::{HmrPullResponse, HmrUpdate};
use rolldown_error::BuildResult;
use rolldown_testing_config::TestMeta;

use super::{DevRoundOutput, SnapshotSection};
use crate::hmr_test_client::HmrClientStepRecord;
use crate::types::build_artifacts_snapshot::BuildArtifactsSnapshot;
use crate::utils::{snapshot as snapshot_utils, tweak_snapshot};

#[derive(Default)]
pub struct DevArtifactsSnapshot {
  pub builds: Vec<DevRoundOutput>,
}

impl DevArtifactsSnapshot {
  pub fn render(self, test_meta: &TestMeta) -> String {
    let mut root_section = SnapshotSection::root();

    for build_round in self.builds {
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
      for (step_index, step_output) in build_round.hmr_steps.into_iter().enumerate() {
        let hmr_sections = Self::create_hmr_step_sections(
          test_meta,
          step_index,
          step_output.hmr_update,
          step_output.client,
          step_output.build_outputs,
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

  fn create_hmr_step_sections(
    test_meta: &TestMeta,
    step: usize,
    hmr_result: BuildResult<(Option<HmrUpdate>, Vec<String>)>,
    client: Option<HmrClientStepRecord>,
    build_outputs: Vec<BuildResult<BundleOutput>>,
    cwd: &Path,
  ) -> Vec<SnapshotSection> {
    let mut step_section = SnapshotSection::with_title(format!("HMR Step {step}"));

    // 1. Render HMR updates (always)
    match hmr_result {
      Ok((hmr_update, _changed_files)) => {
        if let Some(hmr_update) = hmr_update {
          step_section.add_child(Self::create_hmr_meta_section(&hmr_update, cwd));
        }
      }
      Err(errs) => {
        let errors_section = snapshot_utils::create_error_section(errs.into_vec(), cwd);
        step_section.add_child(errors_section);
      }
    }

    // 2. Render what the test client did with the push
    if let Some(client) = client {
      for (index, pull) in client.pulls.into_iter().enumerate() {
        let mut pull_section = SnapshotSection::with_title(format!("Pull {index}"));
        let ids =
          |ids: &[String]| if ids.is_empty() { "(none)".to_owned() } else { ids.join(", ") };
        pull_section.add_content(&format!(
          "- g: {}\n- update: {}\n- executed: {}\n- factories: {}",
          pull.generation,
          ids(&pull.update),
          ids(&pull.executed),
          ids(&pull.factories),
        ));
        match pull.response {
          Ok(HmrPullResponse::Payload(patch)) => {
            let mut code_section = SnapshotSection::with_title("Code");
            code_section.add_content("```js\n");
            // The inline map is long and says little in a snapshot.
            let code = match patch.code.split_once("\n//# sourceMappingURL=data:") {
              Some((code, _)) => format!("{code}\n//# sourceMappingURL=data:<inline map>"),
              None => patch.code,
            };
            code_section.add_content(&tweak_snapshot(&code, test_meta.hidden_runtime_module));
            code_section.add_content("\n```");
            pull_section.add_child(code_section);
          }
          Ok(HmrPullResponse::Reload { reason }) => {
            pull_section.add_content(&format!("\n- reload: {reason}"));
          }
          Err(errs) => {
            pull_section.add_child(snapshot_utils::create_error_section(errs.into_vec(), cwd));
          }
        }
        step_section.add_child(pull_section);
      }
      let mut result_section = SnapshotSection::with_title("Client");
      result_section.add_content(&format!("- result: {}", client.result));
      step_section.add_child(result_section);
    }

    // 3. Render build outputs as children (if any and non-empty)
    for build_output in build_outputs {
      if let Some(section) = Self::create_build_output_section(test_meta, build_output, cwd) {
        step_section.add_child(section);
      }
    }

    vec![step_section]
  }

  fn create_hmr_meta_section(hmr_update: &HmrUpdate, cwd: &Path) -> SnapshotSection {
    let mut meta_section = SnapshotSection::with_title("Meta");

    // Update type
    meta_section.add_content(&format!(
      "- update type: {}",
      match hmr_update {
        HmrUpdate::Changed { .. } => "changed",
        HmrUpdate::FullReload { .. } => "full-reload",
        HmrUpdate::Noop => "noop",
      }
    ));

    let cwd_str = cwd.to_str().unwrap_or("");

    // Type-specific metadata
    match hmr_update {
      HmrUpdate::Changed { changed_ids, stale_ids, generation } => {
        meta_section.add_content(&format!("\n- gen: {generation}"));
        for (title, ids) in [("Changed Ids", changed_ids), ("Stale Ids", stale_ids)] {
          let mut section = SnapshotSection::with_title(title);
          let meta =
            ids.iter().map(|id| format!("- {}", id.replace(cwd_str, "$CWD"))).collect::<Vec<_>>();
          section.add_content(&meta.join("\n"));
          meta_section.add_child(section);
        }
      }
      HmrUpdate::FullReload { reason } => {
        let reason = reason.replace(cwd_str, "$CWD");
        meta_section.add_content(&format!("\n- reason: {reason}"));
      }
      HmrUpdate::Noop => {}
    }

    meta_section
  }

  /// Render build output only (Assets/Errors). Returns None if section would be empty.
  fn create_build_output_section(
    test_meta: &TestMeta,
    build_result: BuildResult<BundleOutput>,
    cwd: &Path,
  ) -> Option<SnapshotSection> {
    match build_result {
      Ok(bundle_output) => {
        let bundle_sections =
          BuildArtifactsSnapshot::create_bundle_output_sections(test_meta, bundle_output, cwd);

        // Return None if no content
        if bundle_sections.is_empty() {
          return None;
        }

        let mut section = SnapshotSection::with_title("Build Output");
        section.children.extend(bundle_sections);
        Some(section)
      }
      Err(errs) => {
        let mut section = SnapshotSection::with_title("Build Output");
        let errors_section = snapshot_utils::create_error_section(errs.into_vec(), cwd);
        section.add_child(errors_section);
        Some(section)
      }
    }
  }
}
