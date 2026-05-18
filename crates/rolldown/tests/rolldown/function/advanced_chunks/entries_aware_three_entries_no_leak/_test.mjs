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
const entryC = fs.readFileSync(path.join(dist, 'entry-c.js'), 'utf-8');

// Each entry must contain its own side effect
assert.ok(entryA.includes('side-effect-a'), 'entry-a.js should contain side-effect-a');
assert.ok(entryB.includes('side-effect-b'), 'entry-b.js should contain side-effect-b');
assert.ok(entryC.includes('side-effect-c'), 'entry-c.js should contain side-effect-c');

// No single chunk should contain side effects from different entries
const sideEffects = ['side-effect-a', 'side-effect-b', 'side-effect-c'];
for (const file of files) {
  const content = fs.readFileSync(path.join(dist, file), 'utf-8');
  const found = sideEffects.filter((se) => content.includes(se));
  assert.ok(
    found.length <= 1,
    `${file} contains side effects from multiple entries: [${found.join(', ')}]`,
  );
}
