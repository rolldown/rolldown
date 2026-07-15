use std::{
  fs,
  path::{Path, PathBuf},
  sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use bench::{
  create_bench_context_with_memory_fs,
  link_baseline::{
    BaselineConfig, BaselineMode, BuildProvenance, REPORT_SCHEMA_VERSION,
    digest::{FramedHasher, digest_diagnostics, digest_output},
    run_baseline, timing_stats,
    trace::{LinkTraceLayer, records_link_trace_metadata},
    validate_build_provenance_values,
    workloads::{
      DEFAULT_SEED, SYNTHETIC_WORKLOAD_IDS, diagnostic_order_workload, read_repository_head,
      synthetic_workload_with_seed,
    },
  },
};
use rolldown::BundleOutput;
use rolldown_common::{Modules, Output, OutputChunk};
use tracing_subscriber::{Layer as _, filter::filter_fn, prelude::*};

static TEMP_DIRECTORY_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
  fn new(label: &str) -> Self {
    let sequence = TEMP_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
      .join(format!("rolldown-link-baseline-{label}-{}-{sequence}", std::process::id()));
    fs::create_dir_all(&path).expect("create temporary repository");
    Self(path)
  }

  fn join(&self, path: impl AsRef<Path>) -> PathBuf {
    self.0.join(path)
  }

  fn write(&self, path: impl AsRef<Path>, contents: &str) {
    let path = self.join(path);
    fs::create_dir_all(path.parent().expect("temporary path parent"))
      .expect("create temporary parent");
    fs::write(path, contents).expect("write temporary Git metadata");
  }
}

impl Drop for TempDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

const COMMIT_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const COMMIT_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn canonical_build_provenance(commit: &str) -> BuildProvenance {
  BuildProvenance {
    verified: true,
    version: "1".to_string(),
    git_commit: commit.to_string(),
    git_tree: COMMIT_B.to_string(),
    git_dirty: Some(false),
    rustc: "rustc 1.97.0 (2d8144b78 2026-07-07)".to_string(),
    rustc_commit_hash: "2d8144b7880597b6e6d3dfd63a9a9efae3f533d3".to_string(),
    rustc_host: "x86_64-unknown-linux-gnu".to_string(),
    rustc_llvm: "22.1.6".to_string(),
    cargo: "cargo 1.97.0 (c980f4866 2026-06-30)".to_string(),
    profile: "release".to_string(),
    opt_level: "3".to_string(),
    debug: "false".to_string(),
    debug_assertions: false,
    lto: "fat".to_string(),
    codegen_units: "1".to_string(),
    strip: "symbols".to_string(),
    target: "x86_64-unknown-linux-gnu".to_string(),
    host: "x86_64-unknown-linux-gnu".to_string(),
    rustflags_hex: String::new(),
    command: "cargo build --locked --release -p bench --features link-baseline --bin link-baseline"
      .to_string(),
  }
}

#[test]
fn canonical_build_provenance_rejects_old_or_differently_compiled_binaries() {
  let rustc = "rustc 1.97.0 (2d8144b78 2026-07-07)";
  let cargo = "cargo 1.97.0 (c980f4866 2026-06-30)";
  let valid = canonical_build_provenance(COMMIT_A);
  validate_build_provenance_values(&valid, COMMIT_A, rustc, cargo)
    .expect("matching canonical provenance");

  let mut candidate = valid.clone();
  candidate.git_commit = COMMIT_B.to_string();
  assert!(validate_build_provenance_values(&candidate, COMMIT_A, rustc, cargo).is_err());

  let mut candidate = valid.clone();
  candidate.lto = "thin".to_string();
  assert!(validate_build_provenance_values(&candidate, COMMIT_A, rustc, cargo).is_err());

  let mut candidate = valid.clone();
  candidate.rustflags_hex = "2d43636f646567656e2d756e6974733d3136".to_string();
  assert!(validate_build_provenance_values(&candidate, COMMIT_A, rustc, cargo).is_err());

  let mut candidate = valid;
  candidate.verified = false;
  assert!(validate_build_provenance_values(&candidate, COMMIT_A, rustc, cargo).is_err());
}

#[test]
fn repository_head_supports_detached_loose_and_packed_references() {
  let temp = TempDirectory::new("ordinary-git-dir");
  let repository = temp.join("repository");
  temp.write("repository/.git/HEAD", &format!("{COMMIT_A}\n"));
  assert_eq!(read_repository_head(&repository).as_deref(), Ok(COMMIT_A));

  temp.write("repository/.git/HEAD", "ref: refs/heads/main\n");
  temp.write("repository/.git/refs/heads/main", &format!("{COMMIT_B}\n"));
  assert_eq!(read_repository_head(&repository).as_deref(), Ok(COMMIT_B));

  fs::remove_file(temp.join("repository/.git/refs/heads/main")).expect("remove loose ref");
  temp.write(
    "repository/.git/packed-refs",
    &format!("# pack-refs with: peeled fully-peeled\n{COMMIT_A} refs/heads/main\n"),
  );
  assert_eq!(read_repository_head(&repository).as_deref(), Ok(COMMIT_A));
}

#[test]
fn repository_head_resolves_linked_worktree_common_and_private_refs() {
  let temp = TempDirectory::new("linked-worktree");
  let repository = temp.join("repository");
  let git_dir = temp.join("common/worktrees/link");
  temp.write("repository/.git", "gitdir: ../common/worktrees/link\n");
  temp.write("common/worktrees/link/HEAD", "ref: refs/heads/topic\n");
  temp.write("common/worktrees/link/commondir", "../..\n");
  temp.write("common/refs/heads/topic", &format!("{COMMIT_A}\n"));
  assert_eq!(read_repository_head(&repository).as_deref(), Ok(COMMIT_A));

  fs::remove_file(temp.join("common/refs/heads/topic")).expect("remove common loose ref");
  temp.write("common/packed-refs", &format!("{COMMIT_B} refs/heads/topic\n"));
  temp.write("common/worktrees/link/commondir", &format!("{}\n", temp.join("common").display()));
  temp.write("repository/.git", &format!("gitdir: {}\n", git_dir.display()));
  assert_eq!(read_repository_head(&repository).as_deref(), Ok(COMMIT_B));

  temp.write("common/worktrees/link/HEAD", "ref: refs/worktree/local\n");
  temp.write("common/worktrees/link/refs/worktree/local", &format!("{COMMIT_A}\n"));
  assert_eq!(read_repository_head(&repository).as_deref(), Ok(COMMIT_A));
}

#[test]
fn repository_head_fails_closed_for_corrupt_or_unsafe_refs() {
  let temp = TempDirectory::new("invalid-git-metadata");
  let repository = temp.join("repository");
  temp.write("repository/.git/HEAD", "ref: refs/heads/main\n");
  temp.write("repository/.git/refs/heads/main", "not-a-commit\n");
  temp.write("repository/.git/packed-refs", &format!("{COMMIT_A} refs/heads/main\n"));
  let error = read_repository_head(&repository).expect_err("corrupt loose ref must not fall back");
  assert!(error.contains("invalid commit ID"), "{error}");

  fs::remove_file(temp.join("repository/.git/refs/heads/main")).expect("remove corrupt loose ref");
  fs::create_dir(temp.join("repository/.git/refs/heads/main")).expect("replace ref with directory");
  let error =
    read_repository_head(&repository).expect_err("loose ref I/O errors must not fall back");
  assert!(error.contains("failed to read"), "{error}");
  fs::remove_dir(temp.join("repository/.git/refs/heads/main"))
    .expect("remove invalid ref directory");

  temp.write("repository/.git/HEAD", "ref: refs/heads/../../escaped\n");
  temp.write("repository/.git/escaped", &format!("{COMMIT_B}\n"));
  let error = read_repository_head(&repository).expect_err("unsafe ref path must be rejected");
  assert!(error.contains("unsafe Git reference"), "{error}");

  temp.write("repository/.git/HEAD", "ref: refs/heads/missing\n");
  temp.write("repository/.git/packed-refs", "malformed packed ref\n");
  let error = read_repository_head(&repository).expect_err("malformed packed refs must fail");
  assert!(error.contains("invalid entry"), "{error}");
}

#[test]
fn synthetic_manifests_are_stable_and_have_exact_sizes() {
  let expected = [
    ("overhead-64", 64, 8_376, "78c70e0cae50f65611eb18e2778e2791"),
    ("wide-4096", 4_096, 542_191, "a4352d87b0afdb449f386c9f4f94bae1"),
    ("deep-1024", 1_024, 136_484, "1b408cfbea976792a83bc4419863ff42"),
    ("scc-256x4", 1_025, 116_358, "146e457a611d041edcb86a7b81d2de2b"),
    ("export-star-1024", 2_048, 117_171, "2559380bc3588fa5ae719a2a31a6734d"),
    ("cjs-2048", 2_049, 211_769, "d59a17f985d639bdf05c5d911cd5cd0f"),
    ("json-2048", 2_049, 222_009, "127f8a9a04531df66c6ac20518c54051"),
    ("dynamic-1024", 1_025, 79_326, "cf3c6f6754d17f9bb59018887db85c50"),
  ];
  assert_eq!(SYNTHETIC_WORKLOAD_IDS, expected.map(|(id, _, _, _)| id));

  for (id, module_count, source_bytes, input_digest) in expected {
    let first = synthetic_workload_with_seed(id, DEFAULT_SEED).expect("known workload");
    let second = synthetic_workload_with_seed(id, DEFAULT_SEED).expect("known workload");
    let different_seed =
      synthetic_workload_with_seed(id, DEFAULT_SEED + 1).expect("known workload");
    assert_eq!(first.manifest.source_module_count, Some(module_count), "{id}");
    assert_eq!(first.manifest.file_count, module_count, "{id}");
    assert_eq!(first.manifest.source_bytes, source_bytes, "{id}");
    assert_eq!(first.manifest.input_digest, input_digest, "{id}");
    assert_eq!(first.manifest.input_digest, second.manifest.input_digest, "{id}");
    assert_ne!(first.manifest.input_digest, different_seed.manifest.input_digest, "{id}");
  }
}

#[test]
fn digest_framing_distinguishes_field_boundaries_and_order() {
  let mut left = FramedHasher::new("test");
  left.str("ab");
  left.str("c");

  let mut different_boundary = FramedHasher::new("test");
  different_boundary.str("a");
  different_boundary.str("bc");

  let mut different_order = FramedHasher::new("test");
  different_order.str("c");
  different_order.str("ab");

  assert_ne!(left.finish(), different_boundary.finish());
  assert_ne!(left.finish(), different_order.finish());
}

#[test]
fn output_digest_preserves_semantic_backslashes_and_cwd_text() {
  let cwd = Path::new("/worktree");
  assert_ne!(
    digest_output(&output_with_code("globalThis.value = '\\\\worktree';"), cwd),
    digest_output(&output_with_code("globalThis.value = '/worktree';"), cwd)
  );
  assert_ne!(
    digest_output(&output_with_code("globalThis.value = '/worktree';"), cwd),
    digest_output(&output_with_code("globalThis.value = '<cwd>';"), cwd)
  );
}

#[test]
fn output_digest_separates_cwd_relative_and_literal_path_categories() {
  let cwd = Path::new("/worktree");
  assert_ne!(
    digest_output(&output_with_filename("/worktree/a.js"), cwd),
    digest_output(&output_with_filename("<cwd>/a.js"), cwd)
  );
}

fn output_with_code(code: &str) -> BundleOutput {
  output_with_code_and_filename(code, "entry.js")
}

fn output_with_filename(filename: &str) -> BundleOutput {
  output_with_code_and_filename("", filename)
}

fn output_with_code_and_filename(code: &str, filename: &str) -> BundleOutput {
  BundleOutput {
    warnings: Vec::new(),
    assets: vec![Output::Chunk(Arc::new(OutputChunk {
      name: "entry".into(),
      is_entry: true,
      is_dynamic_entry: false,
      facade_module_id: None,
      module_ids: Vec::new(),
      exports: Vec::new(),
      filename: filename.into(),
      modules: Modules { keys: Vec::new(), values: Vec::new() },
      imports: Vec::new(),
      dynamic_imports: Vec::new(),
      code: code.to_string(),
      map: None,
      sourcemap_filename: None,
      preliminary_filename: filename.to_string(),
    }))],
  }
}

#[test]
fn timing_statistics_use_median_absolute_deviation() {
  let stats = timing_stats(&[100, 101, 102, 103, 10_000]).expect("non-empty samples");
  assert_eq!(stats.median_ns, 102);
  assert_eq!(stats.mad_ns, 1);
  assert!((stats.relative_mad - 1.0 / 102.0).abs() < f64::EPSILON);
}

#[test]
fn trace_collector_records_direct_children_and_pass_labels() {
  let layer = LinkTraceLayer::default();
  let subscriber = tracing_subscriber::registry()
    .with(layer.clone().with_filter(filter_fn(records_link_trace_metadata)));
  tracing::subscriber::with_default(subscriber, || {
    let link = tracing::debug_span!(target: "rolldown::stages::link_stage", "link");
    let _link = link.enter();
    {
      let step = tracing::debug_span!("sort_modules");
      let _step = step.enter();
    }
    {
      let pass = tracing::debug_span!(target: "rolldown::pass", "run_pass", pass = "test::Pass");
      let _pass = pass.enter();
    }
  });

  let sample = layer.sample(Duration::from_secs(1)).expect("link span");
  assert_eq!(
    sample.direct_children.iter().map(|span| span.name.as_str()).collect::<Vec<_>>(),
    ["sort_modules", "run_pass"]
  );
  assert_eq!(sample.direct_children[1].target, "rolldown::pass");
  assert_eq!(sample.direct_children[1].pass.as_deref(), Some("test::Pass"));
  assert!(sample.detached_passes.is_empty());
  assert_eq!(
    sample.inside_link_unattributed_ns + sample.direct_children_wall_coverage_ns,
    sample.link_span_ns
  );
}

#[test]
fn trace_collector_keeps_closed_spans_when_registry_ids_are_reused() {
  let layer = LinkTraceLayer::default();
  let subscriber = tracing_subscriber::registry()
    .with(layer.clone().with_filter(filter_fn(records_link_trace_metadata)));
  tracing::subscriber::with_default(subscriber, || {
    let link = tracing::debug_span!(target: "rolldown::stages::link_stage", "link");
    let _link = link.enter();
    for _ in 0..64 {
      let child = tracing::debug_span!("reused_child_callsite");
      let _child = child.enter();
    }
  });

  let sample = layer.sample(Duration::from_secs(1)).expect("link span");
  assert_eq!(sample.direct_children.len(), 64);
  assert!(sample.direct_children.iter().all(|span| span.call_count == 1));
  layer.reset().expect("completed sample can reset");
  assert!(layer.sample(Duration::from_secs(1)).is_err(), "reset must discard the previous sample");
}

#[test]
fn trace_collector_rejects_multiple_entered_link_spans() {
  let layer = LinkTraceLayer::default();
  let subscriber = tracing_subscriber::registry()
    .with(layer.clone().with_filter(filter_fn(records_link_trace_metadata)));
  tracing::subscriber::with_default(subscriber, || {
    for _ in 0..2 {
      let link = tracing::debug_span!(target: "rolldown::stages::link_stage", "link");
      let _link = link.enter();
    }
  });

  let error = layer.sample(Duration::from_secs(1)).expect_err("multiple link spans must fail");
  assert!(error.contains("exactly one"), "{error}");
}

#[test]
fn trace_collector_uses_interval_union_and_detects_detached_passes() {
  let layer = LinkTraceLayer::default();
  let subscriber = tracing_subscriber::registry()
    .with(layer.clone().with_filter(filter_fn(records_link_trace_metadata)));
  let dispatch = tracing::Dispatch::new(subscriber);
  tracing::dispatcher::with_default(&dispatch, || {
    let link = tracing::debug_span!(target: "rolldown::stages::link_stage", "link");
    let _link = link.enter();
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|scope| {
      for pass_name in ["test::Left", "test::Right"] {
        let dispatch = dispatch.clone();
        let link = link.clone();
        let barrier = Arc::clone(&barrier);
        scope.spawn(move || {
          tracing::dispatcher::with_default(&dispatch, || {
            let pass = tracing::debug_span!(
              target: "rolldown::pass",
              parent: &link,
              "run_pass",
              pass = pass_name
            );
            let _pass = pass.enter();
            barrier.wait();
            std::thread::sleep(Duration::from_millis(20));
          });
        });
      }
    });
    let dispatch = dispatch.clone();
    std::thread::scope(|scope| {
      scope.spawn(move || {
        tracing::dispatcher::with_default(&dispatch, || {
          let pass = tracing::debug_span!(
            target: "rolldown::pass",
            "run_pass",
            pass = "test::Detached"
          );
          let _pass = pass.enter();
          std::thread::sleep(Duration::from_millis(1));
        });
      });
    });
  });

  let sample = layer.sample(Duration::from_secs(1)).expect("link span");
  assert_eq!(sample.direct_children.len(), 2);
  assert_eq!(sample.detached_passes.len(), 1);
  assert_eq!(sample.detached_passes[0].pass.as_deref(), Some("test::Detached"));
  assert!(sample.direct_children_overlap_excess_ns > 0);
  assert_eq!(
    sample.direct_children_wall_coverage_ns + sample.direct_children_overlap_excess_ns,
    sample.direct_children_inclusive_sum_ns
  );
  assert_eq!(
    sample.inside_link_unattributed_ns + sample.direct_children_wall_coverage_ns,
    sample.link_span_ns
  );
}

#[tokio::test(flavor = "current_thread")]
async fn testing_hook_scans_outside_and_returns_one_link_sample() {
  let workload = synthetic_workload_with_seed("overhead-64", DEFAULT_SEED).expect("workload");
  let report = run_baseline(
    BaselineConfig {
      mode: BaselineMode::LinkTime,
      warmups: 0,
      samples: 1,
      iterations_per_sample: 3,
      canonical: false,
    },
    workload,
  )
  .await
  .expect("link baseline");
  assert_eq!(report.sample_ns.len(), 1);
  assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
  assert_eq!(report.environment.build_profile, report.build.profile);
  assert_eq!(report.iterations_per_sample, 3);
  assert_eq!(report.linked_module_count, Some(65));
  assert!(!report.environment.canonical);
  assert!(report.stats.is_some());
  assert!(report.digests.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn trace_mode_never_installs_a_global_subscriber_from_the_library_api() {
  let workload = synthetic_workload_with_seed("overhead-64", DEFAULT_SEED).expect("workload");
  let error = run_baseline(
    BaselineConfig {
      mode: BaselineMode::LinkTrace,
      warmups: 0,
      samples: 1,
      iterations_per_sample: 1,
      canonical: false,
    },
    workload,
  )
  .await
  .expect_err("trace mode requires the dedicated process collector");
  assert!(error.contains("process-global trace collector"));
}

#[tokio::test(flavor = "current_thread")]
async fn generate_observer_preserves_standard_output() {
  let observed_workload =
    synthetic_workload_with_seed("overhead-64", DEFAULT_SEED).expect("observed workload");
  let standard_workload =
    synthetic_workload_with_seed("overhead-64", DEFAULT_SEED).expect("standard workload");
  let cwd = standard_workload.options.cwd.clone().expect("workload cwd");
  let mut context =
    create_bench_context_with_memory_fs(&standard_workload.options, standard_workload.fs.clone());
  let bundle =
    context.factory.create_bundle_with_fs(context.mem_fs.clone(), context.create_resolver());
  let standard_output = bundle.generate().await.expect("standard Generate output");

  let observed = run_baseline(
    BaselineConfig {
      mode: BaselineMode::Digest,
      warmups: 0,
      samples: 1,
      iterations_per_sample: 1,
      canonical: false,
    },
    observed_workload,
  )
  .await
  .expect("observed Generate output");
  let digests = observed.digests.expect("digest set");
  assert_eq!(digests.output_digest, digest_output(&standard_output, &cwd));
  assert_eq!(digests.final_diagnostic_digest, digest_diagnostics(&standard_output.warnings, &cwd));
}

#[tokio::test(flavor = "current_thread")]
async fn pre_generate_diagnostics_preserve_cross_pass_and_local_order() {
  let report = run_baseline(
    BaselineConfig {
      mode: BaselineMode::Digest,
      warmups: 0,
      samples: 1,
      iterations_per_sample: 1,
      canonical: false,
    },
    diagnostic_order_workload(),
  )
  .await
  .expect("diagnostic baseline");
  assert_eq!(report.manifest.file_count, 12);
  assert_eq!(report.manifest.source_bytes, 909);
  assert_eq!(report.manifest.input_digest, "9586764cf0f88b8c207e13aaf4b452d9");

  let descriptors = report
    .pre_generate_diagnostics
    .iter()
    .map(|diagnostic| {
      (
        diagnostic.severity.as_str(),
        diagnostic.kind.as_str(),
        diagnostic.rendered.lines().next().unwrap_or_default(),
        diagnostic.file.as_ref().map(|file| file.value.as_str()),
        diagnostic.line,
        diagnostic.column,
        diagnostic.utf16_offset,
      )
    })
    .collect::<Vec<_>>();
  assert_eq!(
    descriptors,
    [
      (
        "warning",
        "CIRCULAR_DEPENDENCY",
        "[CIRCULAR_DEPENDENCY] Circular dependency: cycle_a0.js -> cycle_a1.js -> cycle_a0.js.",
        None,
        None,
        None,
        None,
      ),
      (
        "warning",
        "CIRCULAR_DEPENDENCY",
        "[CIRCULAR_DEPENDENCY] Circular dependency: cycle_b0.js -> cycle_b1.js -> cycle_b0.js.",
        None,
        None,
        None,
        None,
      ),
      (
        "error",
        "REQUIRE_TLA",
        "[REQUIRE_TLA] This require call is not allowed because the transitive dependency \"tla_a1.js\" contains a top-level await",
        Some("entry.js"),
        Some(5),
        Some(19),
        Some(209),
      ),
      (
        "error",
        "REQUIRE_TLA",
        "[REQUIRE_TLA] This require call is not allowed because the transitive dependency \"tla_b1.js\" contains a top-level await",
        Some("entry.js"),
        Some(6),
        Some(19),
        Some(252),
      ),
      (
        "error",
        "MISSING_EXPORT",
        "[MISSING_EXPORT] \"missing_a\" is not exported by \"missing_a.js\".",
        Some("entry.js"),
        Some(3),
        Some(9),
        Some(111),
      ),
      (
        "error",
        "MISSING_EXPORT",
        "[MISSING_EXPORT] \"missing_b\" is not exported by \"missing_b.js\".",
        Some("entry.js"),
        Some(4),
        Some(9),
        Some(155),
      ),
    ]
  );
  assert_eq!(report.final_diagnostics.len(), 4, "Generate returns only error diagnostics");
  assert_eq!(report.linked_module_count, Some(13));
  assert!(report.sample_ns.is_empty(), "digest mode must not report a timing sample");
  assert!(report.stats.is_none());
  assert_eq!(
    report.digests.as_ref().map(|digests| digests.capture_model),
    Some("single-standard-generate-run-with-pre-generate-observer")
  );
  assert_eq!(report.digests.as_ref().map(|digests| digests.outcome), Some("error"));

  let expected_order = report
    .pre_generate_diagnostics
    .iter()
    .map(|diagnostic| {
      (diagnostic.kind.clone(), diagnostic.rendered.lines().next().unwrap_or_default().to_string())
    })
    .collect::<Vec<_>>();
  for _ in 0..9 {
    let repeat = run_baseline(
      BaselineConfig {
        mode: BaselineMode::Digest,
        warmups: 0,
        samples: 1,
        iterations_per_sample: 1,
        canonical: false,
      },
      diagnostic_order_workload(),
    )
    .await
    .expect("repeat diagnostic baseline");
    let actual_order = repeat
      .pre_generate_diagnostics
      .iter()
      .map(|diagnostic| {
        (
          diagnostic.kind.clone(),
          diagnostic.rendered.lines().next().unwrap_or_default().to_string(),
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(actual_order, expected_order);
  }
}
