import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import assert from 'node:assert';

const distDir = join(import.meta.dirname, 'dist');
const jsFiles = readdirSync(distDir).filter((f) => f.endsWith('.js'));

// With the fix: services are merged into route chunks, producing 3 chunks
// (main + route0 + route1). Without the fix (#8371 regression), the cycle
// detection falsely blocks the merge, creating an extra common chunk (4 chunks).
// Under preserveEntrySignatures: 'strict', the entry can't gain extra exports,
// so the shared service stays in its own chunk (4 chunks is expected).
const isStrict =
  globalThis.__configName === 'extended-preserve-entry-signatures-strict';
const expected = isStrict ? 4 : 3;
assert.strictEqual(
  jsFiles.length,
  expected,
  `Expected ${expected} chunks but got ${jsFiles.length}: ${jsFiles.join(', ')}`,
);
