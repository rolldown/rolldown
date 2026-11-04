// Cloudflare Worker startup driver: run a Vite-built Worker under the app's OWN workerd
// (through its miniflare and wrangler), with the Worker's startup moved inside a request
// so it can be timed, profiled and covered.
//
// workerd evaluates a Worker's main module when it loads the worker, before any request
// or inspector can observe it. So the worker perf-kit runs is a generated wrapper entry
// whose fetch handler does `await import('./<main>')`: the import compiles and evaluates
// the main module and its whole static closure, which is exactly the work Cloudflare's
// startup limit counts. `wrangler check startup` uses the same trick.
//
// Two instance shapes, deliberately separate:
//   wrapper  - startup happens inside the lab route. Durable Object and Workflow classes
//              live in the main module, not in the wrapper, so their namespaces are
//              dropped here and DO-backed routes cannot be served through it.
//   plain    - the app's real entry is the worker: startup happens at boot, every export
//              is in place. The shape for first-request numbers and response parity.

import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';

export const WRAPPER_NAME = '__lab_wrapper.js';
export const EMPTY_NAME = '__lab_empty.js';
export const LAB_IMPORT_PATH = '/__lab/import';
export const LAB_COVERAGE_PATH = '/__lab/coverage';
// `enable_nodejs_inspector_module` makes `node:inspector` importable on compatibility dates
// before its default-on date; workerd knows it from 1.20260507.1, before the session shipped.
export const COVERAGE_FLAGS = [
  'nodejs_compat',
  'enable_nodejs_inspector_local_dev',
  'enable_nodejs_inspector_module',
];
/** First Wrangler whose workerd (1.20260706.1) carries the in-process inspector session. */
export const COVERAGE_MIN_WRANGLER = '4.108.0';

/**
 * Resolve miniflare, wrangler and the workerd version from the app directory. perf-kit
 * never ships a runtime of its own: the number must come from the workerd the app pins.
 */
export function resolveWorkerTools(appDir) {
  const require = createRequire(path.join(appDir, 'package.json'));
  const resolveOrNull = (id) => {
    try {
      return require.resolve(id);
    } catch {
      return null;
    }
  };
  const miniflarePath = resolveOrNull('miniflare');
  const wranglerPath = resolveOrNull('wrangler');
  if (!miniflarePath || !wranglerPath) {
    throw new Error(
      `${appDir} has no ${!miniflarePath ? 'miniflare' : 'wrangler'} - add wrangler as a dev dependency ` +
        "(it brings miniflare and workerd). perf-kit runs the app's own workerd, never one of its own.",
    );
  }
  const version = (id) => {
    try {
      return require(`${id}/package.json`).version;
    } catch {
      return null;
    }
  };
  return {
    appDir,
    miniflarePath,
    wranglerPath,
    versions: {
      wrangler: version('wrangler'),
      miniflare: version('miniflare'),
      workerd: version('workerd'),
    },
    miniflare: null,
    wrangler: null,
    async load() {
      this.miniflare ??= await import(this.miniflarePath);
      this.wrangler ??= await import(this.wranglerPath);
      return this;
    },
    /** Miniflare 5 takes `workers: [...]`; v4 takes the flat shape wrangler's helper returns. */
    toOptions(flat) {
      const convert = this.miniflare?.convertV4MiniflareOptions;
      return typeof convert === 'function' ? convert(flat) : flat;
    },
  };
}

/** `a.b.c` >= `x.y.z` */
export function versionAtLeast(version, minimum) {
  if (!version) return false;
  const a = String(version).split(/[.-]/).map(Number);
  const b = String(minimum).split(/[.-]/).map(Number);
  for (let i = 0; i < 3; i++) {
    const x = a[i] || 0;
    const y = b[i] || 0;
    if (x !== y) return x > y;
  }
  return true;
}

/**
 * The output `wrangler.json` the Cloudflare Vite plugin writes next to the worker's
 * `index.js`, read through wrangler's own helper so bindings, flags and the compatibility
 * date mean what `wrangler dev` makes them mean. When the helper cannot express a binding,
 * startup is still measurable: module evaluation rarely touches `env`.
 */
export function loadWorkerOptions(tools, configPath) {
  const helper = tools.wrangler?.unstable_getMiniflareWorkerOptions;
  if (typeof helper !== 'function') {
    throw new Error(
      'this wrangler does not export unstable_getMiniflareWorkerOptions - update wrangler',
    );
  }
  try {
    const { workerOptions, main } = helper(configPath, undefined, {
      overrides: { enableContainers: false },
    });
    return { workerOptions, main, fallback: null };
  } catch (error) {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    return {
      workerOptions: {
        compatibilityDate: config.compatibility_date,
        compatibilityFlags: config.compatibility_flags ?? [],
        modulesRules: (config.rules ?? []).map((r) => ({ type: r.type, include: r.globs })),
      },
      main: config.main,
      fallback: String(error.message).split('\n')[0],
    };
  }
}

export const DEFAULT_RULES = [
  { type: 'ESModule', include: ['**/*.js', '**/*.mjs'] },
  { type: 'CompiledWasm', include: ['**/*.wasm'] },
  { type: 'Text', include: ['**/*.txt', '**/*.html', '**/*.sql'] },
  { type: 'Data', include: ['**/*.bin'] },
];

/** A wrangler module-rule glob (`**` and `*`, `?`) to a RegExp over a `/`-joined path. */
export function globToRegExp(glob) {
  let re = '';
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i];
    if (c === '*') {
      if (glob[i + 1] === '*') {
        i++;
        if (glob[i + 1] === '/') {
          i++;
          re += '(?:.*/)?';
        } else re += '.*';
      } else re += '[^/]*';
    } else if (c === '?') re += '[^/]';
    else re += /[.+^${}()|[\]\\]/.test(c) ? `\\${c}` : c;
  }
  return new RegExp(`^${re}$`);
}

/**
 * Every file under the output dir that a module rule classifies, as miniflare module
 * descriptors. The app's own rules come first, the defaults close the gaps (`.wasm` is a
 * CompiledWasm module even when the app lists only ESModule rules, as wrangler does).
 */
export function classifyModules(outDir, rules = []) {
  const all = [...rules, ...DEFAULT_RULES].map((rule) => ({
    type: rule.type,
    patterns: (rule.include ?? rule.globs ?? []).map(globToRegExp),
  }));
  const typeOf = (rel) => all.find((rule) => rule.patterns.some((p) => p.test(rel)))?.type ?? null;
  const modules = [];
  const skipped = [];
  const walk = (dir, rel) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      // Dot directories hold modules too: a preserveModules build of a pnpm app writes its
      // libraries under `node_modules/.pnpm/`. Only `.vite/` (the manifest) and dot files stay out.
      if (entry.name.startsWith('.') && (!entry.isDirectory() || entry.name === '.vite')) continue;
      const r = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        walk(path.join(dir, entry.name), r);
        continue;
      }
      if (r.endsWith('.map') || r === 'wrangler.json' || r.startsWith('wrangler.')) continue;
      const type = typeOf(r);
      if (type) modules.push({ type, path: path.join(outDir, r), rel: r });
      else skipped.push(r);
    }
  };
  walk(outDir, '');
  return { modules, skipped };
}

/** The wrapper entry. `coverage: true` is the variant that collects precise coverage. */
export function wrapperModule(outDir, target, { coverage = false } = {}) {
  const spec = JSON.stringify(`./${target}`);
  const contents = coverage
    ? `import { Session } from 'node:inspector/promises';
export default {
  async fetch() {
    let s;
    try { s = new Session(); s.connect(); } catch (e) { return Response.json({ unsupported: String((e && e.code) || e) }); }
    await s.post('Profiler.enable');
    await s.post('Profiler.startPreciseCoverage', { callCount: true, detailed: true });
    let error = null;
    try { await import(${spec}); } catch (e) { error = String((e && e.stack) || e); }
    const { result } = await s.post('Profiler.takePreciseCoverage');
    await s.post('Profiler.stopPreciseCoverage');
    s.disconnect();
    return Response.json({ error, result });
  }
};
`
    : `let mod = null;
export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (url.pathname === ${JSON.stringify(LAB_IMPORT_PATH)}) {
      let error = null;
      try { mod = await import(${spec}); } catch (e) { error = String((e && e.stack) || e); }
      return new Response(error ? 'startup threw: ' + error : 'ok', { status: error ? 500 : 200 });
    }
    mod ??= await import(${spec});
    const handler = mod.default && mod.default.fetch ? mod.default : null;
    if (!handler) return new Response('no default export with fetch', { status: 501 });
    return handler.fetch(request, env, ctx);
  }
};
`;
  return { type: 'ESModule', path: path.join(outDir, WRAPPER_NAME), contents };
}

export const emptyModule = (outDir) => ({
  type: 'ESModule',
  path: path.join(outDir, EMPTY_NAME),
  contents: 'export {};\n',
});

/**
 * Static assets answer before the worker unless a route is listed as worker-first; with
 * SPA fallback every unknown path becomes `index.html`, lab routes included - and that
 * answer is a 200, so the import silently never runs. Lab routes go worker-first. Newer
 * Miniflare builds the router from a `run_worker_first` list, so the route goes there too.
 */
export function labAssetsRouting(assets) {
  if (!assets) return assets;
  const router = { ...assets.routerConfig, has_user_worker: true };
  const routing = { ...router.static_routing };
  routing.user_worker = ['/__lab/*', ...(routing.user_worker ?? [])];
  router.static_routing = routing;
  const routed = { ...assets, routerConfig: router };
  if (Array.isArray(assets.run_worker_first)) {
    routed.run_worker_first = ['/__lab/*', ...assets.run_worker_first];
  }
  return routed;
}

/** Deduplicated workerd stderr, printed once at the end instead of once per run. */
export function createStderrCollector() {
  const lines = new Map();
  return {
    lines,
    handleRuntimeStdio(stdout, stderr) {
      stdout.on('data', () => {});
      stderr.on('data', (chunk) => {
        for (const line of String(chunk).split('\n')) {
          const key = line.replace(/\s+/g, ' ').trim().slice(0, 220);
          if (key) lines.set(key, (lines.get(key) ?? 0) + 1);
        }
      });
    },
  };
}

/**
 * One miniflare instance. `ctx` is the resolved worker context (tools, workerOptions,
 * outDir, mainName, modules). Options: `target` (module the wrapper imports), `coverage`
 * (coverage wrapper), `extraFlags` (compatibility flags added for this instance only),
 * `shared` (top-level miniflare options such as `inspectorPort`), `plain` (real entry).
 */
export async function createInstance(
  ctx,
  { target, coverage = false, extraFlags = [], shared = {}, plain = false, tools = null } = {},
) {
  const mod = tools ?? ctx.tools;
  await mod.load();
  const { Log, LogLevel } = mod.miniflare;
  const appFlags = ctx.workerOptions.compatibilityFlags ?? [];
  const flat = {
    ...ctx.workerOptions,
    name: 'lab',
    compatibilityFlags: [...new Set([...appFlags, ...extraFlags])],
    modulesRoot: ctx.outDir,
    handleRuntimeStdio: ctx.stderr.handleRuntimeStdio,
    log: new Log(LogLevel.WARN),
    ...shared,
  };
  if (plain) {
    const mainModule = ctx.modules.find((m) => m.rel === ctx.mainName);
    if (!mainModule) throw new Error(`main module ${ctx.mainName} is not among the output modules`);
    flat.modules = [mainModule, ...ctx.modules.filter((m) => m !== mainModule)];
  } else {
    flat.modules = [
      wrapperModule(ctx.outDir, target ?? ctx.mainName, { coverage }),
      emptyModule(ctx.outDir),
      ...ctx.modules,
    ];
    delete flat.durableObjects;
    delete flat.workflows;
    if (flat.assets) flat.assets = labAssetsRouting(flat.assets);
  }
  const mf = new mod.miniflare.Miniflare(mod.toOptions(flat));
  await mf.ready;
  return mf;
}

export async function fetchText(mf, p, { timeoutMs = 120_000 } = {}) {
  const res = await mf.dispatchFetch(`http://lab${p}`, { signal: AbortSignal.timeout(timeoutMs) });
  return { status: res.status, text: await res.text() };
}

export async function disposeQuietly(mf) {
  try {
    await mf.dispose();
  } catch {
    /* already gone */
  }
}

/**
 * One cold run: a fresh instance, then the time of the lab import request as seen from
 * the host. Timers inside workerd advance only on I/O, so the Worker cannot time itself.
 * `bootMs` (instance creation to ready) is reported but never counted: it is workerd
 * process start, config load and wasm compile, not the Worker's global scope.
 */
export async function timedImport(ctx, { target } = {}) {
  const b0 = performance.now();
  const mf = await createInstance(ctx, { target });
  const bootMs = performance.now() - b0;
  try {
    const t0 = performance.now();
    const { status, text } = await fetchText(mf, LAB_IMPORT_PATH);
    const ms = performance.now() - t0;
    if (status !== 200 || text !== 'ok') {
      throw new Error(`the lab import route did not run (${status}): ${text.slice(0, 600)}`);
    }
    return { ms, bootMs };
  } finally {
    await disposeQuietly(mf);
  }
}
