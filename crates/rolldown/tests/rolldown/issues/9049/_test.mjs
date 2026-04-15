import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import assert from 'node:assert';

const distDir = join(import.meta.dirname, 'dist');
const jsFiles = readdirSync(distDir).filter((f) => f.endsWith('.js'));

// With the fix: services are merged into route chunks, producing 3 chunks
// (main + route0 + route1). Without the fix (#8371 regression), the cycle
// detection falsely blocks the merge, creating an extra common chunk (4 chunks).
assert.strictEqual(
  jsFiles.length,
  3,
  `Expected 3 chunks but got ${jsFiles.length}: ${jsFiles.join(', ')}`,
);
