#!/usr/bin/env node
// Bench seven variants of the same React Compiler transform on the Infisical
// frontend corpus. See docs/superpowers/specs/2026-06-20-seven-way-react-compiler-bench-design.md.

import { existsSync, readFileSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { performance } from 'node:perf_hooks';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { rolldown } from 'rolldown';
import { defineNativeLibPlugin, defineParallelPlugin } from 'rolldown/experimental';
import { transform as utilsTransform, transformSync as utilsTransformSync } from 'rolldown/utils';

const require = createRequire(import.meta.url);
const binding = require('../../../packages/rolldown/src/binding.cjs');

const __dirname = dirname(fileURLToPath(import.meta.url));
const CORPUS_JSON = join(__dirname, 'corpus.json');
const FIXTURE_DIR = join(__dirname, '.fixture');

if (!existsSync(CORPUS_JSON)) {
  console.error('corpus.json not found. Run setup.mjs first.');
  process.exit(1);
}

const ITERATIONS = Number(process.env.ITERS ?? 6);
const corpus = JSON.parse(readFileSync(CORPUS_JSON, 'utf8'));
const ROOT = corpus.root;

// Skip-list of files that crash `builtin` with an oxc panic
// (`oxc_ecmascript-0.136.0/src/side_effects/statements.rs:98` unreachable
// when the AST still contains TS-only declarations like TSInterfaceDeclaration
// at side-effects analysis time). The JS-plugin variants don't hit this
// because they re-parse the transformed code, which strips TS leftovers.
//
// We apply the skip list to ALL variants when it exists, for a fair
// comparison on the same corpus subset. Set NO_SKIP=1 to disable.
const SKIP_LIST_JSON = process.env.SKIP_LIST_JSON ?? join(__dirname, 'builtin-skip.json');
let skipSet = new Set();
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
  console.log(`skip-list: ${skipSet.size} files excluded (builtin-panic)`);
}
console.log(`corpus: ${FILES.length} files under ${ROOT}`);
console.log(`iterations: ${ITERATIONS} (1 warm-up dropped, ${ITERATIONS - 1} measured)`);

const ENTRY_ID = '\0seven-way-bench:entry';
const SOURCE_EXTS = ['.tsx', '.ts', '.jsx', '.js', '.mjs', '.cjs'];
const isBareSpecifier = (s) =>
  !!s && !s.startsWith('.') && !s.startsWith('/') && !s.startsWith('\0');
const looksLikeSourceImport = (s) =>
  SOURCE_EXTS.some((ext) => s.endsWith(ext)) || !/\.[a-z0-9]+$/i.test(s);

// To match `builtin` (which runs React Compiler on every module the bundler
// touches), we don't filter here. React Compiler no-ops on non-React files
// at the oxc level, but parse+transform+codegen still happens per file.
const shouldTransform = (_id) => true;

function makeBasePlugins() {
  const entrySource = FILES.map((f) => `import ${JSON.stringify(join(ROOT, f))};`).join('\n');
  return [
    {
      name: 'virtual-entry',
      resolveId(id) {
        if (id === ENTRY_ID) return id;
        if (isBareSpecifier(id)) return { id, external: true };
        if (!looksLikeSourceImport(id)) return { id, external: true };
        return null;
      },
      load(id) {
        if (id === ENTRY_ID) return entrySource;
        return null;
      },
    },
  ];
}

// --- Variant 1: utils-sync ---
function utilsSyncPlugin() {
  return {
    name: 'oxc-bench-utils-sync',
    transform(code, id) {
      if (!shouldTransform(id)) return null;
      try {
        return utilsTransformSync(id, code, { reactCompiler: { panicThreshold: 'none' } }).code;
      } catch {
        return null;
      }
    },
  };
}

// --- Variant 2: utils-async ---
function utilsAsyncPlugin() {
  return {
    name: 'oxc-bench-utils-async',
    async transform(code, id) {
      if (!shouldTransform(id)) return null;
      try {
        const r = await utilsTransform(id, code, { reactCompiler: { panicThreshold: 'none' } });
        return r.code;
      } catch {
        return null;
      }
    },
  };
}

// --- Variants 3 + 4: bridge sync / async ---
const transformer = new binding.BenchOxcTransformer();

function bridgeSyncPlugin() {
  return {
    name: 'oxc-bench-bridge-sync',
    transformNativeBridge(sourceHandle, id) {
      if (!shouldTransform(id)) return undefined;
      try {
        return transformer.transformNative(sourceHandle, id);
      } catch {
        return undefined;
      }
    },
  };
}

function bridgeAsyncPlugin() {
  return {
    name: 'oxc-bench-bridge-async',
    transformNativeBridgeAsync(sourceHandle, id) {
      if (!shouldTransform(id)) return Promise.resolve(undefined);
      return transformer.transformNativeAsync(sourceHandle, id).catch(() => undefined);
    },
  };
}

// --- Variant 5: native-lib ---
const NATIVE_LIB_PATH = process.env.NATIVE_LIB_PATH ?? resolve(
  __dirname,
  '../../../target/release/libbench_native_lib_plugin.dylib',
);
function nativeLibPlugin() {
  return defineNativeLibPlugin({ name: 'oxc-bench-native-lib', path: NATIVE_LIB_PATH });
}

// --- Variant 7: bridge-parallel ---
const PARALLEL_IMPL = resolve(__dirname, 'parallel-impl.mjs');
const makeParallelPlugin = defineParallelPlugin(PARALLEL_IMPL);
function bridgeParallelPlugin() {
  return makeParallelPlugin({});
}

// Variant 6 (builtin) doesn't append a transform plugin; it sets the
// bundler-level `transform.reactCompiler` option instead.

async function runOnce(variant) {
  const basePlugins = makeBasePlugins();
  let transformPlugin;
  let bundlerTransform;

  switch (variant) {
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
    case 'builtin':
      transformPlugin = null;
      bundlerTransform = { reactCompiler: { panicThreshold: 'none' } };
      break;
    default:
      throw new Error(`unknown variant: ${variant}`);
  }

  const plugins = transformPlugin ? [...basePlugins, transformPlugin] : basePlugins;

  const t0 = performance.now();
  const bundle = await rolldown({
    input: ENTRY_ID,
    plugins,
    transform: bundlerTransform,
    logLevel: 'silent',
    onLog() {
      // Swallow warnings/errors. React Compiler is strict and emits a few
      // hundred per run on Infisical's frontend (refs during render, etc.) —
      // none of them affect transform timing, which is what we're measuring.
    },
    // Infisical's frontend has a handful of intra-tree type-only imports
    // imported as values. Without this rolldown fails with MISSING_EXPORT.
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
  process.env.VARIANTS ?? 'utils-sync,bridge-sync,native-lib,builtin,bridge-parallel'
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
const baseline = results['utils-sync'];
if (baseline) {
  for (const v of variants) {
    if (v === 'utils-sync') continue;
    const medX = (baseline.med / results[v].med).toFixed(3);
    const minX = (baseline.min / results[v].min).toFixed(3);
    console.log(`speedup utils-sync→${v.padEnd(16)} median: ${medX}x  min: ${minX}x`);
  }
}
