//! Renders the aggregated metrics into the progressive markdown directory + delta state.

use std::{
  fmt::Write as _,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{HookStat, MetricsAggregator};

/// Minimal snapshot persisted to `.state.json` purely to compute the next build's delta.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetricsState {
  total_build_ms: u64,
  total_bytes: u64,
  modules: usize,
  chunks: usize,
  assets: usize,
  packages: usize,
}

impl MetricsAggregator {
  fn build_total_ms(&self) -> u64 {
    match (self.build_start, self.build_end) {
      (Some(start), Some(end)) if end >= start => {
        u64::try_from(end.duration_since(start).as_millis()).unwrap_or(u64::MAX)
      }
      _ => 0,
    }
  }

  fn chunk_count(&self) -> usize {
    self.chunks.len()
  }

  fn package_count(&self) -> usize {
    self.package_direct + self.package_transitive
  }

  fn state(&self) -> MetricsState {
    MetricsState {
      total_build_ms: self.build_total_ms(),
      total_bytes: self.total_bytes,
      modules: self.module_count,
      chunks: self.chunk_count(),
      assets: self.asset_count,
      packages: self.package_count(),
    }
  }

  /// Per-plugin aggregated `(count, micros)`, summed across hook types, sorted slowest-first.
  fn plugins_by_time(&self) -> Vec<(String, usize, u64)> {
    let mut by_plugin: rustc_hash::FxHashMap<&str, (usize, u64)> = rustc_hash::FxHashMap::default();
    for ((plugin, _hook), stat) in &self.hook_calls {
      let entry = by_plugin.entry(plugin.as_str()).or_default();
      entry.0 += stat.count;
      entry.1 += stat.micros;
    }
    let mut rows: Vec<(String, usize, u64)> =
      by_plugin.into_iter().map(|(name, (count, micros))| (name.to_string(), count, micros)).collect();
    rows.sort_by(|a, b| b.2.cmp(&a.2));
    rows
  }

  fn stabilize(&self, id: &str) -> String {
    if let Some(cwd) = &self.cwd {
      if let Some(rest) = id.strip_prefix(cwd.as_str()) {
        let rest = rest.trim_start_matches(['/', '\\']);
        if !rest.is_empty() {
          return rest.replace('\\', "/");
        }
      }
    }
    if let Some(stripped) = id.strip_prefix('\0') {
      return format!("\\0{stripped}");
    }
    id.replace('\\', "/")
  }

  /// Human-friendly chunk label: prefer the emitted asset filename, then the chunk's own name.
  fn chunk_label(&self, id: u32) -> String {
    self
      .chunk_filenames
      .get(&id)
      .cloned()
      .or_else(|| self.chunks.get(&id).map(|chunk| chunk.name.clone()))
      .unwrap_or_else(|| format!("chunk-{id}"))
  }

  fn out_dir(&self) -> PathBuf {
    let dir = &self.config.dir;
    if Path::new(dir).is_absolute() {
      PathBuf::from(dir)
    } else {
      let cwd = self.cwd.clone().unwrap_or_else(|| ".".to_string());
      Path::new(&cwd).join(dir)
    }
  }

  /// Render the report directory + persist delta state. Errors are returned for the caller to log;
  /// a metrics-writing failure must never fail the build.
  pub fn render(&self) -> std::io::Result<()> {
    let dir = self.out_dir();
    std::fs::create_dir_all(&dir)?;

    let prev = if self.config.delta {
      std::fs::read_to_string(dir.join(".state.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<MetricsState>(&raw).ok())
    } else {
      None
    };

    std::fs::write(dir.join("entry.md"), self.render_entry(prev.as_ref()))?;
    std::fs::write(dir.join("timing.md"), self.render_timing())?;
    std::fs::write(dir.join("chunks.md"), self.render_chunks())?;
    std::fs::write(dir.join("modules.md"), self.render_modules())?;
    std::fs::write(dir.join("packages.md"), self.render_packages())?;
    std::fs::write(dir.join("graph.md"), self.render_graph())?;
    if let Some(prev) = prev.as_ref() {
      std::fs::write(dir.join("delta.md"), self.render_delta(prev))?;
    }

    if self.config.delta {
      if let Ok(state) = serde_json::to_string_pretty(&self.state()) {
        std::fs::write(dir.join(".state.json"), state)?;
      }
    }
    Ok(())
  }

  fn render_entry(&self, prev: Option<&MetricsState>) -> String {
    let mut out = String::new();
    out.push_str("# Build Metrics (devtools metrics mode)\n\n");
    writeln!(
      out,
      "Build {} · output {} · {} modules ({} external) / {} chunks / {} assets / {} packages · {} / {}\n",
      format_ms(self.build_total_ms()),
      format_size(self.total_bytes),
      self.module_count,
      self.external_count,
      self.chunk_count(),
      self.asset_count,
      self.package_count(),
      self.platform.as_deref().unwrap_or("?"),
      self.format.as_deref().unwrap_or("?"),
    )
    .unwrap();

    if let Some(prev) = prev {
      writeln!(
        out,
        "Δ vs previous: build {}, size {}\n",
        pct_change(prev.total_build_ms, self.build_total_ms()),
        pct_change(prev.total_bytes, self.total_bytes),
      )
      .unwrap();
    }

    out.push_str("## Details (load on demand)\n\n");
    out.push_str("| Report | Load this when… |\n| --- | --- |\n");
    out.push_str("| `timing.md` | investigating slow builds / plugin & hook cost |\n");
    out.push_str("| `chunks.md` | reducing bundle size, inspecting chunk composition & reasons |\n");
    out.push_str("| `modules.md` | module graph: import kinds, most-imported, transform hotspots |\n");
    out.push_str("| `packages.md` | dependency bloat: largest npm packages, direct vs transitive |\n");
    out.push_str("| `graph.md` | code-splitting: entry points & chunk import graph |\n");
    if prev.is_some() {
      out.push_str("| `delta.md` | checking this build for regressions vs the last |\n");
    }
    out.push('\n');
    out.push_str(
      "> Derived in-memory from the rolldown devtools event stream (same data that feeds Vite \
       DevTools). Timing is approximate (measured on the devtools writer thread). Sizes are \
       per-asset/chunk and per-package; per-module byte size is not in the devtools stream.\n",
    );
    out
  }

  fn render_timing(&self) -> String {
    let mut out = String::new();
    out.push_str("# Build Timing\n\n");
    writeln!(out, "- **Total build (approx)**: {}\n", format_ms(self.build_total_ms())).unwrap();

    out.push_str("## Hook cost by plugin & type\n\n");
    out.push_str("Call counts are exact; durations are approximate (devtools-writer-side).\n\n");
    let mut rows: Vec<(&str, &str, &HookStat)> =
      self.hook_calls.iter().map(|((p, h), s)| (p.as_str(), *h, s)).collect();
    rows.sort_by(|a, b| b.2.micros.cmp(&a.2.micros));
    if rows.is_empty() {
      out.push_str("_No plugin hook calls recorded._\n\n");
    } else {
      out.push_str("| Plugin | Hook | Calls | ~Time |\n| --- | --- | --- | --- |\n");
      for (plugin, hook, stat) in rows.iter().take(self.config.top_n) {
        writeln!(out, "| `{plugin}` | {hook} | {} | {} |", stat.count, format_us(stat.micros))
          .unwrap();
      }
      out.push('\n');
    }

    out.push_str("## Slowest plugins (all hooks)\n\n");
    let plugins = self.plugins_by_time();
    if plugins.is_empty() {
      out.push_str("_None recorded._\n\n");
    } else {
      out.push_str("| Plugin | Calls | ~Time |\n| --- | --- | --- |\n");
      for (name, count, micros) in plugins.iter().take(self.config.top_n) {
        writeln!(out, "| `{name}` | {count} | {} |", format_us(*micros)).unwrap();
      }
      out.push('\n');
    }

    out.push_str("## Transform hotspots (modules)\n\n");
    let mut modules: Vec<(&String, &HookStat)> = self.module_transform.iter().collect();
    modules.sort_by(|a, b| b.1.micros.cmp(&a.1.micros));
    if modules.is_empty() {
      out.push_str("_No transforms recorded._\n");
    } else {
      out.push_str("| ~Time | Calls | Module |\n| --- | --- | --- |\n");
      for (id, stat) in modules.iter().take(self.config.top_n) {
        writeln!(out, "| {} | {} | `{}` |", format_us(stat.micros), stat.count, self.stabilize(id))
          .unwrap();
      }
    }
    out
  }

  fn render_chunks(&self) -> String {
    let mut out = String::new();
    out.push_str("# Chunks\n\n");
    writeln!(
      out,
      "Output {} ({} JS, {} CSS, {} other) across {} chunks / {} assets.\n",
      format_size(self.total_bytes),
      format_size(self.js_bytes),
      format_size(self.css_bytes),
      format_size(self.other_bytes),
      self.chunk_count(),
      self.asset_count,
    )
    .unwrap();

    out.push_str("## Chunk reasons\n\n| Reason | Count |\n| --- | --- |\n");
    let mut reasons: Vec<(&String, &usize)> = self.chunk_reason_hist.iter().collect();
    reasons.sort_by(|a, b| b.1.cmp(a.1));
    for (reason, count) in reasons {
      writeln!(out, "| {reason} | {count} |").unwrap();
    }
    out.push('\n');

    out.push_str("## Largest chunks\n\n");
    let mut chunks: Vec<(u32, &crate::ChunkAgg, u64)> = self
      .chunks
      .iter()
      .map(|(id, chunk)| (*id, chunk, self.chunk_sizes.get(id).copied().unwrap_or(0)))
      .collect();
    chunks.sort_by(|a, b| b.2.cmp(&a.2));
    out.push_str("| Bytes | Entry? | Reason | Modules | Chunk |\n| --- | --- | --- | --- | --- |\n");
    for (id, chunk, size) in chunks.iter().take(self.config.top_n) {
      writeln!(
        out,
        "| {} | {} | {} | {} | `{}` |",
        format_size(*size),
        if chunk.is_entry { "yes" } else { "no" },
        chunk.reason,
        chunk.module_count,
        self.chunk_label(*id),
      )
      .unwrap();
    }

    out.push_str("\n## Largest assets\n\n");
    out.push_str("Includes chunk JS and standalone assets (CSS, images, …); sourcemaps excluded.\n\n");
    out.push_str("| Bytes | Asset |\n| --- | --- |\n");
    for asset in &self.assets {
      writeln!(out, "| {} | `{}` |", format_size(asset.size), asset.filename).unwrap();
    }
    out
  }

  fn render_modules(&self) -> String {
    let mut out = String::new();
    out.push_str("# Modules\n\n");
    writeln!(out, "{} modules ({} external).\n", self.module_count, self.external_count).unwrap();

    out.push_str("## Import kinds\n\n| Kind | Count |\n| --- | --- |\n");
    let mut kinds: Vec<(&String, &usize)> = self.import_kind_hist.iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(a.1));
    for (kind, count) in kinds {
      writeln!(out, "| {kind} | {count} |").unwrap();
    }
    out.push('\n');

    out.push_str("## Most-imported modules\n\n");
    if self.most_imported.is_empty() {
      out.push_str("_No shared modules detected._\n");
    } else {
      out.push_str("| Importers | Module |\n| --- | --- |\n");
      for (id, count) in &self.most_imported {
        writeln!(out, "| {count} | `{}` |", self.stabilize(id)).unwrap();
      }
    }
    out
  }

  fn render_packages(&self) -> String {
    let mut out = String::new();
    out.push_str("# Packages\n\n");
    writeln!(
      out,
      "{} packages ({} direct, {} transitive). Top {} by rendered size:\n",
      self.package_count(),
      self.package_direct,
      self.package_transitive,
      self.packages.len(),
    )
    .unwrap();
    if self.packages.is_empty() {
      out.push_str("_No package graph available._\n");
      return out;
    }
    out.push_str("| Size | Type | Used? | Modules | Package |\n| --- | --- | --- | --- | --- |\n");
    for package in &self.packages {
      let name = match &package.version {
        Some(version) => format!("{}@{version}", package.name),
        None => package.name.clone(),
      };
      writeln!(
        out,
        "| {} | {} | {} | {} | `{name}` |",
        format_size(package.size),
        package.dependency_type,
        if package.is_used { "yes" } else { "no" },
        package.module_count,
      )
      .unwrap();
    }
    out
  }

  fn render_graph(&self) -> String {
    let mut out = String::new();
    out.push_str("# Entry Points & Chunk Graph\n\n");
    let mut entries: Vec<(&u32, &crate::ChunkAgg)> =
      self.chunks.iter().filter(|(_, chunk)| chunk.is_entry).collect();
    entries.sort_by(|a, b| {
      self.chunk_sizes.get(b.0).copied().unwrap_or(0).cmp(&self.chunk_sizes.get(a.0).copied().unwrap_or(0))
    });
    if entries.is_empty() {
      out.push_str("_No entry chunks._\n");
      return out;
    }
    for (id, chunk) in entries.iter().take(self.config.top_n) {
      let entry_module =
        chunk.entry_module.as_ref().map_or_else(|| chunk.name.clone(), |m| self.stabilize(m));
      writeln!(out, "### Entry: `{entry_module}`\n").unwrap();
      writeln!(
        out,
        "- **Output chunk**: `{}` ({})",
        self.chunk_label(**id),
        format_size(self.chunk_sizes.get(id).copied().unwrap_or(0)),
      )
      .unwrap();
      let names = |ids: &[u32]| {
        ids.iter().map(|i| format!("`{}`", self.chunk_label(*i))).collect::<Vec<_>>().join(", ")
      };
      if chunk.static_imports.is_empty() {
        out.push_str("- **Static imports**: none\n");
      } else {
        writeln!(out, "- **Static imports**: {}", names(&chunk.static_imports)).unwrap();
      }
      if !chunk.dynamic_imports.is_empty() {
        writeln!(out, "- **Dynamic imports**: {}", names(&chunk.dynamic_imports)).unwrap();
      }
      out.push('\n');
    }
    out
  }

  fn render_delta(&self, prev: &MetricsState) -> String {
    let mut out = String::new();
    out.push_str("# Build-over-Build Delta\n\n");
    out.push_str("| Metric | Previous | Current | Δ |\n| --- | --- | --- | --- |\n");
    writeln!(
      out,
      "| Total build time | {} | {} | {} |",
      format_ms(prev.total_build_ms),
      format_ms(self.build_total_ms()),
      pct_change(prev.total_build_ms, self.build_total_ms()),
    )
    .unwrap();
    writeln!(
      out,
      "| Total output size | {} | {} | {} |",
      format_size(prev.total_bytes),
      format_size(self.total_bytes),
      pct_change(prev.total_bytes, self.total_bytes),
    )
    .unwrap();
    writeln!(out, "| Modules | {} | {} | {} |", prev.modules, self.module_count, idiff(prev.modules, self.module_count)).unwrap();
    writeln!(out, "| Chunks | {} | {} | {} |", prev.chunks, self.chunk_count(), idiff(prev.chunks, self.chunk_count())).unwrap();
    writeln!(out, "| Assets | {} | {} | {} |", prev.assets, self.asset_count, idiff(prev.assets, self.asset_count)).unwrap();
    writeln!(out, "| Packages | {} | {} | {} |", prev.packages, self.package_count(), idiff(prev.packages, self.package_count())).unwrap();
    out
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

fn format_ms(ms: u64) -> String {
  if ms < 1000 {
    format!("{ms} ms")
  } else {
    format!("{:.2} s", ms as f64 / 1000.0)
  }
}

fn format_us(micros: u64) -> String {
  if micros < 1000 {
    format!("{micros} µs")
  } else {
    format_ms(micros / 1000)
  }
}

fn pct_change(prev: u64, curr: u64) -> String {
  if prev == 0 {
    return "n/a".to_string();
  }
  let pct = (curr as f64 - prev as f64) / prev as f64 * 100.0;
  format!("{pct:+.1}%")
}

fn idiff(prev: usize, curr: usize) -> String {
  if curr >= prev {
    format!("+{}", curr - prev)
  } else {
    format!("-{}", prev - curr)
  }
}
