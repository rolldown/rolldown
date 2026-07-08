//! Renders the report directory: `metrics.json` (canonical), the progressive markdown views,
//! the delta state, and the per-build history log. Every markdown file is rendered from the
//! same [`Report`] model that `metrics.json` serializes, so the two views cannot drift.

use std::{
  fmt::Write as _,
  io::Write as _,
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

use crate::{
  MetricsAggregator,
  report::{DeltaSection, MetricValue, MetricsState, Report},
};

impl MetricsAggregator {
  fn out_dir(&self) -> PathBuf {
    let dir = &self.config.dir;
    if Path::new(dir).is_absolute() {
      PathBuf::from(dir)
    } else {
      let cwd = self.cwd.clone().unwrap_or_else(|| ".".to_string());
      Path::new(&cwd).join(dir)
    }
  }

  /// Append the current build's summary line to `history.jsonl`. Called when a completed build
  /// is about to be discarded (rebuild in the same session) and at session close, so history
  /// records every build. Failures are swallowed — metrics must never fail the build.
  pub(crate) fn append_history(&self) {
    let Some(line) = self.history_line(unix_ms_now()) else {
      return;
    };
    let Ok(json) = serde_json::to_string(&line) else {
      return;
    };
    let dir = self.out_dir();
    if std::fs::create_dir_all(&dir).is_err() {
      return;
    }
    if let Ok(mut file) =
      std::fs::OpenOptions::new().create(true).append(true).open(dir.join("history.jsonl"))
    {
      let _ = writeln!(file, "{json}");
    }
  }

  /// Render the report directory + persist delta state. Errors are returned for the caller to
  /// log; a metrics-writing failure must never fail the build.
  pub fn render(&self) -> std::io::Result<()> {
    let dir = self.out_dir();
    std::fs::create_dir_all(&dir)?;

    let read_state = |name: &str| {
      std::fs::read_to_string(dir.join(name))
        .ok()
        .and_then(|raw| serde_json::from_str::<MetricsState>(&raw).ok())
    };
    let prev = if self.config.delta { read_state(".state.json") } else { None };
    // A user-pinned reference (copy `.state.json` -> `baseline.json`): honored whenever the
    // file exists, so N experiments can all compare against one fixed build.
    let baseline = read_state("baseline.json");

    let mut report = self.build_report(prev.as_ref(), baseline.as_ref());
    report.generated_at_ms = Some(unix_ms_now());

    let json = serde_json::to_string_pretty(&report).map_err(std::io::Error::other)?;
    std::fs::write(dir.join("metrics.json"), json)?;
    std::fs::write(dir.join("entry.md"), render_entry(&report))?;
    std::fs::write(dir.join("timing.md"), render_timing(&report))?;
    std::fs::write(dir.join("chunks.md"), render_chunks(&report))?;
    std::fs::write(dir.join("modules.md"), render_modules(&report))?;
    std::fs::write(dir.join("packages.md"), render_packages(&report))?;
    std::fs::write(dir.join("graph.md"), render_graph(&report))?;
    if report.delta.is_some() || report.baseline_delta.is_some() {
      std::fs::write(dir.join("delta.md"), render_delta(&report))?;
    }
    std::fs::write(dir.join("AGENTS.md"), AGENTS_MD)?;

    if self.config.delta
      && let Ok(state) = serde_json::to_string_pretty(&report.to_state())
    {
      std::fs::write(dir.join(".state.json"), state)?;
    }
    self.append_history();
    Ok(())
  }
}

fn unix_ms_now() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn metric_f64(report: &Report, id: &str) -> f64 {
  report.metrics.get(id).map_or(0.0, |v| v.as_f64())
}

fn metric_u64(report: &Report, id: &str) -> u64 {
  metric_f64(report, id) as u64
}

pub(crate) fn render_entry(report: &Report) -> String {
  let mut out = String::new();
  out.push_str("# Build Metrics (devtools metrics mode)\n\n");
  writeln!(
    out,
    "Build {} · output {} · {} ({} external) / {} / {} / {} · {} / {} · build #{}\n",
    format_ms(metric_f64(report, "build.total_ms")),
    format_size(metric_u64(report, "output.total_bytes")),
    plural(report.modules.count, "module"),
    report.modules.external_count,
    plural(report.chunks.count, "chunk"),
    plural(metric_u64(report, "output.asset_count") as usize, "asset"),
    plural(report.packages.count, "package"),
    report.session.platform.as_deref().unwrap_or("?"),
    report.session.format.as_deref().unwrap_or("?"),
    report.session.build_index,
  )
  .unwrap();

  let summary_pct = |delta: &DeltaSection, id: &str| {
    delta.metrics.get(id).map_or_else(|| "n/a".to_string(), |d| format_pct(d.pct))
  };
  if let Some(delta) = &report.delta {
    writeln!(
      out,
      "Δ vs previous: build {}, size {}\n",
      summary_pct(delta, "build.total_ms"),
      summary_pct(delta, "output.total_bytes"),
    )
    .unwrap();
  }
  if let Some(baseline) = &report.baseline_delta {
    writeln!(
      out,
      "Δ vs pinned baseline: build {}, size {}\n",
      summary_pct(baseline, "build.total_ms"),
      summary_pct(baseline, "output.total_bytes"),
    )
    .unwrap();
  }

  out.push_str("## Details (load on demand)\n\n");
  out.push_str("| File | Load this when… |\n| --- | --- |\n");
  out.push_str(
    "| `metrics.json` | you need one precise number or the `delta`/`baselineDelta` sections — query it (jq/grep a metric id), don't read it wholesale |\n",
  );
  out.push_str("| `timing.md` | investigating slow builds: stage split, plugin & hook cost |\n");
  out.push_str(
    "| `chunks.md` | bundle size: chunk composition, reasons & cross-chunk duplication |\n",
  );
  out.push_str(
    "| `modules.md` | module graph: import kinds, most-imported, shared-across-entries |\n",
  );
  out.push_str("| `packages.md` | dependency bloat: largest packages, direct/transitive, duplicate versions |\n");
  out.push_str(
    "| `graph.md` | code-splitting: entry points, chunk import graph & initial-load cost |\n",
  );
  if report.delta.is_some() || report.baseline_delta.is_some() {
    out.push_str(
      "| `delta.md` | checking this build for regressions vs the last (and the pinned baseline, if any) |\n",
    );
  }
  out.push_str("| `history.jsonl` | trends: one summary line per build |\n");
  out
    .push_str("| `AGENTS.md` | the directory contract: files, metric ids, experiment workflow |\n");
  out.push('\n');
  out.push_str(
    "> Derived in-memory from the rolldown devtools event stream (same data that feeds Vite \
     DevTools). Durations are measured at event emission. Sizes are per-asset/chunk and \
     per-package; per-module byte size is not yet in the devtools stream.\n",
  );
  out
}

pub(crate) fn render_timing(report: &Report) -> String {
  let mut out = String::new();
  out.push_str("# Build Timing\n\n");
  writeln!(
    out,
    "- **Total build**: {} — scan {} + generate {}\n",
    format_ms(metric_f64(report, "build.total_ms")),
    format_ms(metric_f64(report, "build.scan_ms")),
    format_ms(metric_f64(report, "build.generate_ms")),
  )
  .unwrap();

  out.push_str("## Hook cost by plugin & type\n\n");
  out.push_str(
    "Durations are measured at event emission and summed across concurrent calls, so they can \
     exceed wall-clock on parallel builds. No-op = the hook ran but did nothing (resolveId/\
     load/renderChunk returned nothing; transform returned the code unchanged) — a high no-op \
     share on a hot hook means the plugin should declare a hook filter.\n\n",
  );
  let mut rows: Vec<(&str, &str, &crate::report::HookRow)> = report
    .plugins
    .iter()
    .flat_map(|p| p.hooks.iter().map(|(hook, row)| (p.name.as_str(), hook.as_str(), row)))
    .collect();
  rows.sort_by(|a, b| {
    b.2
      .ms
      .partial_cmp(&a.2.ms)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| a.0.cmp(b.0))
      .then_with(|| a.1.cmp(b.1))
  });
  if rows.is_empty() {
    out.push_str("_No plugin hook calls recorded._\n\n");
  } else {
    out.push_str("| Plugin | Hook | Calls | No-op | Time |\n| --- | --- | --- | --- | --- |\n");
    for (plugin, hook, row) in rows {
      writeln!(
        out,
        "| `{plugin}` | {hook} | {} | {} | {} |",
        row.calls,
        format_noop(row.noop_calls, row.calls),
        format_ms(row.ms),
      )
      .unwrap();
    }
    out.push('\n');
  }

  out.push_str("## Slowest plugins (all hooks)\n\n");
  if report.plugins.is_empty() {
    out.push_str("_None recorded._\n\n");
  } else {
    out.push_str("| Plugin | Calls | Time |\n| --- | --- | --- |\n");
    for plugin in &report.plugins {
      writeln!(out, "| `{}` | {} | {} |", plugin.name, plugin.calls, format_ms(plugin.ms)).unwrap();
    }
    out.push('\n');
  }

  out.push_str("## Transform hotspots (modules)\n\n");
  if report.transform_hotspots.is_empty() {
    out.push_str(
      "_No plugin `transform` hooks ran. (Core TS/JSX/TSX transformation happens inside \
       rolldown, not via a plugin hook, so it isn't attributed per-module here.)_\n",
    );
  } else {
    out.push_str("| Time | Calls | Module |\n| --- | --- | --- |\n");
    for hotspot in &report.transform_hotspots {
      writeln!(out, "| {} | {} | `{}` |", format_ms(hotspot.ms), hotspot.calls, hotspot.module)
        .unwrap();
    }
  }
  out
}

pub(crate) fn render_chunks(report: &Report) -> String {
  let mut out = String::new();
  out.push_str("# Chunks\n\n");
  writeln!(
    out,
    "Output {} ({} JS, {} CSS, {} other) across {} / {}.\n",
    format_size(metric_u64(report, "output.total_bytes")),
    format_size(metric_u64(report, "output.js_bytes")),
    format_size(metric_u64(report, "output.css_bytes")),
    format_size(metric_u64(report, "output.other_bytes")),
    plural(report.chunks.count, "chunk"),
    plural(metric_u64(report, "output.asset_count") as usize, "asset"),
  )
  .unwrap();

  out.push_str("## Chunk reasons\n\n| Reason | Count |\n| --- | --- |\n");
  let mut reasons: Vec<(&String, &usize)> = report.chunks.reasons.iter().collect();
  reasons.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
  for (reason, count) in reasons {
    writeln!(out, "| {reason} | {count} |").unwrap();
  }
  out.push('\n');

  out.push_str("## Largest chunks\n\n");
  out.push_str("| Bytes | Entry? | Reason | Modules | Chunk |\n| --- | --- | --- | --- | --- |\n");
  for chunk in &report.chunks.largest {
    writeln!(
      out,
      "| {} | {} | {} | {} | `{}` |",
      format_size(chunk.bytes),
      if chunk.is_entry { "yes" } else { "no" },
      chunk.reason,
      chunk.module_count,
      chunk.file,
    )
    .unwrap();
  }

  out.push_str("\n## Largest assets\n\n");
  out
    .push_str("Includes chunk JS and standalone assets (CSS, images, …); sourcemaps excluded.\n\n");
  out.push_str("| Bytes | Asset |\n| --- | --- |\n");
  for asset in &report.assets {
    writeln!(out, "| {} | `{}` |", format_size(asset.bytes), asset.file).unwrap();
  }

  out.push_str("\n## Duplicated modules (in >1 chunk)\n\n");
  out.push_str(
    "Modules bundled into multiple chunks ship multiple times — candidates to hoist into a \
     shared chunk.\n\n",
  );
  if report.chunks.duplicated_modules.is_empty() {
    out.push_str("_None — no module is bundled into more than one chunk._\n");
  } else {
    writeln!(
      out,
      "{} duplicated; most-duplicated:\n",
      plural(report.chunks.duplicated_module_count, "module")
    )
    .unwrap();
    out.push_str("| Chunks | Module | In |\n| --- | --- | --- |\n");
    for dup in &report.chunks.duplicated_modules {
      let inn = dup.chunks.iter().map(|l| format!("`{l}`")).collect::<Vec<_>>().join(", ");
      writeln!(out, "| {} | `{}` | {inn} |", dup.chunks.len(), dup.module).unwrap();
    }
  }
  out
}

pub(crate) fn render_modules(report: &Report) -> String {
  let mut out = String::new();
  out.push_str("# Modules\n\n");
  writeln!(
    out,
    "{} ({} external).\n",
    plural(report.modules.count, "module"),
    report.modules.external_count
  )
  .unwrap();

  out.push_str("## Import kinds\n\n| Kind | Count |\n| --- | --- |\n");
  let mut kinds: Vec<(&String, &usize)> = report.modules.import_kinds.iter().collect();
  kinds.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
  for (kind, count) in kinds {
    writeln!(out, "| {kind} | {count} |").unwrap();
  }
  out.push('\n');

  out.push_str("## Most-imported modules\n\n");
  out.push_str("Modules imported by 2+ modules — shared-chunk candidates.\n\n");
  if report.modules.most_imported.is_empty() {
    out.push_str("_No module is imported by more than one other module._\n");
  } else {
    out.push_str("| Importers | Module |\n| --- | --- |\n");
    for row in &report.modules.most_imported {
      writeln!(out, "| {} | `{}` |", row.importers, row.module).unwrap();
    }
  }

  out.push_str("\n## Shared across entry points\n\n");
  out.push_str(
    "Modules reachable from 2+ entry points — the real shared-chunk signal (vs. raw import \
     fan-in above, which counts any importer).\n\n",
  );
  if report.modules.entry_point_count <= 1 {
    out.push_str("_Single entry point — no cross-entry sharing to analyze._\n");
  } else if report.modules.shared_across_entries.is_empty() {
    out.push_str("_No module is reachable from more than one entry point._\n");
  } else {
    writeln!(
      out,
      "{}, {} shared:\n",
      plural(report.modules.entry_point_count, "entry point"),
      plural(report.modules.shared_across_entries_count, "module"),
    )
    .unwrap();
    out.push_str("| Entries | Module |\n| --- | --- |\n");
    for row in &report.modules.shared_across_entries {
      writeln!(out, "| {} | `{}` |", row.entries, row.module).unwrap();
    }
  }
  out
}

pub(crate) fn render_packages(report: &Report) -> String {
  let mut out = String::new();
  out.push_str("# Packages\n\n");
  writeln!(
    out,
    "{} ({} direct, {} transitive). Top {} by rendered size:\n",
    plural(report.packages.count, "package"),
    report.packages.direct_count,
    report.packages.transitive_count,
    report.packages.largest.len(),
  )
  .unwrap();
  if report.packages.largest.is_empty() {
    out.push_str("_No package graph available._\n");
    return out;
  }
  out.push_str("| Size | Type | Used? | Modules | Package |\n| --- | --- | --- | --- | --- |\n");
  for package in &report.packages.largest {
    let name = match &package.version {
      Some(version) => format!("{}@{version}", package.name),
      None => package.name.clone(),
    };
    writeln!(
      out,
      "| {} | {} | {} | {} | `{name}` |",
      format_size(package.bytes),
      package.dependency_type,
      if package.used { "yes" } else { "no" },
      package.module_count,
    )
    .unwrap();
  }

  out.push_str("\n## Duplicate versions\n\n");
  out.push_str("Same package shipped at multiple versions — deduping can cut size.\n\n");
  if report.packages.duplicate_versions.is_empty() {
    out.push_str("_None — every package resolves to a single version._\n");
  } else {
    writeln!(out, "{} affected:\n", plural(report.packages.duplicate_version_count, "package"))
      .unwrap();
    out.push_str("| Package | Versions (size) |\n| --- | --- |\n");
    for dup in &report.packages.duplicate_versions {
      let vs = dup
        .versions
        .iter()
        .map(|v| format!("{} ({})", v.version, format_size(v.bytes)))
        .collect::<Vec<_>>()
        .join(", ");
      writeln!(out, "| `{}` | {vs} |", dup.name).unwrap();
    }
  }
  out
}

pub(crate) fn render_graph(report: &Report) -> String {
  let mut out = String::new();
  out.push_str("# Entry Points & Chunk Graph\n\n");
  writeln!(
    out,
    "{}, {}.\n",
    plural(report.entries.len(), "entry point"),
    plural(report.chunks.count, "non-empty chunk"),
  )
  .unwrap();
  if report.entries.is_empty() {
    out.push_str("_No entry chunks._\n");
    return out;
  }
  let names =
    |labels: &[String]| labels.iter().map(|l| format!("`{l}`")).collect::<Vec<_>>().join(", ");
  for entry in &report.entries {
    writeln!(out, "### Entry: `{}`\n", entry.entry).unwrap();
    writeln!(out, "- **Output chunk**: `{}` ({})", entry.chunk, format_size(entry.chunk_bytes))
      .unwrap();
    writeln!(
      out,
      "- **Initial load**: {} (this chunk + its transitive static-import chunks)",
      format_size(entry.initial_load_bytes),
    )
    .unwrap();
    if entry.static_imports.is_empty() {
      out.push_str("- **Static imports**: none\n");
    } else {
      writeln!(out, "- **Static imports**: {}", names(&entry.static_imports)).unwrap();
    }
    if !entry.dynamic_imports.is_empty() {
      writeln!(out, "- **Dynamic imports**: {}", names(&entry.dynamic_imports)).unwrap();
    }
    out.push('\n');
  }
  out
}

pub(crate) fn render_delta(report: &Report) -> String {
  let mut out = String::new();
  out.push_str("# Build-over-Build Delta\n\n");
  out.push_str("Machine-readable version: `metrics.json` → `delta` / `baselineDelta`.\n\n");
  if let Some(delta) = &report.delta {
    render_delta_section(&mut out, delta, "Vs previous build");
  }
  if let Some(baseline) = &report.baseline_delta {
    render_delta_section(&mut out, baseline, "Vs pinned baseline (`baseline.json`)");
  }
  out
}

fn render_delta_section(out: &mut String, delta: &DeltaSection, heading: &str) {
  writeln!(out, "## {heading}\n").unwrap();
  out.push_str("| Metric | Previous | Current | Δ | Δ% |\n| --- | --- | --- | --- | --- |\n");
  for (id, row) in &delta.metrics {
    writeln!(
      out,
      "| `{id}` | {} | {} | {} | {} |",
      format_metric_value(id, row.prev),
      format_metric_value(id, row.curr),
      format_metric_delta(id, row.delta),
      format_pct(row.pct),
    )
    .unwrap();
  }

  if !delta.entries.is_empty() {
    out.push_str("\n### Per-entry initial load\n\n");
    out.push_str("| Entry | Previous | Current | Δ | Δ% |\n| --- | --- | --- | --- | --- |\n");
    for entry in &delta.entries {
      let row = &entry.initial_load_bytes;
      writeln!(
        out,
        "| `{}` | {} | {} | {} | {} |",
        entry.entry,
        format_size(row.prev.as_f64() as u64),
        format_size(row.curr.as_f64() as u64),
        format_metric_delta("initial_load_bytes", row.delta),
        format_pct(row.pct),
      )
      .unwrap();
    }
  }
  if !delta.added_entries.is_empty() {
    let list = delta.added_entries.iter().map(|e| format!("`{e}`")).collect::<Vec<_>>().join(", ");
    writeln!(out, "\nAdded entries: {list}").unwrap();
  }
  if !delta.removed_entries.is_empty() {
    let list =
      delta.removed_entries.iter().map(|e| format!("`{e}`")).collect::<Vec<_>>().join(", ");
    writeln!(out, "\nRemoved entries: {list}").unwrap();
  }
  out.push('\n');
}

fn format_metric_value(id: &str, value: MetricValue) -> String {
  if id.ends_with("_ms") {
    format_ms(value.as_f64())
  } else if id.ends_with("_bytes") {
    format_size(value.as_f64() as u64)
  } else {
    format!("{}", value.as_f64() as u64)
  }
}

fn format_metric_delta(id: &str, delta: f64) -> String {
  let sign = if delta >= 0.0 { "+" } else { "-" };
  let abs = delta.abs();
  if id.ends_with("_ms") {
    format!("{sign}{}", format_ms(abs))
  } else if id.ends_with("_bytes") {
    format!("{sign}{}", format_size(abs as u64))
  } else {
    format!("{sign}{}", abs as u64)
  }
}

fn format_pct(pct: Option<f64>) -> String {
  pct.map_or_else(|| "n/a".to_string(), |p| format!("{p:+.1}%"))
}

fn format_noop(noop: usize, calls: usize) -> String {
  if noop == 0 || calls == 0 {
    "0".to_string()
  } else {
    format!("{noop} ({:.0}%)", noop as f64 / calls as f64 * 100.0)
  }
}

fn format_size(bytes: u64) -> String {
  if bytes < 1024 {
    format!("{bytes} B")
  } else if bytes < 1024 * 1024 {
    format!("{:.1} kB", bytes as f64 / 1024.0)
  } else {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
  }
}

fn format_ms(ms: f64) -> String {
  if ms >= 1000.0 {
    format!("{:.2} s", ms / 1000.0)
  } else if ms >= 1.0 {
    format!("{ms:.1} ms")
  } else if ms > 0.0 {
    format!("{:.0} µs", ms * 1000.0)
  } else {
    "0 ms".to_string()
  }
}

fn plural(n: usize, word: &str) -> String {
  if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

/// The self-describing contract dropped into the metrics directory, so agents that merely
/// list the directory learn how to read it and how to run a config experiment against it.
const AGENTS_MD: &str = r#"# Rolldown build metrics — agent guide

This directory is generated by Rolldown's devtools metrics mode
(`devtools: { mode: 'metrics' }`). It summarizes one build session; every list is bounded
(top-N), so the directory stays small no matter how large the app is.

## Files

The markdown files are the reading layer — start at `entry.md`, load detail files on demand.
`metrics.json` is the computation layer — query a metric id or read only its delta sections;
don't load the whole file into context.

| File | Contents |
| --- | --- |
| `entry.md` | One-paragraph summary + file index. Start here for orientation. |
| `metrics.json` | Canonical machine-readable report (schema-versioned): flat `metrics` id map, sections, `delta`, `baselineDelta`. For tools and precise lookups. |
| `timing.md` | Where build time went: scan/generate split, per-plugin hook cost, transform hotspots. |
| `chunks.md` | Output composition: chunk reasons/sizes, largest assets, cross-chunk duplication. |
| `modules.md` | Module graph: import kinds, most-imported, shared-across-entries. |
| `packages.md` | Dependency bloat: largest packages, duplicate versions. |
| `graph.md` | Per-entry chunk graph and initial-load bytes. |
| `delta.md` | This build vs the previous one — and vs the pinned baseline, if any. |
| `history.jsonl` | One JSON line per build: `{schemaVersion, tsMs, build, metrics, entries}`. |
| `.state.json` | Internal: previous-build snapshot used to compute the next `delta`. |
| `baseline.json` | Optional, user-pinned: copy `.state.json` here to make every following build also report `baselineDelta` against this fixed reference. Delete to unpin. |

## Metric ids (`metrics.json` → `metrics`, `delta.metrics`, `history.jsonl`)

Stable, append-only ids; the unit is suffixed (`_ms` milliseconds, `_bytes` bytes, counts
otherwise).

| Id | Meaning |
| --- | --- |
| `build.total_ms` | Wall time from build start to final build end (scan + generate). |
| `build.scan_ms` | Resolve/load/transform (scan) stage wall time. |
| `build.generate_ms` | Link/chunk/render (generate) stage wall time. |
| `output.total_bytes` | All emitted assets (sourcemaps excluded). |
| `output.js_bytes` / `output.css_bytes` / `output.other_bytes` | Byte split by asset type. |
| `output.chunk_count` / `output.asset_count` / `output.entry_count` | Output shape. |
| `output.max_initial_load_bytes` | Worst entry's initial load (entry chunk + transitive static-import chunks). |
| `modules.count` / `modules.external_count` | Module graph size. |
| `modules.shared_across_entries_count` | Modules reachable from 2+ entries (shared-chunk candidates). |
| `chunks.duplicated_module_count` | Modules bundled into >1 chunk (shipped multiple times). |
| `packages.count` / `packages.direct_count` / `packages.transitive_count` | Package graph size. |
| `packages.duplicate_version_count` | Packages shipped at multiple versions (dedupe = size win). |
| `plugins.hook_call_count` / `plugins.hook_time_ms` | Plugin hook volume and cost (summed across concurrent calls). |
| `plugins.hook_noop_call_count` | Hook calls that did no work (returned nothing, or transform returned the code unchanged). High share ⇒ plugins should declare hook filters; compare across builds to verify a filter change. |

Per-entry numbers live in `metrics.json` → `entries`, joined across builds by `entry`.

## Running a config experiment

1. Build once — this is the reference build (`.state.json` is written). To keep it as a
   fixed reference across several attempts, pin it: copy `.state.json` to `baseline.json`.
2. Change the config (or code) and build again.
3. Read `metrics.json` → `delta` (vs the previous build) and, when pinned, `baselineDelta`
   (vs the fixed baseline — unaffected by intermediate experiments). Each is per-metric
   `{prev, curr, delta, pct}` plus per-entry initial-load changes and added/removed entries.
   `delta.md` shows the same data as tables.
4. Trying N configs? Judge each attempt by `baselineDelta`, not the chain `delta`;
   `history.jsonl` accumulates one line per build for the whole series.

Sizes are raw (pre-compression) bytes. Durations are wall-clock, measured at event emission;
plugin hook time is summed across concurrent calls and can exceed wall-clock on parallel builds.
"#;
