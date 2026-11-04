// Join emitted static chunk imports to the runtime waterfall. A discovery delay
// is a lead to test, not a predicted LCP gain or proof that paint awaits a chunk.
// The waterfall is a lab scan's Resource Timing list, or the CDP requests of field
// and replay runs (requestSamples).

import fs from 'node:fs';
import path from 'node:path';
import { median } from './measure.mjs';

/** Prefer a current emitted graph; Vite's manifest supplies the same static edges. */
export function loadChunkGraph(dist, candidates = []) {
  const manifestFile = path.join(dist, '.vite', 'manifest.json');
  const manifestMtime = fs.existsSync(manifestFile) ? fs.statSync(manifestFile).mtimeMs : 0;
  for (const file of candidates) {
    if (!fs.existsSync(file)) continue;
    const graphMtime = fs.statSync(file).mtimeMs;
    if (graphMtime < manifestMtime) continue;
    const data = JSON.parse(fs.readFileSync(file, 'utf8'));
    if (data.schemaVersion !== 1 || !Array.isArray(data.chunks) || !data.chunks.length) continue;
    const files = new Map();
    for (const chunk of data.chunks) {
      // Metrics filenames may include the build's relative output directory.
      // Match a path suffix only when it names a real file inside this dist.
      const parts = chunk.fileName.replaceAll('\\', '/').split('/');
      for (let i = 0; i < parts.length; i++) {
        const name = parts.slice(i).join('/');
        const resolved = path.resolve(dist, name);
        if (!resolved.startsWith(`${path.resolve(dist)}${path.sep}`)) continue;
        const stat = fs.existsSync(resolved) ? fs.statSync(resolved) : null;
        if (stat?.isFile() && stat.mtimeMs <= graphMtime) {
          files.set(chunk.fileName, name);
          break;
        }
      }
    }
    // Rebuilds can reuse filenames. The graph must also follow the emitted files
    // and the manifest so old imports cannot produce a false all-clear.
    if (files.size !== data.chunks.length) continue;
    return {
      file,
      chunks: new Map(
        data.chunks.map((chunk) => [
          files.get(chunk.fileName),
          {
            facadeModuleId: chunk.facadeModuleId ?? null,
            imports: (chunk.imports ?? []).map((name) => files.get(name)).filter(Boolean),
          },
        ]),
      ),
    };
  }
  const file = manifestFile;
  if (!fs.existsSync(file)) return null;
  const manifest = JSON.parse(fs.readFileSync(file, 'utf8'));
  const rows = Object.values(manifest).filter((chunk) => /\.[cm]?js$/.test(chunk.file ?? ''));
  if (
    !rows.length ||
    rows.some((chunk) => {
      const file = path.join(dist, chunk.file);
      const stat = fs.existsSync(file) ? fs.statSync(file) : null;
      return !stat?.isFile() || stat.mtimeMs > manifestMtime;
    })
  )
    return null;
  return {
    file,
    chunks: new Map(
      rows.map((chunk) => [
        chunk.file,
        {
          facadeModuleId: chunk.src ?? null,
          imports: (chunk.imports ?? []).map((key) => manifest[key]?.file).filter(Boolean),
        },
      ]),
    ),
  };
}

/** Keep one shortest static import chain per descendant, excluding cycles. */
function staticDescendants(graph, root) {
  const paths = new Map([[root, [root]]]);
  const queue = [root];
  for (let i = 0; i < queue.length; i++) {
    const parent = queue[i];
    for (const file of graph.chunks.get(parent)?.imports ?? []) {
      if (paths.has(file)) continue;
      paths.set(file, [...paths.get(parent), file]);
      queue.push(file);
    }
  }
  paths.delete(root);
  return paths;
}

/**
 * Field and replay runs as discovery samples. Their CDP request rows (page time,
 * every request of the page) stand in for the Resource Timing list, which field
 * runs do not keep. `redact` cleans initiator URLs: a document URL can name an account.
 */
export function requestSamples(runs, origin, redact = (value) => value) {
  const local = (url) => (url?.startsWith(`${origin}/`) ? url.slice(origin.length) : url);
  return runs.map((run) => ({
    fcp: run.fcp ?? null,
    lcp: run.lcp ?? null,
    resourceDetailsComplete: true,
    resources: (run.requests ?? []).map((r) => ({
      name: local(r.url),
      type: String(r.type ?? 'other').toLowerCase(),
      start: r.startMs ?? 0,
      // end 0 = unfinished or failed, as in the Resource Timing list.
      end: r.failed || typeof r.endMs !== 'number' ? 0 : r.endMs,
      bytes: r.encodedBytes ?? 0,
      initiator: { type: r.initiator?.type ?? null, url: redact(local(r.initiator?.url ?? null)) },
      requestPriority: { initial: r.priority ?? null, last: r.lastPriority ?? r.priority ?? null },
    })),
  }));
}

export function analyzeChunkDiscovery(graph, samples, latencyMs) {
  return Object.fromEntries(
    ['fcp', 'lcp'].map((paint) => [paint, analyzePaintDiscovery(graph, samples, latencyMs, paint)]),
  );
}

function analyzePaintDiscovery(graph, samples, latencyMs, paint) {
  const complete = samples.length > 0 && samples.every((s) => s.resourceDetailsComplete === true);
  if (!graph) return { state: 'unknown', reason: 'no chunk graph or Vite manifest', groups: [] };
  if (typeof latencyMs !== 'number' || latencyMs <= 0) {
    return {
      state: 'unknown',
      reason: 'no throttled RTT reference',
      graphFile: graph.file,
      groups: [],
    };
  }
  const groups = new Map();
  const matchedCounts = [];
  let paintedRuns = 0;
  for (const sample of samples) {
    if (typeof sample[paint] !== 'number') continue;
    paintedRuns++;
    const requests = new Map();
    for (const resource of sample.resources ?? []) {
      // Same-origin names are relative. Cross-origin scripts can have identical
      // basenames but do not belong to this build's graph.
      if (resource.type !== 'script' || !resource.name.startsWith('/') || resource.end <= 0)
        continue;
      let file;
      try {
        file = decodeURIComponent(resource.name.split(/[?#]/)[0].slice(1));
      } catch {
        continue;
      }
      if (!graph.chunks.has(file)) continue;
      const previous = requests.get(file);
      if (!previous || resource.start < previous.start) requests.set(file, resource);
    }
    matchedCounts.push(requests.size);
    for (const [parent, request] of requests) {
      if (request.end > sample[paint]) continue;
      const late = [];
      for (const [file, importChain] of staticDescendants(graph, parent)) {
        const child = requests.get(file);
        if (!child || child.end > sample[paint] || child.start - request.end <= latencyMs) continue;
        late.push({
          file,
          facadeModuleId: graph.chunks.get(file)?.facadeModuleId ?? null,
          importChain,
          startMs: child.start,
          endMs: child.end,
          delayMs: child.start - request.end,
          bytes: child.bytes,
          initiator: child.initiator ?? null,
          requestPriority: child.requestPriority ?? null,
        });
      }
      if (!late.length) continue;
      late.sort((a, b) => b.bytes - a.bytes || a.file.localeCompare(b.file));
      if (!groups.has(parent)) groups.set(parent, []);
      groups.get(parent).push({
        paintMs: sample[paint],
        parentEndMs: request.end,
        bytes: late.reduce((sum, r) => sum + r.bytes, 0),
        count: late.length,
        maxDelayMs: Math.max(...late.map((r) => r.delayMs)),
        chunks: late,
      });
    }
  }
  const rows = [...groups]
    .map(([parent, runs]) => {
      const sorted = runs.slice().sort((a, b) => a.bytes - b.bytes);
      return {
        parent,
        facadeModuleId: graph.chunks.get(parent)?.facadeModuleId ?? null,
        paintDependency: 'unverified',
        runsAffected: runs.length,
        measuredRuns: paintedRuns,
        bytes: median(runs.map((r) => r.bytes)),
        count: median(runs.map((r) => r.count)),
        maxDelayMs: median(runs.map((r) => r.maxDelayMs)),
        // Keep an actual run's edges and timings; do not invent a median waterfall.
        example: sorted[Math.floor(sorted.length / 2)],
      };
    })
    .sort(
      (a, b) =>
        b.bytes - a.bytes || b.maxDelayMs - a.maxDelayMs || a.parent.localeCompare(b.parent),
    );
  const matchedScripts = median(matchedCounts) ?? 0;
  const reason = !complete
    ? 'resource details missing or browser timing buffer overflow'
    : paintedRuns !== samples.length
      ? `${paint.toUpperCase()} missing in one or more runs`
      : matchedCounts.some((count) => count === 0)
        ? 'no runtime scripts matched the chunk graph in one or more runs'
        : null;
  return {
    state: reason ? 'unknown' : rows.length ? 'open' : 'clear',
    reason,
    graphFile: graph.file,
    latencyMs,
    matchedScripts,
    groups: rows,
  };
}
