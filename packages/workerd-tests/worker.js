// The worker half of `packages/workerd-tests/suite.mjs`: runs INSIDE workerd and
// returns JSON reports the driver asserts on. The sibling specifiers below come
// from the driver's explicit Miniflare `modules` list (`@rolldown/browser` dist).
import * as workerdApi from './workerd.browser.mjs';
import wasmModule from './rolldown-binding.wasm32-wasip1.wasm';

const { build, createInstance, getWorkerdRuntimeStats } = workerdApi;

const MiB = 1024 * 1024;

// ---------------------------------------------------------------------------
// Error reporting helpers. Errors cannot cross the workerd/Node boundary, so
// everything the driver asserts on is flattened into plain JSON here.

function errInfo(error) {
  return {
    isError: error instanceof Error,
    message: typeof error?.message === 'string' ? error.message : String(error),
    errorCount: Array.isArray(error?.errors) ? error.errors.length : null,
    errorMessages: Array.isArray(error?.errors)
      ? error.errors.map((item) => String(item?.message ?? item))
      : null,
    hasStack: typeof error?.stack === 'string' && error.stack.length > 0,
    frameCount: Array.isArray(error?.errors)
      ? error.errors.filter((item) => typeof item?.frame === 'string' && item.frame.length > 0)
          .length
      : 0,
  };
}

/**
 * Await `work` and flatten the outcome. Resolved values are NOT carried into the
 * report -- a `RolldownOutput` would blow up the response body -- so pass
 * `describe` to record a small summary instead.
 */
async function settle(work, describe = () => true) {
  try {
    return { ok: true, value: describe(await work) };
  } catch (error) {
    return { ok: false, error: errInfo(error) };
  }
}

const isChunk = (result) => result?.output?.[0]?.type === 'chunk';

// ---------------------------------------------------------------------------
// Behaviour graph: a 26-module fan. 1 entry + 4 features * 5 leaves + 1 shared.
// Every hook is load-bearing by construction:
//   * imports use a `~` prefix that ONLY resolveId can turn into a real id,
//   * module bodies exist only in the plugin's map, so ONLY load can serve them,
//   * leaves carry a `__LEAF_FACTOR__` token that ONLY transform can replace
//     (left in place, it fails the driver's leftover-token and total checks).
const FEATURES = 4;
const LEAVES = 5;
const BASE = 7;

// workerd requires this module's ONLY export to be the default handler, so the
// expected total (390) is asserted on the driver side rather than exported here.
// Its derivation:
//   leaf(i, j) = BASE * (i + 1) + j
//   feat(i)    = sum over j in 0..4 of leaf(i, j) = 5 * BASE * (i + 1) + 10
//   total      = sum over i in 0..3 of feat(i)
//              = 5 * 7 * (1 + 2 + 3 + 4) + 4 * 10 = 350 + 40 = 390

function fanFiles() {
  const files = new Map();
  files.set('fan:shared.js', `export const BASE = ${BASE};\n`);
  for (let i = 0; i < FEATURES; i += 1) {
    const imports = [];
    const names = [];
    for (let j = 0; j < LEAVES; j += 1) {
      files.set(
        `fan:leaf-${i}-${j}.js`,
        `import { BASE } from '~shared';\nexport const v = BASE * __LEAF_FACTOR__ + ${j};\n`,
      );
      imports.push(`import { v as l${j} } from '~leaf-${i}-${j}';`);
      names.push(`l${j}`);
    }
    files.set(
      `fan:feat-${i}.js`,
      `${imports.join('\n')}\nexport const sum${i} = ${names.join(' + ')};\n`,
    );
  }
  const featureNames = Array.from({ length: FEATURES }, (_, i) => `sum${i}`);
  files.set(
    'fan:entry.js',
    [
      ...featureNames.map((name, i) => `import { ${name} } from '~feat-${i}';`),
      `export const total = ${featureNames.join(' + ')};`,
      "export const stamp = '__STAMP__';",
      'export const moduleTag = String(total) + stamp;',
    ].join('\n') + '\n',
  );
  // Two more entries over the SAME fan, for the concurrency case. Each adds an
  // offset only it supplies, so the two concurrent results are told apart by the
  // value they compute: an empty, cross-wired or silently shared result cannot
  // fake the other side's total.
  for (const [tag, offset] of [
    ['a', 1000],
    ['b', 2000],
  ]) {
    files.set(
      `fan:entry-${tag}.js`,
      [
        ...featureNames.map((name, i) => `import { ${name} } from '~feat-${i}';`),
        `export const tag = '${tag}';`,
        `export const total = ${featureNames.join(' + ')} + ${offset};`,
      ].join('\n') + '\n',
    );
  }
  // An entry whose sole import can never be resolved by any hook.
  files.set(
    'fan:bad-entry.js',
    "import { q } from './definitely-not-resolvable.js';\nexport const y = q;\n",
  );
  return files;
}

function newTrace() {
  return {
    resolved: [],
    loaded: [],
    transformed: [],
    generateBundleCalls: 0,
    bundleKeys: [],
  };
}

function fanPlugin(trace, files) {
  return {
    name: 'workerd-suite-fan',
    resolveId(source, importer) {
      if (files.has(source)) {
        trace.resolved.push(source);
        return source;
      }
      if (source.startsWith('~') && typeof importer === 'string' && importer.startsWith('fan:')) {
        const target = `fan:${source.slice(1)}.js`;
        if (files.has(target)) {
          trace.resolved.push(target);
          return target;
        }
      }
      return null;
    },
    load(id) {
      if (!files.has(id)) return null;
      trace.loaded.push(id);
      return files.get(id);
    },
    transform(code, id) {
      if (!id.startsWith('fan:')) return null;
      let next = code;
      let touched = false;
      if (next.includes('__LEAF_FACTOR__')) {
        const match = /^fan:leaf-(\d+)-\d+\.js$/.exec(id);
        if (!match) throw new Error(`__LEAF_FACTOR__ token outside a leaf module: ${id}`);
        next = next.replaceAll('__LEAF_FACTOR__', String(Number(match[1]) + 1));
        touched = true;
      }
      if (next.includes('__STAMP__')) {
        next = next.replaceAll('__STAMP__', '-transformed-by-workerd-suite');
        touched = true;
      }
      if (!touched) return null;
      trace.transformed.push(id);
      return { code: next };
    },
  };
}

/** A generateBundle hook that emits an asset derived from the real bundle. */
function emitManifestPlugin(trace) {
  return {
    name: 'workerd-suite-manifest',
    generateBundle(_options, bundle) {
      trace.generateBundleCalls += 1;
      trace.bundleKeys = Object.keys(bundle);
      this.emitFile({
        type: 'asset',
        fileName: 'suite-manifest.json',
        source: JSON.stringify({ entries: trace.bundleKeys }),
      });
    },
  };
}

// ---------------------------------------------------------------------------
// CASE 1: a real multi-module build through the high-level `build()` API.
async function caseMultiModuleBuild() {
  const trace = newTrace();
  const files = fanFiles();
  const before = getWorkerdRuntimeStats().liveInstances;
  const result = await build({
    module: wasmModule,
    input: 'fan:entry.js',
    plugins: [fanPlugin(trace, files), emitManifestPlugin(trace)],
    output: { format: 'esm' },
  });
  const after = getWorkerdRuntimeStats().liveInstances;

  const chunks = result.output.filter((item) => item.type === 'chunk');
  const assets = result.output.filter((item) => item.type === 'asset');
  const chunk = chunks[0];
  const manifest = assets.find((asset) => asset.fileName === 'suite-manifest.json');

  return {
    chunkCount: chunks.length,
    assetCount: assets.length,
    fileName: chunk?.fileName ?? null,
    // The driver executes this code in Node and checks the computed exports.
    code: chunk?.code ?? null,
    moduleCount: chunk ? Object.keys(chunk.modules).length : 0,
    loadedCount: new Set(trace.loaded).size,
    transformedCount: new Set(trace.transformed).size,
    resolvedDistinct: new Set(trace.resolved).size,
    generateBundleCalls: trace.generateBundleCalls,
    manifestSource: manifest ? String(manifest.source) : null,
    leftoverLeafToken: chunk?.code?.includes('__LEAF_FACTOR__') ?? null,
    leftoverStampToken: chunk?.code?.includes('__STAMP__') ?? null,
    liveDelta: after - before,
  };
}

// ---------------------------------------------------------------------------
// CASE 2: the error surface of an unresolvable import.
async function caseErrorSurface() {
  const files = fanFiles();
  const baseline = getWorkerdRuntimeStats().liveInstances;
  const instance = await createInstance(wasmModule);
  let caught = null;
  let reuse = null;
  try {
    try {
      await build({
        instance,
        input: 'fan:bad-entry.js',
        plugins: [fanPlugin(newTrace(), files)],
      });
    } catch (error) {
      caught = errInfo(error);
    }
    // Contract: a failed build leaves the very same instance fully usable.
    const trace = newTrace();
    const result = await build({
      instance,
      input: 'fan:entry.js',
      plugins: [fanPlugin(trace, files)],
      output: { format: 'esm' },
    });
    reuse = {
      ok: result.output[0]?.type === 'chunk',
      loadedCount: new Set(trace.loaded).size,
    };
  } finally {
    await instance.dispose();
  }
  // Read before the owned-instance half below, so the two deltas cannot mask
  // each other.
  const callerOwnedLiveDelta = getWorkerdRuntimeStats().liveInstances - baseline;

  // The half above disposes explicitly; `build({ module })` is the path that can
  // leak, since only build() itself can dispose its private instance.
  const ownedBaseline = getWorkerdRuntimeStats().liveInstances;
  let ownedCaught = null;
  try {
    await build({
      module: wasmModule,
      input: 'fan:bad-entry.js',
      plugins: [fanPlugin(newTrace(), files)],
    });
  } catch (error) {
    ownedCaught = errInfo(error);
  }
  const ownedFailure = {
    caught: ownedCaught,
    liveDelta: getWorkerdRuntimeStats().liveInstances - ownedBaseline,
  };

  return {
    caught,
    reuse,
    disposed: instance.disposed,
    liveDelta: callerOwnedLiveDelta,
    ownedFailure,
  };
}

// ---------------------------------------------------------------------------
// CASE 3: instance lifecycle contracts. A build holds its instance from the
// first hook to the end of closeBundle; dispose() is refused for that whole
// window and succeeds once the build settles.
async function caseLifecycle() {
  const files = fanFiles();
  const baseline = getWorkerdRuntimeStats().liveInstances;
  const instanceA = await createInstance(wasmModule);
  const instanceB = await createInstance(wasmModule);
  const out = {};
  try {
    let release;
    const parked = new Promise((resolve) => {
      release = resolve;
    });
    let entered;
    const enteredHook = new Promise((resolve) => {
      entered = resolve;
    });
    let disposeInCloseBundle = null;
    const holdPlugin = { ...fanPlugin(newTrace(), files), name: 'workerd-suite-hold' };
    holdPlugin.load = async (id) => {
      if (!files.has(id)) return null;
      if (id === 'fan:shared.js') {
        entered();
        await parked;
      }
      return files.get(id);
    };
    holdPlugin.closeBundle = async () => {
      disposeInCloseBundle = await settle(instanceA.dispose());
    };
    const slow = build({ instance: instanceA, input: 'fan:entry.js', plugins: [holdPlugin] });
    slow.catch(() => {});
    await enteredHook;

    out.disposeWhileBuilding = await settle(instanceA.dispose());
    release();
    out.slowBuild = await settle(slow, isChunk);
    out.disposeInCloseBundle = disposeInCloseBundle;

    // The settled build released the slot, so a *second* instance now builds.
    out.otherInstanceAfterBuild = await settle(
      build({
        instance: instanceB,
        input: 'fan:entry.js',
        plugins: [fanPlugin(newTrace(), files)],
      }),
      isChunk,
    );
  } finally {
    out.disposeAfterBuild = await settle(instanceA.dispose());
    await instanceB.dispose();
  }
  out.memoryAfterDispose = await settle((async () => instanceA.memory)());
  out.buildAfterDispose = await settle(
    build({ instance: instanceA, input: 'fan:entry.js', plugins: [fanPlugin(newTrace(), files)] }),
    isChunk,
  );
  out.instanceADisposed = instanceA.disposed;
  out.instanceBDisposed = instanceB.disposed;
  out.liveDelta = getWorkerdRuntimeStats().liveInstances - baseline;
  return out;
}

/**
 * Flatten one half of the concurrent pair. The driver executes `code` and checks
 * `entriesLoaded`, so a build that returns the other's output or nothing fails.
 */
function describeConcurrent(settled, trace) {
  if (settled.status !== 'fulfilled') {
    return { ok: false, error: errInfo(settled.reason) };
  }
  const chunk = settled.value.output.find((item) => item.type === 'chunk');
  return {
    ok: chunk !== undefined,
    fileName: chunk?.fileName ?? null,
    code: chunk?.code ?? null,
    loadedCount: new Set(trace.loaded).size,
    entriesLoaded: Array.from(new Set(trace.loaded.filter((id) => id.startsWith('fan:entry-')))),
  };
}

// ---------------------------------------------------------------------------
// CASE 4: concurrency and single-slot admission.
async function caseConcurrency() {
  const files = fanFiles();
  const baseline = getWorkerdRuntimeStats().liveInstances;
  const out = {};

  // (a) Two concurrent builds on ONE shared instance must both succeed, each
  //     with ITS OWN result -- hence distinct entries with distinct totals.
  {
    const shared = await createInstance(wasmModule);
    const traceA = newTrace();
    const traceB = newTrace();
    try {
      const [first, second] = await Promise.allSettled([
        build({
          instance: shared,
          input: 'fan:entry-a.js',
          plugins: [fanPlugin(traceA, files)],
          output: { format: 'esm' },
        }),
        build({
          instance: shared,
          input: 'fan:entry-b.js',
          plugins: [fanPlugin(traceB, files)],
          output: { format: 'esm' },
        }),
      ]);
      out.concurrentSameInstance = {
        first: describeConcurrent(first, traceA),
        second: describeConcurrent(second, traceB),
      };
    } finally {
      await shared.dispose();
    }
  }

  // (b) A build on a SECOND instance while the first is active is refused.
  //     The first build is parked inside its own load hook so the window is
  //     deterministic rather than timing-dependent.
  {
    const instanceA = await createInstance(wasmModule);
    const instanceB = await createInstance(wasmModule);
    let release;
    const parked = new Promise((resolve) => {
      release = resolve;
    });
    let entered;
    const enteredHook = new Promise((resolve) => {
      entered = resolve;
    });
    const holdPlugin = { ...fanPlugin(newTrace(), files), name: 'workerd-suite-hold' };
    holdPlugin.load = async (id) => {
      if (!files.has(id)) return null;
      if (id === 'fan:shared.js') {
        entered();
        await parked;
      }
      return files.get(id);
    };
    try {
      const slow = build({ instance: instanceA, input: 'fan:entry.js', plugins: [holdPlugin] });
      slow.catch(() => {});
      await enteredHook;
      out.secondInstanceWhileActive = await settle(
        build({
          instance: instanceB,
          input: 'fan:entry.js',
          plugins: [fanPlugin(newTrace(), files)],
        }),
        isChunk,
      );
      release();
      out.slowBuild = await settle(slow, isChunk);

      // (c) Once the first build released the slot, instance B is admitted.
      out.secondInstanceAfterRelease = await settle(
        build({
          instance: instanceB,
          input: 'fan:entry.js',
          plugins: [fanPlugin(newTrace(), files)],
        }),
        isChunk,
      );
    } finally {
      await instanceA.dispose();
      await instanceB.dispose();
    }
  }

  out.liveDelta = getWorkerdRuntimeStats().liveInstances - baseline;
  return out;
}

// ---------------------------------------------------------------------------
// CASE 6: fire-and-forget `this.load()` -- the documented un-awaited pattern
// (docs/apis/plugin-api/inter-plugin-communication.md) plus the self-load shape of
// packages/rolldown/tests/fixtures/plugin/context/cycle-load-error. An awaited
// load runs alongside as the control.
async function caseFireAndForgetLoad() {
  const files = fanFiles();
  const before = getWorkerdRuntimeStats().liveInstances;
  let awaitedCodeType = null;
  const result = await build({
    module: wasmModule,
    input: 'fan:entry.js',
    plugins: [
      {
        name: 'fire-and-forget-load',
        buildStart() {
          // Documented shape: trigger loading, deliberately no await.
          this.load({ id: 'fan:shared.js' });
        },
        async load(id) {
          // cycle-load-error shape: un-awaited self-load of the module
          // currently loading (a CYCLE_LOADING warning, never an error).
          this.load({ id });
          if (id === 'fan:entry.js') {
            const info = await this.load({ id: 'fan:leaf-0-0.js' });
            awaitedCodeType = typeof info?.code;
          }
          return null;
        },
      },
      fanPlugin(newTrace(), files),
    ],
    output: { format: 'esm' },
  });
  const after = getWorkerdRuntimeStats().liveInstances;
  return {
    chunkCount: result.output.filter((item) => item.type === 'chunk').length,
    awaitedCodeType,
    liveDelta: after - before,
  };
}

// CASE 7: failed builds never reach the native `invalidateJsSideCache` callback
// (it fires only after a successful generate), so they must release their option
// boxes at their own settle. The ~1 MiB banner rides inside the normalized
// options: a stranded box grows the arena ~1 MiB per failed build.
async function caseFailedBuildReuse() {
  const banner = '/*' + 'x'.repeat(1024 * 1024) + '*/';
  const rounds = 10;
  const warmup = 2;
  const instance = await createInstance(wasmModule);
  const mem = [];
  let failures = 0;
  let sawOptions = 0;
  try {
    for (let i = 0; i < rounds; i++) {
      try {
        await build({
          instance,
          input: 'virt:main.js',
          plugins: [
            {
              name: 'failing-load',
              buildStart(opts) {
                // Read through the options box so the wrapper (and its
                // retained box) actually exists, mirroring real plugins.
                if (opts.platform) sawOptions += 1;
              },
              resolveId: (source) => (source === 'virt:main.js' ? source : null),
              load() {
                throw new Error('deliberate load failure');
              },
            },
          ],
          output: { format: 'esm', banner },
        });
        return { unexpectedSuccess: true };
      } catch (e) {
        if (!/deliberate load failure/.test(String(e?.message ?? e))) throw e;
        failures += 1;
      }
      mem.push(instance.memoryBytes);
    }
    const recovered = await build({
      instance,
      input: 'virt:main.js',
      plugins: [
        {
          name: 'recovering-load',
          resolveId: (source) => (source === 'virt:main.js' ? source : null),
          load: () => 'export default 1;',
        },
      ],
      output: { format: 'esm' },
    });
    const post = mem.slice(warmup);
    return {
      failures,
      sawOptions,
      slopeMiBPerFailedBuild: (post.at(-1) - post[0]) / (post.length - 1) / (1024 * 1024),
      recoveredChunkCount: recovered.output.filter((item) => item.type === 'chunk').length,
    };
  } finally {
    await instance.dispose();
  }
}

// CASE 8: `map.toUrl()` in an isolate with no `Buffer` global. The source is
// non-ASCII so the data URL must carry UTF-8 bytes, not UTF-16 code units. At
// ~190 KB (the map's `sourcesContent` carries it) the encoder crosses several
// chunks, with multi-byte sequences throughout. The driver decodes the payload
// on its own side.
const UTF8_SOURCE = Array.from(
  { length: 4000 },
  (_, i) => `export const g${i} = 'h\u00e9llo \u6f22\u5b57 \u{1f642} ${i}';\n`,
).join('');

async function caseSourcemapUrl() {
  const result = await build({
    module: wasmModule,
    input: 'virt:utf8.js',
    plugins: [
      {
        name: 'workerd-suite-utf8-source',
        resolveId: (source) => (source === 'virt:utf8.js' ? source : null),
        load: (id) => (id === 'virt:utf8.js' ? UTF8_SOURCE : null),
      },
    ],
    output: { format: 'esm', sourcemap: true },
  });
  const map = result.output[0].map;
  return {
    bufferGlobal: typeof Buffer,
    source: UTF8_SOURCE,
    mapString: map.toString(),
    url: map.toUrl(),
  };
}

// ---------------------------------------------------------------------------
// CASE 5: capabilities, as reported from inside workerd. The handle has no raw
// exports, so read them the way the pipeline does: a parallel plugin is refused
// by `rolldown()` with an error carrying the active instance's capability report.
async function caseCapabilities() {
  const instance = await createInstance(wasmModule);
  try {
    const refused = await build({
      instance,
      input: 'fan:entry.js',
      plugins: [{ name: 'parallel', _parallel: { fileUrl: 'file:///parallel.js', options: null } }],
    }).then(
      () => null,
      (error) => error,
    );
    const caps = refused?.runtime ?? {};
    return {
      refusedCode: refused?.code ?? null,
      instanceKeys: Object.keys(instance).sort(),
      target: caps.target,
      flavor: caps.flavor,
      threads: caps.threads,
      wasi: caps.wasi,
      watchSupported: caps.watchSupported,
      devSupported: caps.devSupported,
      entryExports: Object.keys(workerdApi).sort(),
      watchExported: 'watch' in workerdApi,
      memoryBytes: instance.memoryBytes,
    };
  } finally {
    await instance.dispose();
  }
}

// ---------------------------------------------------------------------------
// PART 2: memory slope across identical rebuilds on ONE reusable instance. The
// graph is deliberately larger than the behaviour fan so per-rebuild retention
// stays well above measurement noise.
function makeSlopeGraph(moduleCount) {
  const files = new Map();
  const utils = 4;
  for (let u = 0; u < utils; u += 1) {
    files.set(
      `slope:util-${u}.js`,
      [
        `export function util${u}(x) { return (x * ${u + 1} + ${u}) % 65521; }`,
        `export const NAME_${u} = 'util-${u}';`,
        `export function fmt${u}(v) { return NAME_${u} + ':' + util${u}(v); }`,
      ].join('\n'),
    );
  }
  for (let i = 0; i < moduleCount; i += 1) {
    const u = i % utils;
    const lines = [];
    lines.push(
      i + 1 < moduleCount
        ? `import { value as next } from 'slope:mod-${i + 1}.js';`
        : 'const next = 1;',
    );
    lines.push(`import { util${u}, fmt${u} } from 'slope:util-${u}.js';`);
    // Periodic fat modules keep the rendered output substantial.
    if (i % 25 === 24) {
      for (let k = 0; k < 60; k += 1) {
        lines.push(
          `export function handler_${i}_${k}(input) { const s = fmt${u}(input + ${k}); return input % ${k + 2} === 0 ? s + '-' + util${u}(input ^ ${k}) : s; }`,
        );
      }
    }
    lines.push(`export const value = util${u}(${i}) + next;`);
    lines.push(`export function describe_${i}() { return fmt${u}(value); }`);
    files.set(`slope:mod-${i}.js`, lines.join('\n'));
  }
  const fanOut = Math.min(moduleCount, 8);
  const entry = [];
  for (let f = 0; f < fanOut; f += 1) {
    const target = Math.floor((moduleCount / fanOut) * f);
    entry.push(
      `import { value as v${f}, describe_${target} as d${f} } from 'slope:mod-${target}.js';`,
    );
  }
  entry.push(
    `export const total = [${Array.from({ length: fanOut }, (_, f) => `v${f}`).join(', ')}].reduce((a, b) => a + b, 0);`,
  );
  entry.push(
    `export const banner = [${Array.from({ length: fanOut }, (_, f) => `d${f}()`).join(', ')}].join('|');`,
  );
  files.set('slope:entry.js', entry.join('\n'));
  return files;
}

function slopeGraphPlugin(files) {
  return {
    name: 'workerd-suite-slope-graph',
    resolveId(source) {
      return files.has(source) ? source : null;
    },
    load(id) {
      return files.get(id) ?? null;
    },
  };
}

async function caseMemorySlope(url) {
  const variant = url.searchParams.get('variant') ?? 'nohooks';
  const rounds = Number(url.searchParams.get('rounds') ?? 10);
  const moduleCount = Number(url.searchParams.get('modules') ?? 150);
  // renderChunk stack depth, supplied by the driver so it can predict `hookCalls`
  // (see STACKED_RENDER_CHUNK_PLUGINS there).
  const renderChunkPlugins = Number(url.searchParams.get('hooks') ?? 1);
  const files = makeSlopeGraph(moduleCount);

  const baseline = getWorkerdRuntimeStats().liveInstances;
  const instance = await createInstance(wasmModule);
  const memPerRound = [];
  const outputBytesPerRound = [];
  let hookCalls = 0;
  let modulesSeen = 0;
  let error = null;
  let disposeError = null;
  try {
    for (let round = 0; round < rounds; round += 1) {
      const plugins = [slopeGraphPlugin(files)];
      if (variant === 'generatebundle') {
        // What a real generateBundle hook does: read the chunks it is given.
        plugins.push({
          name: 'workerd-suite-generate-bundle',
          generateBundle(_options, bundle) {
            hookCalls += 1;
            let seen = 0;
            for (const item of Object.values(bundle)) {
              if (item.type === 'chunk') seen += item.code.length;
            }
            modulesSeen = seen;
          },
        });
      } else if (variant === 'renderchunk-nomodules' || variant === 'renderchunk-stacked') {
        // Receives the rendered chunk but never touches `modules`. Same hook for
        // both variants; only the stack depth differs.
        for (let p = 0; p < renderChunkPlugins; p += 1) {
          plugins.push({
            name: `workerd-suite-render-chunk-nomodules-${p}`,
            renderChunk(code, chunk) {
              hookCalls += 1;
              modulesSeen = code.length + chunk.fileName.length;
              return null;
            },
          });
        }
      } else if (variant === 'renderchunk') {
        for (let p = 0; p < renderChunkPlugins; p += 1) {
          plugins.push({
            name: `workerd-suite-render-chunk-${p}`,
            renderChunk(_code, chunk) {
              hookCalls += 1;
              // Reading `chunk.modules` marshals one object per module.
              modulesSeen = Object.keys(chunk.modules).length;
              return null;
            },
          });
        }
      } else if (variant === 'transform-ast') {
        // Reads `meta.ast` (the parseAst() path) once per MODULE. Each read mints
        // an oxc `ParseResult` freed only by reading its fields (`wrapParseResult`
        // in packages/rolldown/src/utils/parse.ts); `body.length` forces the program.
        plugins.push({
          name: 'workerd-suite-transform-ast',
          transform(_code, _id, meta) {
            hookCalls += 1;
            modulesSeen += meta.ast.body.length;
            return null;
          },
        });
      } else if (variant === 'transform-ast-error') {
        // The failing half: `this.parse()` on broken source throws, but oxc fills
        // the whole `ParseResult` first, so the throwing branch owes the same
        // drains. Appending to the real source keeps retention on the
        // `transform-ast` scale.
        plugins.push({
          name: 'workerd-suite-transform-ast-error',
          transform(code) {
            hookCalls += 1;
            let caught = null;
            try {
              this.parse(`${code}\nfunction (`);
            } catch (e) {
              caught = e;
            }
            // Those drains run BEFORE the error check, so this pins that they do
            // not eat the diagnostics on the way past: same code, same frame.
            if (caught?.code !== 'PARSE_ERROR' || !caught.message.includes('function (')) {
              throw new Error(
                'this.parse() must reject broken source with a framed PARSE_ERROR, got: ' +
                  `${caught?.message ?? '(no error)'}`,
              );
            }
            modulesSeen += 1;
            return null;
          },
        });
      } else if (variant !== 'nohooks') {
        throw new Error(`unknown memory variant: ${variant}`);
      }

      const result = await build({
        instance,
        input: 'slope:entry.js',
        plugins,
        output: { format: 'esm' },
      });
      let bytes = 0;
      for (const item of result.output) {
        bytes += item.type === 'chunk' ? item.code.length : String(item.source).length;
      }
      if (bytes === 0) throw new Error(`round ${round + 1} produced no output`);
      outputBytesPerRound.push(bytes);
      memPerRound.push(instance.memoryBytes);
    }
  } catch (e) {
    error = String(e?.stack ?? e);
  } finally {
    // Own field, never folded into `error`: if every round succeeded and only
    // teardown broke, swallowing it would pass the budget on a broken instance.
    try {
      await instance.dispose();
    } catch (e) {
      disposeError = String(e?.stack ?? e);
    }
  }

  return {
    variant,
    rounds,
    moduleCount,
    hookCalls,
    modulesSeen,
    memPerRound,
    memFirstMiB: memPerRound.length ? +(memPerRound[0] / MiB).toFixed(3) : null,
    memLastMiB: memPerRound.length ? +(memPerRound.at(-1) / MiB).toFixed(3) : null,
    // The floor every sample must clear.
    declaredInitialMemoryBytes: getWorkerdRuntimeStats().declaredInitialMemoryBytes,
    outputBytesPerRound,
    disposed: instance.disposed,
    liveDelta: getWorkerdRuntimeStats().liveInstances - baseline,
    error,
    disposeError,
  };
}

// ---------------------------------------------------------------------------
export default {
  async fetch(request) {
    const url = new URL(request.url);
    let payload;
    try {
      if (url.pathname === '/memory') {
        payload = await caseMemorySlope(url);
      } else if (url.pathname === '/behavior') {
        // One warm-up build first, so the cases start from a warm isolate: it
        // pays the first-touch Wasm growth.
        const files = fanFiles();
        await build({
          module: wasmModule,
          input: 'fan:entry.js',
          plugins: [fanPlugin(newTrace(), files)],
        });

        const report = {
          ok: true,
          baselineLiveInstances: getWorkerdRuntimeStats().liveInstances,
          cases: {},
        };
        const steps = [
          ['multiModuleBuild', caseMultiModuleBuild],
          ['errorSurface', caseErrorSurface],
          ['lifecycle', caseLifecycle],
          ['concurrency', caseConcurrency],
          ['capabilities', caseCapabilities],
          ['fireAndForgetLoad', caseFireAndForgetLoad],
          ['failedBuildReuse', caseFailedBuildReuse],
          ['sourcemapUrl', caseSourcemapUrl],
        ];
        for (const [name, fn] of steps) {
          try {
            report.cases[name] = await fn();
          } catch (e) {
            report.ok = false;
            report.cases[name] = { unexpected: errInfo(e), stack: String(e?.stack ?? e) };
          }
        }
        report.finalLiveInstances = getWorkerdRuntimeStats().liveInstances;
        payload = report;
      } else {
        payload = { error: `unknown route ${url.pathname}` };
      }
    } catch (e) {
      payload = { error: String(e?.stack ?? e) };
    }
    return new Response(JSON.stringify(payload), {
      headers: { 'content-type': 'application/json' },
    });
  },
};
