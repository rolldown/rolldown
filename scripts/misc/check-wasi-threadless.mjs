// Guard that the generated single-thread WASI loaders are threadless
// end-to-end. The non-threaded build (`just build-rolldown-wasi-single`)
// regenerates these loaders via @napi-rs/cli; a future cli bump or a wrong
// `hasThreads` resolution could silently reintroduce a Worker / shared memory
// / non-zero async work pool into the shipped browser artifact while CI stays
// green. This mirrors the napi-rs upstream guard
// (examples/custom-async-runtime/test.mjs) for rolldown's own loaders.
//
// Run this AFTER `just build-rolldown-wasi-single` so it inspects the freshly
// generated single-thread loaders, not the threaded ones.

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = fileURLToPath(new URL('../../', import.meta.url));

// The browser loader is the shipped browser artifact and the primary target;
// the node (.cjs) loader is checked too since the same build regenerates both.
const LOADERS = [
  'packages/rolldown/src/rolldown-binding.wasi-browser.js',
  'packages/rolldown/src/rolldown-binding.wasi.cjs',
];

// Markers that must NOT appear in a single-thread loader (threaded loaders add
// `new Worker(...)`, `onCreateWorker() {...}` and `shared: true`).
const FORBIDDEN = [/new Worker\b/, /onCreateWorker\b/, /shared:\s*true\b/];
// Markers that MUST appear: the async work pool must be disabled.
const REQUIRED = [/asyncWorkPoolSize:\s*0\b/];

const failures = [];

for (const rel of LOADERS) {
  const abs = path.join(REPO_ROOT, rel);
  let source;
  try {
    source = fs.readFileSync(abs, 'utf8');
  } catch {
    failures.push(`${rel}: file not found — did \`just build-rolldown-wasi-single\` run?`);
    continue;
  }

  for (const pattern of FORBIDDEN) {
    if (pattern.test(source)) {
      failures.push(`${rel}: contains forbidden thread marker ${pattern}`);
    }
  }
  for (const pattern of REQUIRED) {
    if (!pattern.test(source)) {
      failures.push(`${rel}: missing required marker ${pattern}`);
    }
  }
}

if (failures.length > 0) {
  console.error('Single-thread WASI loaders are NOT threadless:');
  console.error();
  for (const f of failures) {
    console.error(`  ${f}`);
  }
  console.error();
  console.error(
    'The non-threaded build must emit loaders with no Worker creation, no shared memory, and asyncWorkPoolSize: 0. ' +
      'Check the @napi-rs/cli version / patch and the `hasThreads` target resolution.',
  );
  process.exit(1);
}

console.log(
  `OK: single-thread WASI loaders are threadless (no Worker, no onCreateWorker, no shared memory, asyncWorkPoolSize: 0) — checked ${LOADERS.join(', ')}.`,
);
