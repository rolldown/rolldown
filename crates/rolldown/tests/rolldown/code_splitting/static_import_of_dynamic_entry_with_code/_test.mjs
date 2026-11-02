import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.join(import.meta.dirname, 'dist');
const files = fs.readdirSync(dist).filter((file) => file.endsWith('.js'));
const aChunk = 'a.js';
assert.ok(files.includes(aChunk), 'a.js has a chunk of its own');

const loaded = new Set();
const pending = [aChunk];
while (pending.length) {
  const file = pending.pop();
  if (loaded.has(file)) continue;
  loaded.add(file);
  const code = fs.readFileSync(path.join(dist, file), 'utf8');
  for (const match of code.matchAll(/^import\s(?:[^'"]*\sfrom\s)?["']\.\/([^"']+)["']/gm)) {
    pending.push(match[1]);
  }
}
for (const file of loaded) {
  assert.ok(
    !fs.readFileSync(path.join(dist, file), 'utf8').includes('HEAVY'),
    `a.js statically loads ${file}, which holds heavy.js`,
  );
}

const { loadA, loadB, loadOwn } = await import('./dist/main.js');
assert.strictEqual((await loadA()).a(), 'foo');
assert.strictEqual((await loadB()).b(), 'HEAVYHEAVYHEAVYfoofoo');
assert.strictEqual(typeof (await loadOwn()).bar, 'function');
