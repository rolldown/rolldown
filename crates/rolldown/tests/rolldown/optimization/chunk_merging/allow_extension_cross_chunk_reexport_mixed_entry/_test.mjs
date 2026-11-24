import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { sep } from 'node:path';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

const index = await load('index');
const { foreign, required } = index;

// `seg_pure.js` keeps its facade, and the facade exports `pure` (issue #11767). In ESM output,
// `seg_external_named.js` also keeps its facade. In CJS output, its import from `node:path` is a
// property of a namespace, so it gets no facade. The facade of an entry has the name of the entry.
const facades = configName === 'cjs' ? ['seg_pure.js'] : ['seg_external_named.js', 'seg_pure.js'];
const files = readdirSync(new URL('./dist', import.meta.url));
assert.deepStrictEqual(files.filter((file) => file.startsWith('seg_')).sort(), facades);
assert.strictEqual((await load('seg_pure')).pure, foreign);
if (configName !== 'cjs') {
  const facade = await load('seg_external_named');
  assert.deepStrictEqual([facade.external_named, facade.sep], [foreign, sep]);
}

// An `import()` in the bundle gets the exports of each entry from the `group` chunk.
assert.strictEqual((await index.loadPure()).pure, foreign);
const star = await index.loadStar();
assert.deepStrictEqual([star.star, star.x], [foreign, 37]);
const barrel = await index.loadBarrel();
assert.deepStrictEqual([barrel.barrel, barrel.x], [foreign, 37]);
assert.strictEqual((await index.loadExternal()).external, foreign);
const externalNamed = await index.loadExternalNamed();
assert.deepStrictEqual([externalNamed.external_named, externalNamed.sep], [foreign, sep]);
const namespace = await index.loadNamespace();
assert.deepStrictEqual([namespace.namespace, namespace.lib.x], [foreign, 37]);
const named = await index.loadNamed();
assert.deepStrictEqual([named.named, named.named_x], [foreign, 37]);
assert.strictEqual((await index.loadCommonjs()).required, required);
