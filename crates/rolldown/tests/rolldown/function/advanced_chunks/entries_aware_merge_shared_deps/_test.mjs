import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const dist = path.join(import.meta.dirname, 'dist');
const files = fs
  .readdirSync(dist)
  .filter((f) => f !== 'package.json')
  .sort();

// lib-a and lib-b are small (~40 bytes each), below merge threshold (50).
// shared-dep is large (~110 bytes), above threshold.
//
// Correct: lib-a and lib-b subgroups merge → no separate vendor~entry-a/b.
// Bug (main): shared-dep duplicated into each subgroup → inflated sizes
// (lib-a + shared-dep ≈ 150 > 50) → merge skipped → separate chunks.
assert.ok(
  !files.includes('vendor~entry-a.js'),
  'lib-a subgroup should have merged (shared-dep must not inflate its size)',
);
assert.ok(
  !files.includes('vendor~entry-b.js'),
  'lib-b subgroup should have merged (shared-dep must not inflate its size)',
);
