import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// `a.js` reads `import.meta`, so it cannot be a record; `q.js` imports it from the entry's chunk, which therefore needs a facade anyway, and `d.js` merges into that chunk.
globalThis.events = [];
await import('./dist/main.js');
const q = await globalThis.loadQ();
const d = await globalThis.loadD();

assert.deepStrictEqual(globalThis.events, ['d', 'main stringd']);
assert.strictEqual(q.q, 'qstring');
assert.strictEqual(d.d, 'd');
const dist = path.join(import.meta.dirname, 'dist');
const files = fs.readdirSync(dist).filter((file) => file.endsWith('.js'));
assert.deepStrictEqual(files.sort(), ['main.js', 'main2.js', 'q.js', 'rolldown-runtime.js']);
for (const file of files) {
  assert(!fs.readFileSync(path.join(dist, file), 'utf8').includes('__share('), file);
}
