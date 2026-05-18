import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const dist = path.join(import.meta.dirname, 'dist');
const files = fs
  .readdirSync(dist)
  .filter((f) => f.endsWith('.js'))
  .sort();

// Each entry chunk must contain its own side effect
const entryA = fs.readFileSync(path.join(dist, 'entry-a.js'), 'utf-8');
const entryB = fs.readFileSync(path.join(dist, 'entry-b.js'), 'utf-8');

assert.ok(
  entryA.includes('side-effect-a'),
  'entry-a.js should contain side-effect-a',
);
assert.ok(
  entryB.includes('side-effect-b'),
  'entry-b.js should contain side-effect-b',
);

// No single chunk should contain both entries' side effects
for (const file of files) {
  const content = fs.readFileSync(path.join(dist, file), 'utf-8');
  const hasA = content.includes('side-effect-a');
  const hasB = content.includes('side-effect-b');
  assert.ok(
    !(hasA && hasB),
    `${file} contains both side-effect-a and side-effect-b — entry side effects leaked across entries`,
  );
}
