import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const dist = path.join(import.meta.dirname, 'dist');
const files = fs
  .readdirSync(dist)
  .filter((f) => f !== 'package.json')
  .sort();

//when ertriesAware:true+ eentriesAwareMergeThreshold,lib-a and lib-b are small (~40 bytes each), below merge threshold (50).
// shared-dep is large (~110 bytes), above threshold,it will merged large chunks vendor~entry-a~entry-b.js

// However, entry-a and entry-b both have side effects, so loading entry-a would trigger entry-b.
// To isolate these side effects, extra vendor~entry-a and vendor~entry-b chunks are generated,
// ensuring that loading entry-a will not trigger entry-b.
assert.ok(
  files.includes('vendor~entry-a.js'),
  'lib-a subgroup should not merged (entry-a has side effect)',
);
assert.ok(
  files.includes('vendor~entry-b.js'),
  'lib-b subgroup should not merged (entry-b has side effect)',
);
