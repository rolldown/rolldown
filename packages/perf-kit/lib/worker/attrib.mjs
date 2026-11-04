// Startup CPU by EVALUATING module, from a profile taken on an attribution build (the
// same environment built with `preserveModules`, every module its own file).
//
// Two things make this build necessary. V8 attaches line ticks to a frame only when
// source positions exist, which in practice means optimized code; the top-level frame
// of a bundled chunk usually has none, so its self time cannot be split by module. And
// ESM evaluation is not a call stack: a library's top-level work has no importer frame
// above it. With one file per module, the root-most frame under the output dir IS the
// module whose evaluation is running, and the build report's importer chain says where
// the import to cut lives.

import path from 'node:path';

import { localFile } from './measure.mjs';

const isLab = (rel) => !rel || rel.startsWith('__lab') || rel.startsWith('_virtual/');

/**
 * `graph` (optional) is the loaded module graph of the attribution build; with it every
 * row carries the chain from the entry. Returns rows sorted by time: `{ module, ms, chain }`,
 * `pairs` (evaluating module -> library file the time is spent in) and `self` by file.
 */
export function evaluatingModules(profile, { outDir, graph = null, top = 20 }) {
  const nodes = new Map(profile.nodes.map((n) => [n.id, n]));
  const parent = new Map();
  for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const rel = (url) => {
    const file = localFile(url, outDir);
    return file ? path.relative(outDir, file).replaceAll('\\', '/') : null;
  };
  const evaluating = new Map();
  const pairs = new Map();
  const self = new Map();
  const add = (m, k, v) => m.set(k, (m.get(k) ?? 0) + v);
  profile.samples.forEach((id, i) => {
    const delta = profile.timeDeltas[i] ?? 0;
    if (delta <= 0) return;
    const leaf = nodes.get(id);
    const fn = leaf?.callFrame.functionName;
    if (fn === '(idle)' || fn === '(program)') return;
    const leafFile = rel(leaf?.callFrame.url);
    add(
      self,
      leafFile ?? (fn === '(garbage collector)' ? '(gc)' : leaf?.callFrame.url || '(engine)'),
      delta,
    );
    let cur = id;
    let root = null;
    while (cur != null) {
      const f = rel(nodes.get(cur)?.callFrame.url);
      if (f && !isLab(f)) root = f;
      cur = parent.get(cur);
    }
    add(evaluating, root ?? (fn === '(garbage collector)' ? '(gc)' : '(no module frame)'), delta);
    if (root && leafFile && leafFile !== root && !isLab(leafFile)) {
      add(pairs, `${root} -> ${leafFile.replace(/^node_modules\//, '')}`, delta);
    }
  });
  const ms = (v) => Math.round(v / 100) / 10;
  const sorted = (m) => [...m.entries()].sort((a, b) => b[1] - a[1]);
  const chainOf = graph ? graphChainResolver(graph) : () => null;
  return {
    rows: sorted(evaluating)
      .slice(0, top)
      .map(([module, us]) => ({ module, ms: ms(us), chain: chainOf(module) })),
    pairs: sorted(pairs)
      .slice(0, top)
      .map(([pair, us]) => ({ pair, ms: ms(us) })),
    self: sorted(self)
      .slice(0, top)
      .map(([file, us]) => ({ file, ms: ms(us) })),
  };
}

/**
 * The calls that module top levels make: the frame right below the root-most frame of a
 * local file. A library that spends its import in one call (a warm-up such as
 * `preloadSchemas()`) shows here by name; the per-file views charge that time to the files
 * the call runs in. `fileOf(url)` maps a frame URL to an output file (relative) or null;
 * `locate(file, line, column)` optionally names the source module of a bundled position.
 * Returns rows `{ module, fn, at, source, ms }` sorted by time; `at` is `file:line` (1-based).
 */
export function topLevelCalls(profile, { fileOf, locate = null }) {
  const nodes = new Map(profile.nodes.map((n) => [n.id, n]));
  const parent = new Map();
  for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const calls = new Map();
  profile.samples.forEach((id, i) => {
    const delta = profile.timeDeltas[i] ?? 0;
    if (delta <= 0) return;
    let cur = id;
    let below = null;
    let root = null;
    let call = null;
    while (cur != null) {
      const f = fileOf(nodes.get(cur)?.callFrame.url);
      if (f && !isLab(f)) {
        root = f;
        call = below;
      }
      below = cur;
      cur = parent.get(cur);
    }
    if (!root || call == null) return;
    const frame = nodes.get(call).callFrame;
    const key = `${root}\0${frame.functionName}\0${frame.url}\0${frame.lineNumber}\0${frame.columnNumber}`;
    const row = calls.get(key) ?? { root, frame, us: 0 };
    row.us += delta;
    calls.set(key, row);
  });
  return [...calls.values()]
    .sort((a, b) => b.us - a.us)
    .map(({ root, frame, us }) => {
      const file = fileOf(frame.url);
      const line = frame.lineNumber >= 0 ? frame.lineNumber + 1 : null;
      return {
        module: root,
        fn: frame.functionName || '(anonymous)',
        at: `${file ?? frame.url ?? '?'}${line ? `:${line}` : ''}`,
        source: file && locate ? locate(file, frame.lineNumber, frame.columnNumber) : null,
        ms: Math.round(us / 100) / 10,
      };
    });
}

/** A module id without the package manager's store prefix: `zod/v4/core/core.js`. */
export function shortModule(id) {
  return String(id).replace(/^(?:\.\.\/)*(?:.*\/)?node_modules\/\.pnpm\/[^/]+\/node_modules\//, '');
}

/** Match an output file of the attribution build to a graph id and walk up to the entry. */
export function graphChainResolver(graph) {
  const norm = (id) =>
    String(id)
      .replace(/^(\.\.\/)+/, '')
      .replace(/^node_modules\//, '')
      .replace(/\.[cm]?[jt]sx?$/, '');
  const byKey = new Map(graph.modules.map((m, i) => [norm(m.id), i]));
  return (file) => {
    const i = byKey.get(norm(file));
    if (i == null) return null;
    const chain = [];
    let cur = i;
    const seen = new Set();
    while (cur != null && !seen.has(cur)) {
      seen.add(cur);
      const m = graph.modules[cur];
      const up = m.idom != null && m.idom >= 0 ? m.idom : (graph.staticPreds[cur] ?? [])[0];
      if (up == null || up === cur) break;
      chain.push(up);
      cur = up;
    }
    return chain
      .reverse()
      .map((j) => graph.modules[j].id)
      .filter((id) => !/^(\\0|\0)/.test(id));
  };
}

/** Direct static importers of a module and one chain from the entry: the "why" view. */
export function whyModule(graph, index) {
  const name = (i) => graph.modules[i].id;
  const preds = graph.staticPreds[index] ?? [];
  const chain = [];
  let cur = index;
  const seen = new Set();
  while (cur != null && !seen.has(cur)) {
    seen.add(cur);
    const m = graph.modules[cur];
    const up = m.idom != null && m.idom >= 0 ? m.idom : (graph.staticPreds[cur] ?? [])[0];
    if (up == null || up === cur) break;
    chain.push(up);
    cur = up;
  }
  return {
    module: name(index),
    bytes: graph.modules[index].bytes ?? 0,
    importers: preds.map((p) => ({ module: name(p), bytes: graph.modules[p].bytes ?? 0 })),
    chain: chain.reverse().map(name),
  };
}
