// Guard that a built dist contains EXACTLY the expected WASI artifact set for
// its flavor, and that only the threaded wasm links the heap-sync allocator.
// packages/rolldown/copy-addon-plugin.ts copies that set into dist;
// if its list drops a file (as happened with `wasip1-deferred`) the package
// ships without it while every build stays green. This guard holds its OWN copy
// of the canonical sets so that drift fails loudly here.
//
// Usage: node scripts/wasi/check-wasi-dist-files.mjs <threaded|single> [distDir]
//   flavor   threaded = wasm32-wasip1-threads dist (legacy `wasi` names)
//            single   = wasm32-wasip1 dist (`wasip1` names, deferred loader,
//                       no worker scripts)
//   distDir  defaults to packages/rolldown/dist (repo-relative); pass
//            packages/browser/dist for the @rolldown/browser publish path.

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import { EXPORT_KIND_FUNCTION, readExports } from './wasm-sections.mjs';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));

// Canonical per-flavor artifact sets. Keep in sync with the naming matrix in
// internal-docs/async-runtime/implementation.md — NOT with
// copy-addon-plugin.ts, whose drift this guard exists to catch.
const WASI_FILE_SETS = {
  threaded: [
    'rolldown-binding.wasm32-wasi.wasm',
    'rolldown-binding.wasi-browser.js',
    'rolldown-binding.wasi.cjs',
    'wasi-worker-browser.mjs',
    'wasi-worker.mjs',
  ],
  single: [
    'rolldown-binding.wasm32-wasip1.wasm',
    'rolldown-binding.wasip1-browser.js',
    'rolldown-binding.wasip1-deferred.js',
    'rolldown-binding.wasip1.cjs',
  ],
};

// WASI-artifact discriminator for top-level dist entries: `rolldown-binding.*`
// loaders/wasm of BOTH flavors (incl. `.debug.wasm` leftovers), `wasi-worker*`
// scripts, and any `.wasm` (every wasm in these dists is a WASI artifact).
// Deliberately name-prefix-anchored so hashed chunk files (e.g.
// `constructors-<hash>.js` in the browser dist) can never false-positive.
const WASI_ARTIFACT_RE = /^rolldown-binding\..*wasi|^wasi-worker|\.wasm$/;

const [flavor, distDirArg] = process.argv.slice(2);
if (flavor !== 'threaded' && flavor !== 'single') {
  console.error('Usage: node scripts/wasi/check-wasi-dist-files.mjs <threaded|single> [distDir]');
  process.exit(2);
}

const distDir = distDirArg
  ? path.resolve(process.cwd(), distDirArg)
  : path.join(REPO_ROOT, 'packages/rolldown/dist');

if (!fs.existsSync(distDir)) {
  console.error(`dist directory not found: ${distDir}`);
  process.exit(1);
}

const expected = WASI_FILE_SETS[flavor];
const supportFiles =
  flavor === 'single' && path.basename(path.dirname(distDir)) === 'browser'
    ? ['workerd-wasm.d.ts']
    : [];

// Strict set equality against the ACTUAL WASI-family subset of dist. Every
// build wipes dist first (packages/rolldown/build.ts) and the release workflow
// uploads dist/**, so anything extra is a packaging bug that would ship.
const entries = fs.readdirSync(distDir, { withFileTypes: true });
const wasiEntries = entries.filter((e) => WASI_ARTIFACT_RE.test(e.name));
const nonFiles = wasiEntries.filter((e) => !e.isFile()).map((e) => e.name);
const actual = new Set(wasiEntries.filter((e) => e.isFile()).map((e) => e.name));

const missing = expected.filter((f) => !actual.has(f));
const unexpected = [...actual].filter((f) => !expected.includes(f)).sort();
// Zero-byte artifacts are truncated/failed copies, not artifacts.
const empty = expected.filter(
  (f) => actual.has(f) && fs.statSync(path.join(distDir, f)).size === 0,
);
const missingSupportFiles = supportFiles.filter((f) => !fs.existsSync(path.join(distDir, f)));
const emptySupportFiles = supportFiles.filter(
  (f) => !missingSupportFiles.includes(f) && fs.statSync(path.join(distDir, f)).size === 0,
);

if (
  missing.length > 0 ||
  unexpected.length > 0 ||
  nonFiles.length > 0 ||
  empty.length > 0 ||
  missingSupportFiles.length > 0 ||
  emptySupportFiles.length > 0
) {
  console.error(`WASI dist file set mismatch for flavor '${flavor}' in ${distDir}:`);
  console.error();
  for (const f of missing) {
    console.error(`  missing:    ${f}`);
  }
  for (const f of unexpected) {
    console.error(`  unexpected: ${f} (not part of the '${flavor}' set)`);
  }
  for (const f of nonFiles) {
    console.error(`  non-file:   ${f} (WASI-family name but not a regular file)`);
  }
  for (const f of empty) {
    console.error(`  empty:      ${f} (0 bytes — truncated copy)`);
  }
  for (const f of missingSupportFiles) {
    console.error(`  missing:    ${f} (workerd package support file)`);
  }
  for (const f of emptySupportFiles) {
    console.error(`  empty:      ${f} (0 bytes — truncated support file)`);
  }
  console.error();
  console.error(
    'The packaged WASI artifact set must match the naming matrix in ' +
      'internal-docs/async-runtime/implementation.md. Check the ' +
      'WASM_FILE_LIST_* lists in packages/rolldown/copy-addon-plugin.ts and ' +
      'the TARGET wiring in packages/rolldown/build.ts.',
  );
  process.exit(1);
}

// The threaded wasm must carry the heap-sync allocator that works around V8's
// stale shared-memory size on threads that did not grow the memory; the
// single-thread wasm must not (it has one thread, and build.rs only adds the
// `--wrap` link args for wasm32-wasip1-threads). On the threaded wasm the
// exports named `malloc` / `free` (what @emnapi/core calls) must be the locked
// wrappers: packages/rolldown/build-binding.ts runs
// scripts/wasi/rename-wasm-allocator-exports.mjs, which points them at the
// functions exported as `rolldown_heap_sync_malloc` / `rolldown_heap_sync_free`
// and drops `__wrap_malloc` / `__wrap_free`. A wasm that skipped that step has
// no `malloc` export at all, so this check cannot pass on it.
// See internal-docs/wasi-shared-memory-grow/implementation.md
const HEAP_SYNC_EXPORTS = [
  '__wrap_calloc',
  '__wrap_realloc',
  '__wrap_posix_memalign',
  '__wrap_sbrk',
  'rolldown_heap_sync_malloc',
  'rolldown_heap_sync_free',
  'rolldown_heap_sync_stat',
];
const wasmFile = expected.find((f) => f.endsWith('.wasm'));
const wasmExports = new Map(
  readExports(new Uint8Array(fs.readFileSync(path.join(distDir, wasmFile)))).map((entry) => [
    entry.name,
    entry,
  ]),
);
const heapSyncFailures = [];
if (flavor === 'threaded') {
  for (const name of HEAP_SYNC_EXPORTS) {
    if (wasmExports.get(name)?.kind !== EXPORT_KIND_FUNCTION) {
      heapSyncFailures.push(
        `missing function export ${name} (threaded wasm must link the heap-sync allocator)`,
      );
    }
  }
  for (const name of ['malloc', 'free']) {
    const exported = wasmExports.get(name);
    const marker = wasmExports.get(`rolldown_heap_sync_${name}`);
    if (!exported) {
      heapSyncFailures.push(
        `missing export ${name} (scripts/wasi/rename-wasm-allocator-exports.mjs did not run)`,
      );
    } else if (
      !marker ||
      exported.kind !== EXPORT_KIND_FUNCTION ||
      exported.index !== marker.index
    ) {
      heapSyncFailures.push(
        `export ${name} is function ${exported.index}, not the heap-sync wrapper rolldown_heap_sync_${name} (function ${marker?.index})`,
      );
    }
    if (wasmExports.has(`__wrap_${name}`)) {
      heapSyncFailures.push(`unexpected export __wrap_${name} (the rename step must drop it)`);
    }
  }
} else {
  for (const name of wasmExports.keys()) {
    if (name.startsWith('__wrap_') || name.startsWith('rolldown_heap_sync_')) {
      heapSyncFailures.push(`unexpected export ${name} (heap-sync allocator is threaded-only)`);
    }
  }
}
if (heapSyncFailures.length > 0) {
  console.error(`WASI heap-sync export check failed for flavor '${flavor}' in ${wasmFile}:`);
  for (const failure of heapSyncFailures) {
    console.error(`  ${failure}`);
  }
  console.error();
  console.error(
    'Check the `--wrap` link args in crates/rolldown_binding/build.rs, the ' +
      '`rolldown_wasi_threads` cfg on crates/rolldown_binding/src/wasm_heap_sync.rs and the ' +
      'rename step in packages/rolldown/build-binding.ts.',
  );
  process.exit(1);
}

const packaged = expected.map((f) => {
  const { size } = fs.statSync(path.join(distDir, f));
  return `  ${f} (${size} bytes)`;
});
for (const file of supportFiles) {
  const { size } = fs.statSync(path.join(distDir, file));
  packaged.push(`  ${file} (${size} bytes)`);
}
console.log(
  `OK: '${flavor}' WASI dist file set complete in ${distDir} (heap-sync allocator ${flavor === 'threaded' ? 'linked, malloc/free renamed' : 'absent'}):`,
);
console.log(packaged.join('\n'));
