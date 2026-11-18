// Measured startup CPU on the module graph: the profile's self time per module, summed over
// what one deferral removes. A row list per file stops an agent once every row is under the
// noise floor; the same time grouped by import shows which deferrals add up.
// See AGENTS.md, "If the target is a Cloudflare Worker".

import fs from 'node:fs';
import path from 'node:path';

import { deferAllInto, evalOverrides, resolveModule } from '../module-graph.mjs';
import { graphIdCandidates } from '../node/measure.mjs';

const round1 = (v) => Math.round(v * 10) / 10;

/** A plugin's virtual module: the report writes its NUL prefix as the text `\\0`. */
export const isVirtual = (id) => id.startsWith('\0') || id.startsWith('\\0');

/**
 * Self time per graph module index. `bySource` maps a sourcemap source (relative to the
 * chunk, as the profile rows name it) to ms. Sources no module matches add to `unmatchedMs`.
 */
export function cpuByModule(graph, bySource) {
  const byIdx = new Map();
  let unmatchedMs = 0;
  for (const [source, ms] of Object.entries(bySource ?? {})) {
    let idx = null;
    for (const form of graphIdCandidates(source)) {
      const hit = resolveModule(graph, form);
      if (hit && !hit.ambiguous) {
        idx = hit.index;
        break;
      }
    }
    if (idx == null) unmatchedMs += ms;
    else byIdx.set(idx, (byIdx.get(idx) ?? 0) + ms);
  }
  return { byIdx, unmatchedMs: round1(unmatchedMs) };
}

const noExt = (id) => id.replace(/\.(?:[cm]?[jt]sx?|json)$/, '');

/**
 * Evaluating time per graph module index, from the attribution build (`preserveModules`,
 * `[name].js`): `byFile` maps an output file to ms. Same shape as `cpuByModule`.
 */
export function cpuByOutputFile(graph, byFile) {
  const byStem = new Map();
  graph.modules.forEach((m, i) => {
    const key = noExt(m.id);
    byStem.set(key, byStem.has(key) ? -1 : i);
  });
  const byIdx = new Map();
  let unmatchedMs = 0;
  for (const [file, ms] of Object.entries(byFile ?? {})) {
    let idx = null;
    for (const form of graphIdCandidates(file)) {
      const stem = noExt(form);
      const hit = byStem.get(stem) ?? byStem.get(noExt(stem));
      if (hit != null && hit >= 0) {
        idx = hit;
        break;
      }
    }
    if (idx == null) unmatchedMs += ms;
    else byIdx.set(idx, (byIdx.get(idx) ?? 0) + ms);
  }
  return { byIdx, unmatchedMs: round1(unmatchedMs) };
}

/**
 * Self time that library code holds: `{ ms, totalMs, top: [[package, ms]] }`. The profile
 * charges a library call to the library, not to the module whose top level makes it.
 */
export function libraryShare(graph, cpu, isApp) {
  const byPkg = new Map();
  let ms = 0;
  let totalMs = 0;
  for (const [i, v] of cpu.byIdx) {
    totalMs += v;
    const id = graph.modules[i].id;
    if (isApp(id)) continue;
    ms += v;
    const pkg = id.startsWith('@') ? id.split('/').slice(0, 2).join('/') : id.split('/')[0];
    byPkg.set(pkg, (byPkg.get(pkg) ?? 0) + v);
  }
  const top = [...byPkg.entries()].sort((a, b) => b[1] - a[1]).map(([p, v]) => [p, round1(v)]);
  return { ms: round1(ms), totalMs: round1(totalMs), top };
}

/** The profile rows that a sourcemap mapped to a source module, as `{ source: ms }`. */
export function sourceCpu(rows) {
  const out = {};
  for (const row of rows ?? []) {
    if (row.kind === 'source') out[row.bucket] = round1((out[row.bucket] ?? 0) + row.ms);
  }
  return out;
}

/** Measured ms and module count of a set of graph modules (objects or indices). */
export function cpuOf(graph, cpu, modules) {
  const idOf = new Map(graph.modules.map((m, i) => [m, i]));
  let ms = 0;
  for (const m of modules) ms += cpu.byIdx.get(typeof m === 'number' ? m : idOf.get(m)) ?? 0;
  return round1(ms);
}

/**
 * Dominator-subtree CPU: `ms[i]` is the self time of every module that leaves startup when
 * all static imports of `i` become dynamic (the build's `idom` tree is over static edges).
 * `count[i]` and `bytes[i]` are the module count and the bytes of that subtree.
 */
export function retainedCpu(graph, cpu) {
  const n = graph.modules.length;
  const children = Array.from({ length: n }, () => []);
  const roots = [];
  graph.modules.forEach((m, i) => {
    if (!m.staticReachable) return;
    if (m.idom != null && m.idom >= 0 && m.idom !== i) children[m.idom].push(i);
    else roots.push(i);
  });
  const ms = new Float64Array(n);
  const count = new Int32Array(n);
  const bytes = new Float64Array(n);
  // Iterative post-order: a deep import chain must not overflow the stack.
  for (const root of roots) {
    const stack = [[root, 0]];
    while (stack.length) {
      const top = stack[stack.length - 1];
      const [v, k] = top;
      if (k < children[v].length) {
        top[1]++;
        stack.push([children[v][k], 0]);
        continue;
      }
      stack.pop();
      ms[v] += cpu.byIdx.get(v) ?? 0;
      count[v] += 1;
      bytes[v] += graph.modules[v].bytes ?? 0;
      const up = graph.modules[v].idom;
      if (up != null && up >= 0 && up !== v && graph.modules[up].staticReachable) {
        ms[up] += ms[v];
        count[up] += count[v];
        bytes[up] += bytes[v];
      }
    }
  }
  return { children, ms, count, bytes, roots };
}

/**
 * Modules that a startup chunk holds although only dynamic imports reach them in the graph:
 * the chunk config (`manualChunks`, `advancedChunks` groups) put them with startup code, so
 * a dynamic import saves nothing for them. `covModules` are the scan's coverage rows
 * (`{ source, script, totalBytes }`, one per module of each loaded chunk).
 */
export function lazyInStartupChunks(graph, covModules) {
  const byScript = new Map();
  const byPackage = new Map();
  let bytes = 0;
  let count = 0;
  for (const m of covModules ?? []) {
    let hit = null;
    for (const form of graphIdCandidates(m.source)) {
      hit = resolveModule(graph, form);
      if (hit && !hit.ambiguous) break;
      hit = null;
    }
    if (!hit || graph.modules[hit.index].staticReachable) continue;
    const id = graph.modules[hit.index].id;
    if (isVirtual(id)) continue;
    const at = m.source.lastIndexOf('node_modules/');
    const rest = at >= 0 ? m.source.slice(at + 'node_modules/'.length) : null;
    const pkg = rest
      ? rest
          .split('/')
          .slice(0, rest.startsWith('@') ? 2 : 1)
          .join('/')
      : '(app)';
    bytes += m.totalBytes;
    count += 1;
    byScript.set(m.script, (byScript.get(m.script) ?? 0) + m.totalBytes);
    byPackage.set(pkg, (byPackage.get(pkg) ?? 0) + m.totalBytes);
  }
  const sorted = (map) => [...map.entries()].sort((a, b) => b[1] - a[1]);
  return { bytes, count, byScript: sorted(byScript), byPackage: sorted(byPackage) };
}

/**
 * Startup CPU grouped by import. From the entry, go down through pass-through modules only:
 * a virtual module (the plugin's entry) or a module with one dominator child and no time
 * of its own, and only into app modules (`isApp`). A module that holds most of the time
 * for itself stays a row: it is the finding. The rows are the dominator children of that
 * anchor, each with its largest children.
 */
export function startupGroups(graph, cpu, opts = {}) {
  const { limit = 8, childLimit = 3, minMs = 0.5, isApp = () => true } = opts;
  const tree = retainedCpu(graph, cpu);
  const byMs = (a, b) => tree.ms[b] - tree.ms[a];
  const top = tree.roots.slice().sort(byMs)[0];
  if (top == null) return null;
  const chain = [top];
  let anchor = top;
  for (;;) {
    const kids = tree.children[anchor].slice().sort(byMs);
    const passThrough =
      isVirtual(graph.modules[anchor].id) ||
      (kids.length === 1 && (cpu.byIdx.get(anchor) ?? 0) < minMs);
    if (!kids.length || !passThrough || !isApp(graph.modules[kids[0]].id)) break;
    anchor = kids[0];
    chain.push(anchor);
  }
  const row = (i) => ({
    index: i,
    id: graph.modules[i].id,
    ms: round1(tree.ms[i]),
    modules: tree.count[i],
    importers: graph.staticPreds[i]?.length ?? 0,
  });
  const kids = tree.children[anchor].slice().sort(byMs);
  const rows = kids
    .filter((i) => tree.ms[i] >= minMs)
    .slice(0, limit)
    .map((i) => ({
      ...row(i),
      children: tree.children[i]
        .slice()
        .sort(byMs)
        .filter((c) => tree.ms[c] >= minMs)
        .slice(0, childLimit)
        .map(row),
    }));
  const restMs = kids.slice(rows.length).reduce((sum, i) => sum + tree.ms[i], 0);
  return {
    chain: chain.map((i) => graph.modules[i].id),
    anchor: row(anchor),
    anchorSelfMs: round1(cpu.byIdx.get(anchor) ?? 0),
    rows,
    restMs: round1(restMs),
    restCount: kids.length - rows.length,
  };
}

/**
 * The module that defines each class the runtime loads by name (Durable Objects,
 * Workflows): an app module in the startup closure whose source declares `class <Name>`.
 * Module ids are relative to `rootDir` (the Vite root). Returns `[{ name, index, id }]`.
 */
export function classModules(graph, names, rootDir) {
  if (!names.length) return [];
  const escaped = names.map((n) => n.replace(/[$]/g, '\\$'));
  const pattern = new RegExp(`\\bclass\\s+(${escaped.join('|')})\\b`, 'g');
  const found = new Map();
  graph.modules.forEach((m, i) => {
    if (!m.staticReachable || isVirtual(m.id) || m.id.includes('node_modules/')) return;
    if (!/\.[cm]?[jt]sx?$/.test(m.id)) return;
    let text;
    try {
      text = fs.readFileSync(path.resolve(rootDir, m.id), 'utf8');
    } catch {
      return;
    }
    for (const match of text.matchAll(pattern)) {
      if (!found.has(match[1])) found.set(match[1], { name: match[1], index: i, id: m.id });
    }
  });
  return names.filter((n) => found.has(n)).map((n) => found.get(n));
}

/**
 * The modules that leave startup only when both edge sets move: with one set alone, the
 * other still imports them (a class module and a handler import that reach the same code).
 * Returns graph module objects, largest first.
 */
export function leaveOnlyTogether(graph, a, b) {
  const alone = new Set([...evalOverrides(graph, a).removed, ...evalOverrides(graph, b).removed]);
  return evalOverrides(graph, [...a, ...b]).removed.filter((m) => !alone.has(m));
}

/**
 * What lazy wrapper classes would move: every static import of every class module
 * deferred at once (the wrapper stays in the entry, the implementation loads on the first
 * call). Per class, what only that class holds; `shared`, what leaves only when every class
 * has a wrapper (modules that two or more classes reach).
 */
export function wrapperPrice(graph, cpu, classes) {
  const targets = [...new Set(classes.map((c) => c.index))];
  if (!targets.length) return null;
  const combined = evalOverrides(
    graph,
    targets.flatMap((t) => deferAllInto(graph, t)),
  );
  const tree = retainedCpu(graph, cpu);
  const ms = cpuOf(graph, cpu, combined.removed);
  const own = { count: 0, bytes: 0, ms: 0 };
  const isTarget = new Set(targets);
  const insideOther = (t) => {
    for (let up = graph.modules[t].idom; up != null && up >= 0; up = graph.modules[up].idom) {
      if (isTarget.has(up)) return true;
      if (graph.modules[up].idom === up) break;
    }
    return false;
  };
  for (const t of targets) {
    if (insideOther(t)) continue;
    own.count += tree.count[t];
    own.bytes += tree.bytes[t];
    own.ms += tree.ms[t];
  }
  return {
    ms,
    removed: combined.removed,
    removedCount: combined.removedCount,
    removedBytes: combined.removedBytes,
    perClass: classes
      .map((c) => ({
        ...c,
        ms: round1(tree.ms[c.index]),
        modules: tree.count[c.index],
        bytes: tree.bytes[c.index],
      }))
      .sort((a, b) => b.ms - a.ms || b.bytes - a.bytes),
    shared: {
      count: Math.max(0, combined.removedCount - own.count),
      bytes: Math.max(0, combined.removedBytes - own.bytes),
      ms: round1(Math.max(0, ms - own.ms)),
    },
  };
}
