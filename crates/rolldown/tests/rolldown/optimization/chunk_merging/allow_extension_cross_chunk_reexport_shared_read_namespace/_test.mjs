import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

// `seg.js` and `seg2.js` get no facade (see `mod.rs`).
const files = readdirSync(new URL('./dist', import.meta.url));
assert.deepStrictEqual(
  files.filter((file) => file.startsWith('seg')),
  [],
);

// `import()` gives the namespace object of each entry from the `group` chunk. The object has only
// the exports of the entry.
const main = await load('main');
assert.deepStrictEqual(Object.keys(await main.load()).sort(), ['first', 'foreign']);
assert.deepStrictEqual(Object.keys(await main.load2()).sort(), ['foreign', 'second']);
assert.strictEqual((await main.load()).foreign, 5);
assert.strictEqual((await main.load2()).foreign, 5);
