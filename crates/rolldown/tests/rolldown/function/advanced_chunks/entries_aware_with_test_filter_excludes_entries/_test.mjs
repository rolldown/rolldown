import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const dist = path.join(import.meta.dirname, 'dist');
const files = fs
  .readdirSync(dist)
  .filter((f) => f.endsWith('.js'))
  .sort();

const entryA = fs.readFileSync(path.join(dist, 'entry-a.js'), 'utf-8');
const entryB = fs.readFileSync(path.join(dist, 'entry-b.js'), 'utf-8');

// Entries contain their own side effects
assert.ok(entryA.includes('side-effect-a'), 'entry-a.js should contain side-effect-a');
assert.ok(entryB.includes('side-effect-b'), 'entry-b.js should contain side-effect-b');

// Shared code must be in vendor chunks, not in entry chunks
const vendorFiles = files.filter((f) => f.startsWith('vendor'));
assert.ok(vendorFiles.length > 0, 'should have at least one vendor chunk');

// No chunk should contain side effects from different entries
const sideEffects = ['side-effect-a', 'side-effect-b'];
for (const file of files) {
  const content = fs.readFileSync(path.join(dist, file), 'utf-8');
  const found = sideEffects.filter((se) => content.includes(se));
  assert.ok(
    found.length <= 1,
    `${file} contains side effects from multiple entries: [${found.join(', ')}]`,
  );
}
