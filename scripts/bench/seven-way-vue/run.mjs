#!/usr/bin/env node
// Bench six variants of Vue SFC compile (plugin-vue JS vs Vize Rust) on the
// Elk corpus. Mirrors scripts/bench/seven-way-react-compiler/run.mjs in
// structure; the `builtin` variant is dropped because rolldown core has no
// Vue compile path (no `transform.vue` option).

import { existsSync, readFileSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { performance } from 'node:perf_hooks';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { rolldown } from 'rolldown';
import { defineNativeLibPlugin, defineParallelPlugin } from 'rolldown/experimental';

import { compileVueAsync, compileVueSync } from './vue-utils.mjs';

const require = createRequire(import.meta.url);
const binding = require('../../../packages/rolldown/src/binding.cjs');

const __dirname = dirname(fileURLToPath(import.meta.url));
const CORPUS_JSON = join(__dirname, 'corpus.json');
const FIXTURE_DIR = join(__dirname, '.fixture');

// Path is now resolved inside BenchVizeTransformer's constructor (compile-time
// relative to crates/rolldown_binding). The `defineNativeLibPlugin` variant
// still needs an explicit path because it's generic across cdylibs.
const VIZE_LIB_PATH = resolve(
  __dirname,
  'native/target/release/libbench_vize_sfc_lib.dylib',
);

if (!existsSync(CORPUS_JSON)) {
  console.error('corpus.json not found. Run setup.mjs first.');
  process.exit(1);
}

const ITERATIONS = Number(process.env.ITERS ?? 6);
const corpus = JSON.parse(readFileSync(CORPUS_JSON, 'utf8'));
const ROOT = corpus.root;

// Skip-list (compile failures, panics) — applied to ALL variants for fairness
// if it exists. Format mirrors the React Compiler bench: { panicked, errored, timeouts }.
const SKIP_LIST_JSON = process.env.SKIP_LIST_JSON ?? join(__dirname, 'skip.json');
const skipSet = new Set();
if (!process.env.NO_SKIP && existsSync(SKIP_LIST_JSON)) {
  const skipData = JSON.parse(readFileSync(SKIP_LIST_JSON, 'utf8'));
  for (const f of skipData.panicked ?? []) skipSet.add(f);
  for (const f of skipData.errored ?? []) skipSet.add(f);
  for (const f of skipData.timeouts ?? []) skipSet.add(f);
}

const ALL_FILES = corpus.files;
const FILTERED_FILES = skipSet.size > 0
  ? ALL_FILES.filter((f) => !skipSet.has(f))
  : ALL_FILES;
const FILES = FILTERED_FILES.slice(0, Number(process.env.LIMIT ?? FILTERED_FILES.length));

if (skipSet.size > 0) {
  console.log(`skip-list: ${skipSet.size} files excluded`);
}
console.log(`corpus: ${FILES.length} .vue files under ${ROOT}`);
console.log(`iterations: ${ITERATIONS} (1 warm-up dropped, ${ITERATIONS - 1} measured)`);

const ENTRY_ID = '\0seven-way-vue-bench:entry';

// Stub emitted in place of an SFC the compiler couldn't handle. Has to be
// valid TS so rolldown's parser doesn't trip on it.
const STUB_MODULE = 'export default {};\n';

function makeBasePlugins() {
  const entrySource = FILES.map((f) => `import ${JSON.stringify(join(ROOT, f))};`).join('\n');
  return [
    {
      name: 'virtual-entry',
      resolveId(id) {
        if (id === ENTRY_ID) return id;
        // Externalize everything that isn't a .vue source file we own.
        // Elk relies on Nuxt auto-imports for `ref`, `useHead`, `useNuxtApp`,
        // etc. — none of those resolve standalone. We only care about the
        // SFC compile cost, not bundling correctness.
        if (!id.endsWith('.vue')) return { id, external: true };
        if (!id.startsWith(ROOT)) return { id, external: true };
        return null;
      },
      load(id) {
        if (id === ENTRY_ID) return entrySource;
        return null;
      },
    },
  ];
}

// `moduleType: 'ts'` because @vue/compiler-sfc leaves `import type` etc. in
// the compileScript output — it's plugin-vue's downstream esbuild-in-Vite
// step that normally strips TS. We let rolldown parse as TS so the
// downstream OXC parser handles those constructs. Vize's output is already
// TS-stripped, but `ts` is a superset so this works for both bridges too.
const transformer = new binding.BenchVizeTransformer();

// --- Variant 1a: utils-sync-js (@vue/compiler-sfc, sync hook) ---
function utilsSyncJsPlugin() {
  return {
    name: 'vue-bench-utils-sync-js',
    transform(code, id) {
      if (!id.endsWith('.vue')) return null;
      try {
        return { code: compileVueSync(id, code).code, moduleType: 'ts' };
      } catch {
        // Compiler choked. Replace with a stub so rolldown doesn't try to
        // parse the original .vue source as TS — that always fails.
        return { code: STUB_MODULE, moduleType: 'ts' };
      }
    },
  };
}

// --- Variant 1b: utils-async-js (@vue/compiler-sfc, async hook) ---
function utilsAsyncJsPlugin() {
  return {
    name: 'vue-bench-utils-async-js',
    async transform(code, id) {
      if (!id.endsWith('.vue')) return null;
      try {
        const r = await compileVueAsync(id, code);
        return { code: r.code, moduleType: 'ts' };
      } catch {
        return { code: STUB_MODULE, moduleType: 'ts' };
      }
    },
  };
}

// --- Variant 2a: utils-sync (Vize via napi string args, no handle bridge) ---
//
// Pays the JS-string marshalling cost on both the source argument and the
// returned code — UTF-16↔UTF-8 plus a heap allocation per call. The
// `BenchVizeTransformer.transformStr` napi method dispatches into the same
// cdylib as `transformNative` and `defineNativeLibPlugin` would.
function utilsSyncPlugin() {
  return {
    name: 'vue-bench-utils-sync',
    transform(code, id) {
      if (!id.endsWith('.vue')) return null;
      return { code: transformer.transformStr(code, id), moduleType: 'ts' };
    },
  };
}

// --- Variant 2b: utils-async (Vize via napi string args, async hook) ---
function utilsAsyncPlugin() {
  return {
    name: 'vue-bench-utils-async',
    async transform(code, id) {
      if (!id.endsWith('.vue')) return null;
      const out = await transformer.transformStrAsync(code, id);
      return { code: out, moduleType: 'ts' };
    },
  };
}

// --- Variants 3 + 4: bridge sync / async (Vize via dlopened cdylib + bigint handle) ---
function bridgeSyncPlugin() {
  return {
    name: 'vue-bench-bridge-sync',
    transformNativeBridge(handle) {
      try {
        return transformer.transformNative(handle);
      } catch {
        return undefined;
      }
    },
  };
}

function bridgeAsyncPlugin() {
  return {
    name: 'vue-bench-bridge-async',
    transformNativeBridgeAsync(handle) {
      return transformer.transformNativeAsync(handle).catch(() => undefined);
    },
  };
}

// --- Variant 5: native-lib (rolldown loads the same Vize cdylib directly) ---
function nativeLibPlugin() {
  return defineNativeLibPlugin({ name: 'vue-bench-native-lib', path: VIZE_LIB_PATH });
}

// --- Variant 6: bridge-parallel (one BenchVizeTransformer per worker) ---
const PARALLEL_IMPL = resolve(__dirname, 'parallel-impl.mjs');
const makeParallelPlugin = defineParallelPlugin(PARALLEL_IMPL);
function bridgeParallelPlugin() {
  return makeParallelPlugin({});
}

async function runOnce(variant) {
  const basePlugins = makeBasePlugins();
  let transformPlugin;

  switch (variant) {
    case 'utils-sync-js':
      transformPlugin = utilsSyncJsPlugin();
      break;
    case 'utils-async-js':
      transformPlugin = utilsAsyncJsPlugin();
      break;
    case 'utils-sync':
      transformPlugin = utilsSyncPlugin();
      break;
    case 'utils-async':
      transformPlugin = utilsAsyncPlugin();
      break;
    case 'bridge-sync':
      transformPlugin = bridgeSyncPlugin();
      break;
    case 'bridge-async':
      transformPlugin = bridgeAsyncPlugin();
      break;
    case 'native-lib':
      transformPlugin = nativeLibPlugin();
      break;
    case 'bridge-parallel':
      transformPlugin = bridgeParallelPlugin();
      break;
    default:
      throw new Error(`unknown variant: ${variant}`);
  }

  const plugins = [...basePlugins, transformPlugin];

  const t0 = performance.now();
  const bundle = await rolldown({
    input: ENTRY_ID,
    plugins,
    // Tell rolldown to parse .vue files as JS after the transform plugin
    // turns them into JS. Without this, rolldown defaults to inferring the
    // module type from the extension and rejects the SFC source as malformed
    // JS before the transform hook can run.
    moduleTypes: { '.vue': 'ts' },
    // Disable tsconfig discovery — Elk's tsconfig.json is excluded from our
    // sparse checkout, and rolldown otherwise climbs the FS looking for one
    // when modules are typed as `ts`.
    tsconfig: false,
    logLevel: 'silent',
    onLog() {
      // Swallow warnings/errors. Elk's SFCs reference Nuxt auto-imports as
      // unresolved bindings, which Vize/plugin-vue will sometimes flag. We're
      // measuring compile-step wall time, not bundling correctness.
    },
    shimMissingExports: true,
  });
  await bundle.generate({ format: 'esm' });
  await bundle.close();
  return performance.now() - t0;
}

function stats(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const min = sorted[0];
  const med = sorted[Math.floor(sorted.length / 2)];
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))];
  const mean = samples.reduce((a, b) => a + b, 0) / samples.length;
  return { min, med, p95, mean };
}

async function benchVariant(name) {
  console.log(`\n--- variant: ${name} ---`);
  const samples = [];
  for (let i = 0; i < ITERATIONS; i++) {
    rmSync(join(FIXTURE_DIR, `out-${name}`), { recursive: true, force: true });
    const ms = await runOnce(name);
    if (i === 0) {
      console.log(`  warm-up: ${ms.toFixed(1)} ms`);
    } else {
      console.log(`  iter ${i}: ${ms.toFixed(1)} ms`);
      samples.push(ms);
    }
  }
  return stats(samples);
}

const variants = (
  process.env.VARIANTS
    ?? 'utils-sync-js,utils-async-js,utils-sync,utils-async,bridge-sync,bridge-async,native-lib,bridge-parallel'
)
  .split(',')
  .map((v) => v.trim())
  .filter(Boolean);

const results = {};
for (const v of variants) {
  results[v] = await benchVariant(v);
}

console.log('\n--- summary (lower is better) ---');
for (const v of variants) {
  console.log(`${v.padEnd(16)}:`, results[v]);
}
const baseline = results['utils-sync-js'] ?? results['utils-sync'];
const baselineName = results['utils-sync-js'] ? 'utils-sync-js' : 'utils-sync';
if (baseline) {
  for (const v of variants) {
    if (v === baselineName) continue;
    const medX = (baseline.med / results[v].med).toFixed(3);
    const minX = (baseline.min / results[v].min).toFixed(3);
    console.log(`speedup ${baselineName}→${v.padEnd(16)} median: ${medX}x  min: ${minX}x`);
  }
}
