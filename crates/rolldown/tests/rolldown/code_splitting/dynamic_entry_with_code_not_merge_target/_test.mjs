import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.join(import.meta.dirname, 'dist');

function staticallyLoadedCode(entryFile) {
  const loaded = new Map();
  const pending = [entryFile];
  while (pending.length) {
    const file = pending.pop();
    if (loaded.has(file)) continue;
    const code = fs.readFileSync(path.join(dist, file), 'utf8');
    loaded.set(file, code);
    for (const match of code.matchAll(/^import\s(?:[^'"]*\sfrom\s)?["']\.\/([^"']+)["']/gm)) {
      pending.push(match[1]);
    }
  }
  return loaded;
}

for (const [file, code] of staticallyLoadedCode('r1.js')) {
  assert.ok(!code.includes('MODULE_B'), `r1.js statically loads ${file}, which holds lib/b.js`);
}
for (const [file, code] of staticallyLoadedCode('r2.js')) {
  assert.ok(!code.includes('MODULE_A'), `r2.js statically loads ${file}, which holds lib/a.js`);
}

const { loadR1, loadR2, loadLib } = await import('./dist/main.js');
assert.strictEqual((await loadR1()).r1(), 'MODULE_A');
assert.strictEqual((await loadR2()).r2(), 'MODULE_B');
assert.deepStrictEqual(
  (await loadLib()).all.map((item) => item.value),
  ['MODULE_A', 'MODULE_B'],
);
