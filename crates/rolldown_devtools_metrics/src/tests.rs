//! Unit tests: fold synthetic devtools events (the same JSON shapes the writer receives) with
//! fixed emit-time instants, then assert on the report model, the delta, the history line, and
//! snapshot the rendered views. Everything is deterministic — no wall clock reaches a snapshot.

use std::time::{Duration, Instant};

use serde_json::json;

use crate::{MetricsAggregator, MetricsConfig, render, report::MetricValue};

fn agg() -> MetricsAggregator {
  MetricsAggregator::new(MetricsConfig { dir: "ignored".to_string(), top_n: 20, delta: true })
}

fn at(base: Instant, ms: u64) -> Instant {
  base + Duration::from_millis(ms)
}

fn session_meta() -> serde_json::Value {
  json!({
    "action": "SessionMeta",
    "inputs": [{"name": "a"}, {"name": "b"}],
    "plugins": [{"name": "test-plugin"}],
    "cwd": "/app",
    "platform": "node",
    "format": "esm",
  })
}

fn module_graph() -> serde_json::Value {
  json!({
    "action": "ModuleGraphReady",
    "modules": [
      {
        "id": "/app/src/a.ts",
        "imports": [
          {"kind": "import-statement", "module_id": "/app/src/shared.ts"},
          {"kind": "import-statement", "module_id": "/app/node_modules/foo/index.js"},
        ],
      },
      {
        "id": "/app/src/b.ts",
        "imports": [
          {"kind": "import-statement", "module_id": "/app/src/shared.ts"},
          {"kind": "dynamic-import", "module_id": "/app/src/lazy.ts"},
        ],
      },
      {
        "id": "/app/src/shared.ts",
        "importers": ["/app/src/a.ts", "/app/src/b.ts"],
        "imports": [{"kind": "import-statement", "module_id": "ext"}],
      },
      {"id": "/app/node_modules/foo/index.js", "importers": ["/app/src/a.ts"]},
      {"id": "/app/src/lazy.ts", "importers": ["/app/src/b.ts"]},
      {"id": "ext", "is_external": true, "importers": ["/app/src/shared.ts"]},
    ],
  })
}

fn chunk_graph() -> serde_json::Value {
  json!({
    "action": "ChunkGraphReady",
    "chunks": [
      {
        "chunk_id": 0,
        "name": "a",
        "is_user_defined_entry": true,
        "entry_module": "/app/src/a.ts",
        "modules": ["/app/src/a.ts"],
        "reason": "entry",
        "imports": [{"chunk_id": 2, "kind": "import-statement"}],
      },
      {
        "chunk_id": 1,
        "name": "b",
        "is_user_defined_entry": true,
        "entry_module": "/app/src/b.ts",
        "modules": ["/app/src/b.ts"],
        "reason": "entry",
        "imports": [
          {"chunk_id": 2, "kind": "import-statement"},
          {"chunk_id": 3, "kind": "dynamic-import"},
        ],
      },
      {
        "chunk_id": 2,
        "name": "shared",
        "modules": ["/app/src/shared.ts", "/app/node_modules/foo/index.js"],
        "reason": "common",
        "imports": [],
      },
      {
        "chunk_id": 3,
        "name": "lazy",
        "is_async_entry": true,
        "entry_module": "/app/src/lazy.ts",
        "modules": ["/app/src/lazy.ts"],
        "reason": "dynamic-entry",
        "imports": [],
      },
    ],
  })
}

fn package_graph() -> serde_json::Value {
  json!({
    "action": "PackageGraphReady",
    "packages": [
      {"name": "foo", "version": "1.0.0", "is_used": true, "dependency_type": "direct", "size": 1200, "modules": ["/app/node_modules/foo/index.js"]},
      {"name": "foo", "version": "2.0.0", "is_used": true, "dependency_type": "transitive", "size": 300, "modules": []},
      {"name": "bar", "version": "1.0.0", "is_used": false, "dependency_type": "transitive", "size": 50, "modules": []},
    ],
  })
}

fn assets(a_size: u32, b_size: u32) -> serde_json::Value {
  json!({
    "action": "AssetsReady",
    "assets": [
      {"chunk_id": 0, "size": a_size, "filename": "dist/a.js"},
      {"chunk_id": 1, "size": b_size, "filename": "dist/b.js"},
      {"chunk_id": 2, "size": 3000, "filename": "dist/shared.js"},
      {"chunk_id": 3, "size": 500, "filename": "dist/lazy.js"},
      {"size": 400, "filename": "dist/style.css"},
      {"size": 99999, "filename": "dist/a.js.map"},
    ],
  })
}

/// Fold one complete build (the real event order, including the double `BuildStart` the bundle
/// emits) spanning `[start_ms, start_ms + 100]`.
fn fold_build(agg: &mut MetricsAggregator, base: Instant, start_ms: u64, a_size: u32, b_size: u32) {
  let t = |offset: u64| at(base, start_ms + offset);
  agg.fold(&session_meta(), t(0));
  agg.fold(&json!({"action": "BuildStart"}), t(0));
  agg.fold(&json!({"action": "BuildStart"}), t(0));
  // c1: resolveId that resolved (did work).
  agg.fold(
    &json!({"action": "HookResolveIdCallStart", "call_id": "c1", "plugin_name": "test-plugin"}),
    t(5),
  );
  agg.fold(
    &json!({"action": "HookResolveIdCallEnd", "call_id": "c1", "resolved_id": "/app/src/shared.ts"}),
    t(7),
  );
  // c2: transform that changed the code (did work).
  agg.fold(
    &json!({
      "action": "HookTransformCallStart",
      "call_id": "c2",
      "plugin_name": "test-plugin",
      "module_id": "/app/src/a.ts",
      "content": "let x = 1;",
    }),
    t(10),
  );
  agg.fold(
    &json!({
      "action": "HookTransformCallEnd",
      "call_id": "c2",
      "content": "let x = 1; export {};",
    }),
    t(15),
  );
  // c3: resolveId early return (no-op).
  agg.fold(
    &json!({"action": "HookResolveIdCallStart", "call_id": "c3", "plugin_name": "test-plugin"}),
    t(20),
  );
  agg.fold(&json!({"action": "HookResolveIdCallEnd", "call_id": "c3", "resolved_id": null}), t(21));
  // c4: transform that returned the code unchanged (no-op) — End always carries Some(content).
  agg.fold(
    &json!({
      "action": "HookTransformCallStart",
      "call_id": "c4",
      "plugin_name": "test-plugin",
      "module_id": "/app/src/b.ts",
      "content": "export const b = 2;",
    }),
    t(25),
  );
  agg.fold(
    &json!({
      "action": "HookTransformCallEnd",
      "call_id": "c4",
      "content": "export const b = 2;",
    }),
    t(27),
  );
  // c5: load early return (no-op).
  agg.fold(
    &json!({"action": "HookLoadCallStart", "call_id": "c5", "plugin_name": "test-plugin", "module_id": "/app/src/lazy.ts"}),
    t(30),
  );
  agg.fold(&json!({"action": "HookLoadCallEnd", "call_id": "c5", "content": null}), t(31));
  agg.fold(&module_graph(), t(55));
  agg.fold(&json!({"action": "BuildEnd"}), t(60));
  agg.fold(&chunk_graph(), t(70));
  agg.fold(&package_graph(), t(75));
  agg.fold(&assets(a_size, b_size), t(95));
  agg.fold(&json!({"action": "BuildEnd"}), t(100));
}

fn int(v: u64) -> MetricValue {
  MetricValue::Int(v)
}

#[test]
fn report_metrics_and_sections() {
  let base = Instant::now();
  let mut agg = agg();
  fold_build(&mut agg, base, 0, 1000, 800);
  let report = agg.build_report(None, None);

  let m = |id: &str| report.metrics.get(id).copied().unwrap();
  assert_eq!(m("build.total_ms"), MetricValue::Float(100.0));
  assert_eq!(m("build.scan_ms"), MetricValue::Float(60.0));
  assert_eq!(m("build.generate_ms"), MetricValue::Float(40.0));
  // 1000 + 800 + 3000 + 500 + 400; the .map asset is excluded.
  assert_eq!(m("output.total_bytes"), int(5700));
  assert_eq!(m("output.js_bytes"), int(5300));
  assert_eq!(m("output.css_bytes"), int(400));
  assert_eq!(m("output.chunk_count"), int(4));
  assert_eq!(m("output.asset_count"), int(5));
  // Entries: a, b (user-defined) + lazy (async entry).
  assert_eq!(m("output.entry_count"), int(3));
  // a: 1000 + shared 3000 = 4000 (dynamic import excluded from b's 800 + 3000).
  assert_eq!(m("output.max_initial_load_bytes"), int(4000));
  assert_eq!(m("modules.count"), int(6));
  assert_eq!(m("modules.external_count"), int(1));
  // shared.ts + ext (via shared) reachable from entries a and b; lazy.ts from entry b's
  // dynamic import and from its own async entry.
  assert_eq!(m("modules.shared_across_entries_count"), int(3));
  assert_eq!(m("chunks.duplicated_module_count"), int(0));
  assert_eq!(m("packages.count"), int(3));
  assert_eq!(m("packages.duplicate_version_count"), int(1));
  assert_eq!(m("plugins.hook_call_count"), int(5));
  // c3 (resolveId returned nothing) + c4 (transform returned unchanged code) + c5 (load
  // returned nothing); c1/c2 did work.
  assert_eq!(m("plugins.hook_noop_call_count"), int(3));
  assert_eq!(m("plugins.hook_time_ms"), MetricValue::Float(11.0));

  let plugin = &report.plugins[0];
  assert_eq!(plugin.name, "test-plugin");
  assert_eq!(plugin.noop_calls, 3);
  assert_eq!(plugin.hooks["resolveId"].calls, 2);
  assert_eq!(plugin.hooks["resolveId"].noop_calls, 1);
  assert_eq!(plugin.hooks["transform"].calls, 2);
  assert_eq!(plugin.hooks["transform"].noop_calls, 1);
  assert_eq!(plugin.hooks["load"].calls, 1);
  assert_eq!(plugin.hooks["load"].noop_calls, 1);

  assert_eq!(report.session.build_index, 1);
  let entry_a = &report.entries[0];
  assert_eq!(entry_a.entry, "src/a.ts");
  assert_eq!(entry_a.chunk, "dist/a.js");
  assert_eq!(entry_a.chunk_bytes, 1000);
  assert_eq!(entry_a.initial_load_bytes, 4000);
  assert_eq!(entry_a.static_imports, vec!["dist/shared.js"]);
  assert_eq!(report.transform_hotspots.len(), 2);
  assert_eq!(report.transform_hotspots[0].module, "src/a.ts");
  assert_eq!(report.transform_hotspots[0].ms, 5.0);
  assert_eq!(report.transform_hotspots[1].module, "src/b.ts");
  assert!(report.delta.is_none());
}

#[test]
fn report_json_snapshot() {
  let base = Instant::now();
  let mut agg = agg();
  fold_build(&mut agg, base, 0, 1000, 800);
  let report = agg.build_report(None, None);
  insta::assert_snapshot!(serde_json::to_string_pretty(&report).unwrap());
}

#[test]
fn markdown_snapshots() {
  let base = Instant::now();
  let mut agg = agg();
  fold_build(&mut agg, base, 0, 1000, 800);
  let report = agg.build_report(None, None);
  insta::assert_snapshot!("entry_md", render::render_entry(&report));
  insta::assert_snapshot!("timing_md", render::render_timing(&report));
  insta::assert_snapshot!("graph_md", render::render_graph(&report));
  insta::assert_snapshot!("packages_md", render::render_packages(&report));
}

#[test]
fn delta_between_two_builds() {
  let base = Instant::now();
  let mut agg = agg();
  fold_build(&mut agg, base, 0, 1000, 800);
  let state = agg.build_report(None, None).to_state();

  // Second build in the same session: entry chunk `a` grows by 500 bytes.
  fold_build(&mut agg, base, 1000, 1500, 800);
  let report = agg.build_report(Some(&state), None);

  assert_eq!(report.session.build_index, 2);
  let delta = report.delta.as_ref().unwrap();
  let total = delta.metrics.get("output.total_bytes").unwrap();
  assert_eq!(total.prev, int(5700));
  assert_eq!(total.curr, int(6200));
  assert_eq!(total.delta, 500.0);
  assert_eq!(total.pct, Some(8.8));
  // Unchanged metric: pct present and zero (prev is nonzero).
  let modules = delta.metrics.get("modules.count").unwrap();
  assert_eq!(modules.delta, 0.0);
  assert_eq!(modules.pct, Some(0.0));

  let entry_a = delta.entries.iter().find(|e| e.entry == "src/a.ts").unwrap();
  assert_eq!(entry_a.initial_load_bytes.prev, int(4000));
  assert_eq!(entry_a.initial_load_bytes.curr, int(4500));
  assert_eq!(entry_a.initial_load_bytes.pct, Some(12.5));
  assert!(delta.added_entries.is_empty());
  assert!(delta.removed_entries.is_empty());

  insta::assert_snapshot!("delta_md", render::render_delta(&report));
}

#[test]
fn baseline_delta_stays_pinned_across_builds() {
  let base = Instant::now();
  let mut agg = agg();
  fold_build(&mut agg, base, 0, 1000, 800);
  // Pin build 1 as the baseline (the file-system equivalent: copy .state.json -> baseline.json).
  let baseline = agg.build_report(None, None).to_state();

  // Experiment A.
  fold_build(&mut agg, base, 1000, 1500, 800);
  let prev = agg.build_report(None, None).to_state();

  // Experiment B: chain delta compares vs A, baseline delta still compares vs build 1.
  fold_build(&mut agg, base, 2000, 2000, 800);
  let report = agg.build_report(Some(&prev), Some(&baseline));

  let chain = report.delta.as_ref().unwrap().metrics.get("output.total_bytes").unwrap();
  assert_eq!(chain.prev, int(6200));
  assert_eq!(chain.curr, int(6700));
  let pinned = report.baseline_delta.as_ref().unwrap().metrics.get("output.total_bytes").unwrap();
  assert_eq!(pinned.prev, int(5700));
  assert_eq!(pinned.curr, int(6700));
  assert_eq!(pinned.pct, Some(17.5));

  insta::assert_snapshot!("delta_md_with_baseline", render::render_delta(&report));
}

#[test]
fn double_build_start_counts_one_build() {
  let base = Instant::now();
  let mut agg = agg();
  agg.fold(&session_meta(), at(base, 0));
  agg.fold(&json!({"action": "BuildStart"}), at(base, 0));
  agg.fold(&json!({"action": "BuildStart"}), at(base, 0));
  let report = agg.build_report(None, None);
  assert_eq!(report.session.build_index, 1);
  // Incomplete build: no history line.
  assert!(agg.history_line(123).is_none());
}

#[test]
fn history_line_after_completed_build() {
  let base = Instant::now();
  let mut agg = agg();
  fold_build(&mut agg, base, 0, 1000, 800);
  let line = agg.history_line(1234).unwrap();
  assert_eq!(line.build, 1);
  assert_eq!(line.ts_ms, 1234);
  assert_eq!(line.metrics.get("output.total_bytes").copied().unwrap(), int(5700));
  assert_eq!(line.entries.len(), 3);
  // One line = one JSON object, safe to append to history.jsonl.
  let json = serde_json::to_string(&line).unwrap();
  assert!(!json.contains('\n'));
}

#[test]
fn state_survives_roundtrip_and_self_delta_is_zero() {
  let base = Instant::now();
  let mut agg = agg();
  fold_build(&mut agg, base, 0, 1000, 800);
  let state = agg.build_report(None, None).to_state();
  let raw = serde_json::to_string(&state).unwrap();
  let parsed: crate::report::MetricsState = serde_json::from_str(&raw).unwrap();
  let report = agg.build_report(Some(&parsed), None);
  let delta = report.delta.as_ref().unwrap();
  assert!(delta.metrics.values().all(|d| d.delta == 0.0));
}
