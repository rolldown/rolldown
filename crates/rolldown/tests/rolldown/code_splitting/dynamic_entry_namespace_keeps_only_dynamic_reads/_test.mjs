import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const dist = path.join(import.meta.dirname, 'dist');
const imports = new Map();
for (const file of fs.readdirSync(dist).filter((name) => name.endsWith('.js'))) {
  const code = fs.readFileSync(path.join(dist, file), 'utf8');
  imports.set(
    file,
    [...code.matchAll(/^import\s(?:[^'"]*\sfrom\s)?["']\.\/([^"']+)["']/gm)].map(
      (match) => match[1],
    ),
  );
}
const visiting = new Set();
const done = new Set();
const visit = (file, trail) => {
  assert.ok(!visiting.has(file), `chunk import cycle: ${[...trail, file].join(' -> ')}`);
  if (done.has(file)) return;
  visiting.add(file);
  for (const imported of imports.get(file) ?? []) visit(imported, [...trail, file]);
  visiting.delete(file);
  done.add(file);
};
for (const file of imports.keys()) visit(file, []);

// main1 first: it enters the host chunk, so a cycle back from it would run `y.js` too early.
const main1 = await import('./dist/main1.js');
const main2 = await import('./dist/main2.js');
const main3 = await import('./dist/main3.js');
assert.strictEqual(main1.fromMain1, 'x');
assert.strictEqual(main2.fromMain2, 'yxx');
assert.strictEqual(main3.fromMain3, 'yxx');
assert.strictEqual(await main1.loadX(), 'x');

// Every getter a rebuilt namespace object keeps must resolve to a binding its chunk can see.
for (const file of imports.keys()) {
  for (const value of Object.values(await import(`./dist/${file}`))) {
    if (value?.[Symbol.toStringTag] !== 'Module') continue;
    for (const key of Object.keys(value)) assert.notStrictEqual(value[key], undefined, key);
  }
}
