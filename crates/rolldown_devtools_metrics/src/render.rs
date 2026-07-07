//! Renders the aggregated metrics into the progressive markdown directory + delta state.

use std::{
  fmt::Write as _,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{ChunkAgg, HookStat, MetricsAggregator};

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

  /// Real (non-empty) output chunk count. Pure placeholders — 0 bytes AND 0 modules, e.g. the
  /// runtime stub or async-entry stubs whose code was merged elsewhere — are excluded.
  fn chunk_count(&self) -> usize {
    self
      .chunks
      .iter()
      .filter(|(id, c)| c.module_count > 0 || self.chunk_sizes.get(id).copied().unwrap_or(0) > 0)
      .count()
  }

  /// Non-empty chunks as `(id, agg, bytes)`, sorted by size desc with a deterministic tie-break.
  fn chunks_with_size(&self) -> Vec<(u32, &ChunkAgg, u64)> {
    let mut chunks: Vec<(u32, &ChunkAgg, u64)> = self
      .chunks
      .iter()
      .map(|(id, c)| (*id, c, self.chunk_sizes.get(id).copied().unwrap_or(0)))
      .filter(|(_, c, size)| c.module_count > 0 || *size > 0)
      .collect();
    chunks
      .sort_by(|a, b| b.2.cmp(&a.2).then_with(|| self.chunk_label(a.0).cmp(&self.chunk_label(b.0))));
    chunks
  }

  /// #2: modules bundled into >1 chunk (they ship multiple times). Returns (total, top-N rows of
  /// `(module, distinct chunk labels)`), most-duplicated first.
  fn duplicated_modules(&self) -> (usize, Vec<(String, Vec<String>)>) {
    let mut all: Vec<(String, Vec<String>)> = self
      .module_chunks
      .iter()
      .filter_map(|(id, chunks)| {
        let mut labels: Vec<String> = chunks.iter().map(|c| self.chunk_label(*c)).collect();
        labels.sort();
        labels.dedup();
        (labels.len() > 1).then(|| (self.stabilize(id), labels))
      })
      .collect();
    all.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    let total = all.len();
    all.truncate(self.config.top_n);
    (total, all)
  }

  /// #3: modules reachable from >1 entry point (the real shared-chunk signal, vs raw import
  /// fan-in). Returns (entry count, top-N rows of `(module, #entries reaching it)`).
  fn reach_from_entries(&self) -> (usize, Vec<(String, usize)>) {
    let mut entries: Vec<&str> =
      self.chunks.values().filter(|c| c.is_entry).filter_map(|c| c.entry_module.as_deref()).collect();
    entries.sort_unstable();
    entries.dedup();
    let mut count: rustc_hash::FxHashMap<&str, usize> = rustc_hash::FxHashMap::default();
    for entry in &entries {
      let mut seen: rustc_hash::FxHashSet<&str> = rustc_hash::FxHashSet::default();
      let mut stack = vec![*entry];
      while let Some(m) = stack.pop() {
        if !seen.insert(m) {
          continue;
        }
        if let Some(deps) = self.module_imports.get(m) {
          for dep in deps {
            stack.push(dep.as_str());
          }
        }
      }
      for m in seen {
        *count.entry(m).or_default() += 1;
      }
    }
    let mut rows: Vec<(String, usize)> =
      count.into_iter().filter(|(_, n)| *n >= 2).map(|(m, n)| (self.stabilize(m), n)).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(self.config.top_n);
    (entries.len(), rows)
  }

  /// #4: bytes loaded on first paint for an entry chunk = its own size + all transitively
  /// STATIC-imported chunks (dynamic imports are lazy, so excluded).
  fn initial_load_bytes(&self, entry_chunk: u32) -> u64 {
    let mut seen: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
    let mut stack = vec![entry_chunk];
    let mut total = 0u64;
    while let Some(cid) = stack.pop() {
      if !seen.insert(cid) {
        continue;
      }
      total += self.chunk_sizes.get(&cid).copied().unwrap_or(0);
      if let Some(chunk) = self.chunks.get(&cid) {
        for &s in &chunk.static_imports {
          stack.push(s);
        }
      }
    }
    total
  }

  /// #6: package names shipped at >1 distinct version, each with its versions + sizes.
  fn duplicate_package_versions(&self) -> Vec<(String, Vec<(String, u64)>)> {
    let mut dups: Vec<(String, Vec<(String, u64)>)> = self
      .package_versions
      .iter()
      .filter_map(|(name, versions)| {
        let mut distinct: Vec<(String, u64)> = Vec::new();
        for (v, s) in versions {
          if !distinct.iter().any(|(dv, _)| dv == v) {
            distinct.push((v.clone(), *s));
          }
        }
        distinct.sort();
        (distinct.len() > 1).then(|| (name.clone(), distinct))
      })
      .collect();
    dups.sort_by(|a, b| a.0.cmp(&b.0));
    dups.truncate(self.config.top_n);
    dups
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
    rows.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    rows
  }

  fn stabilize(&self, id: &str) -> String {
    let normalized = id.replace('\\', "/");
    // node_modules paths -> package-relative (e.g. `picomatch/lib/utils.js`). Slicing after the
    // LAST `/node_modules/` also strips pnpm's `.pnpm/<pkg>@<ver>/node_modules/` nesting, and is
    // independent of where node_modules lives (project, store, symlink, …).
    if let Some(pos) = normalized.rfind("/node_modules/") {
      return normalized[pos + "/node_modules/".len()..].to_string();
    }
    if let Some(rest) = normalized.strip_prefix("node_modules/") {
      return rest.to_string();
    }
    // Project-relative (strip the session cwd).
    if let Some(cwd) = &self.cwd {
      let cwd = cwd.replace('\\', "/");
      if let Some(rest) = normalized.strip_prefix(&cwd) {
        let rest = rest.trim_start_matches('/');
        if !rest.is_empty() {
          return rest.to_string();
        }
      }
    }
    // Virtual modules (`\0`-prefixed).
    if let Some(stripped) = normalized.strip_prefix('\0') {
      return format!("\\0{stripped}");
    }
    normalized
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
      "Build {} · output {} · {} ({} external) / {} / {} / {} · {} / {}\n",
      format_ms(self.build_total_ms()),
      format_size(self.total_bytes),
      plural(self.module_count, "module"),
      self.external_count,
      plural(self.chunk_count(), "chunk"),
      plural(self.asset_count, "asset"),
      plural(self.package_count(), "package"),
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
    out.push_str("| `chunks.md` | bundle size: chunk composition, reasons & cross-chunk duplication |\n");
    out.push_str("| `modules.md` | module graph: import kinds, most-imported, shared-across-entries |\n");
    out.push_str("| `packages.md` | dependency bloat: largest packages, direct/transitive, duplicate versions |\n");
    out.push_str("| `graph.md` | code-splitting: entry points, chunk import graph & initial-load cost |\n");
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
    rows.sort_by(|a, b| b.2.micros.cmp(&a.2.micros).then_with(|| a.0.cmp(b.0)).then_with(|| a.1.cmp(b.1)));
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
    modules.sort_by(|a, b| b.1.micros.cmp(&a.1.micros).then_with(|| a.0.cmp(b.0)));
    if modules.is_empty() {
      out.push_str(
        "_No plugin `transform` hooks ran. (Core TS/JSX/TSX transformation happens inside \
         rolldown, not via a plugin hook, so it isn't attributed per-module here.)_\n",
      );
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
      "Output {} ({} JS, {} CSS, {} other) across {} / {}.\n",
      format_size(self.total_bytes),
      format_size(self.js_bytes),
      format_size(self.css_bytes),
      format_size(self.other_bytes),
      plural(self.chunk_count(), "chunk"),
      plural(self.asset_count, "asset"),
    )
    .unwrap();

    let chunks = self.chunks_with_size();

    out.push_str("## Chunk reasons\n\n| Reason | Count |\n| --- | --- |\n");
    let mut hist: rustc_hash::FxHashMap<&str, usize> = rustc_hash::FxHashMap::default();
    for (_, chunk, _) in &chunks {
      *hist.entry(chunk.reason.as_str()).or_default() += 1;
    }
    let mut reasons: Vec<(&str, usize)> = hist.into_iter().collect();
    reasons.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    for (reason, count) in reasons {
      writeln!(out, "| {reason} | {count} |").unwrap();
    }
    out.push('\n');

    out.push_str("## Largest chunks\n\n");
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

    out.push_str("\n## Duplicated modules (in >1 chunk)\n\n");
    out.push_str(
      "Modules bundled into multiple chunks ship multiple times — candidates to hoist into a \
       shared chunk.\n\n",
    );
    let (dup_total, dups) = self.duplicated_modules();
    if dups.is_empty() {
      out.push_str("_None — no module is bundled into more than one chunk._\n");
    } else {
      writeln!(out, "{} duplicated; most-duplicated:\n", plural(dup_total, "module")).unwrap();
      out.push_str("| Chunks | Module | In |\n| --- | --- | --- |\n");
      for (id, labels) in &dups {
        let inn = labels.iter().map(|l| format!("`{l}`")).collect::<Vec<_>>().join(", ");
        writeln!(out, "| {} | `{id}` | {inn} |", labels.len()).unwrap();
      }
    }
    out
  }

  fn render_modules(&self) -> String {
    let mut out = String::new();
    out.push_str("# Modules\n\n");
    writeln!(out, "{} ({} external).\n", plural(self.module_count, "module"), self.external_count)
      .unwrap();

    out.push_str("## Import kinds\n\n| Kind | Count |\n| --- | --- |\n");
    let mut kinds: Vec<(&String, &usize)> = self.import_kind_hist.iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(a.1));
    for (kind, count) in kinds {
      writeln!(out, "| {kind} | {count} |").unwrap();
    }
    out.push('\n');

    out.push_str("## Most-imported modules\n\n");
    out.push_str("Modules imported by 2+ modules — shared-chunk candidates.\n\n");
    if self.most_imported.is_empty() {
      out.push_str("_No module is imported by more than one other module._\n");
    } else {
      out.push_str("| Importers | Module |\n| --- | --- |\n");
      for (id, count) in &self.most_imported {
        writeln!(out, "| {count} | `{}` |", self.stabilize(id)).unwrap();
      }
    }

    out.push_str("\n## Shared across entry points\n\n");
    out.push_str(
      "Modules reachable from 2+ entry points — the real shared-chunk signal (vs. raw import \
       fan-in above, which counts any importer).\n\n",
    );
    let (entry_count, shared) = self.reach_from_entries();
    if entry_count <= 1 {
      out.push_str("_Single entry point — no cross-entry sharing to analyze._\n");
    } else if shared.is_empty() {
      out.push_str("_No module is reachable from more than one entry point._\n");
    } else {
      writeln!(out, "{}:\n", plural(entry_count, "entry point")).unwrap();
      out.push_str("| Entries | Module |\n| --- | --- |\n");
      for (id, n) in &shared {
        writeln!(out, "| {n} | `{id}` |").unwrap();
      }
    }
    out
  }

  fn render_packages(&self) -> String {
    let mut out = String::new();
    out.push_str("# Packages\n\n");
    writeln!(
      out,
      "{} ({} direct, {} transitive). Top {} by rendered size:\n",
      plural(self.package_count(), "package"),
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

    out.push_str("\n## Duplicate versions\n\n");
    out.push_str("Same package shipped at multiple versions — deduping can cut size.\n\n");
    let dups = self.duplicate_package_versions();
    if dups.is_empty() {
      out.push_str("_None — every package resolves to a single version._\n");
    } else {
      out.push_str("| Package | Versions (size) |\n| --- | --- |\n");
      for (name, versions) in &dups {
        let vs =
          versions.iter().map(|(v, s)| format!("{v} ({})", format_size(*s))).collect::<Vec<_>>().join(", ");
        writeln!(out, "| `{name}` | {vs} |").unwrap();
      }
    }
    out
  }

  fn render_graph(&self) -> String {
    let mut out = String::new();
    out.push_str("# Entry Points & Chunk Graph\n\n");
    let entries: Vec<(u32, &ChunkAgg, u64)> =
      self.chunks_with_size().into_iter().filter(|(_, chunk, _)| chunk.is_entry).collect();
    writeln!(
      out,
      "{}, {}.\n",
      plural(entries.len(), "entry point"),
      plural(self.chunk_count(), "non-empty chunk"),
    )
    .unwrap();
    if entries.is_empty() {
      out.push_str("_No entry chunks._\n");
      return out;
    }
    let names = |ids: &[u32]| {
      ids.iter().map(|i| format!("`{}`", self.chunk_label(*i))).collect::<Vec<_>>().join(", ")
    };
    for (id, chunk, size) in entries.iter().take(self.config.top_n) {
      let entry_module =
        chunk.entry_module.as_ref().map_or_else(|| self.chunk_label(*id), |m| self.stabilize(m));
      writeln!(out, "### Entry: `{entry_module}`\n").unwrap();
      writeln!(out, "- **Output chunk**: `{}` ({})", self.chunk_label(*id), format_size(*size))
        .unwrap();
      writeln!(
        out,
        "- **Initial load**: {} (this chunk + its transitive static-import chunks)",
        format_size(self.initial_load_bytes(*id)),
      )
      .unwrap();
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

fn plural(n: usize, word: &str) -> String {
  if n == 1 {
    format!("1 {word}")
  } else {
    format!("{n} {word}s")
  }
}
