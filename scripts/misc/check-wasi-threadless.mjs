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
// Self-test / debugging: pass explicit loader paths as CLI args to point the
// guard at temporary copies (used to exercise the negative cases). Paths are
// resolved relative to the current working directory when not absolute.
const DEFAULT_LOADERS = [
  'packages/rolldown/src/rolldown-binding.wasi-browser.js',
  'packages/rolldown/src/rolldown-binding.wasi.cjs',
];
const argLoaders = process.argv.slice(2);
const LOADERS = argLoaders.length > 0 ? argLoaders : DEFAULT_LOADERS;
const resolveLoader = (rel) =>
  argLoaders.length > 0 ? path.resolve(process.cwd(), rel) : path.join(REPO_ROOT, rel);

// Markers that must NOT appear in a single-thread loader (threaded loaders add
// `new Worker(...)`, `onCreateWorker() {...}` and `shared: true`).
const FORBIDDEN = [/new Worker\b/, /onCreateWorker\b/, /shared:\s*true\b/];

// Strip comments so a commented `// asyncWorkPoolSize: 0` cannot satisfy the
// requirement and a commented `// new Worker(...)` cannot trip a false failure.
// We check only the EXECUTABLE source. The strip is intentionally conservative
// and dependency-free:
//   - block comments `/* ... */` are removed first;
//   - line comments are removed only when `//` starts a token (start-of-line or
//     after whitespace) so a `://` inside a string literal (e.g. a URL) is left
//     intact. The generated napi-rs loaders carry no `//`-in-string today; this
//     heuristic just keeps the guard from misfiring if one ever appears.
const stripComments = (source) =>
  source.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|\s)\/\/[^\n]*/g, '$1');

const failures = [];

for (const rel of LOADERS) {
  const abs = resolveLoader(rel);
  let source;
  try {
    source = fs.readFileSync(abs, 'utf8');
  } catch {
    failures.push(`${rel}: file not found — did \`just build-rolldown-wasi-single\` run?`);
    continue;
  }

  const code = stripComments(source);

  for (const pattern of FORBIDDEN) {
    if (pattern.test(code)) {
      failures.push(`${rel}: contains forbidden thread marker ${pattern}`);
    }
  }

  // The async work pool must be disabled. Collect EVERY executable
  // `asyncWorkPoolSize: <n>` occurrence: require at least one literal 0 and
  // reject any non-zero value (a presence-only check let `asyncWorkPoolSize: 4`
  // pass as long as a `: 0` appeared anywhere, including in a comment).
  const sizes = [...code.matchAll(/asyncWorkPoolSize:\s*(\d+)\b/g)].map((m) => Number(m[1]));
  if (sizes.length === 0) {
    failures.push(`${rel}: missing required option asyncWorkPoolSize: 0`);
  } else {
    const nonZero = sizes.filter((n) => n !== 0);
    if (nonZero.length > 0) {
      failures.push(
        `${rel}: non-zero asyncWorkPoolSize present (${nonZero.join(', ')}) — async work pool must be disabled (0)`,
      );
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
